//! Loop 目标集成：时钟、课表加载、路由与 LVGL 刷新。
//!
//! LVGL 只在 page owner 线程触碰；刷新定时器亦由该页自己创建。

use alloc::{format, string::String};
use core::sync::atomic::Ordering;

use loop_core::{Action, Effect, Route, ScheduleFile, ui};

use runtime::{initialized, runtime, try_with_core, with_core};

pub mod clock;
pub mod native_app;
pub mod runtime;
pub mod storage;
pub mod ui_backend;

pub fn prepare() {
    runtime::prepare();
}

pub fn activate() -> i32 {
    if !initialized() {
        return -1;
    }
    let result = canopus_target_private::canopus_identity_guard();
    if result != 0 {
        runtime().last_error.store(result, Ordering::Release);
        return result;
    }
    // activate 仍可能在较小栈上：先 Boot 空课表，首帧刷新再加载 JSON。
    let effects = with_core(|core| {
        core.app
            .update(Action::Boot(loop_core::ScheduleFile::default()))
    });
    execute_effects(effects);
    0
}

pub fn query_status() -> [u32; 6] {
    let r = runtime();
    let core = try_with_core(|core| {
        [
            core.app.generation,
            core.app.route.page_index() as u32,
            core.app.week() as u32,
        ]
    })
    .unwrap_or([u32::MAX; 3]);
    [
        r.app_state.load(Ordering::Acquire),
        r.app_error.load(Ordering::Acquire) as u32,
        r.last_error.load(Ordering::Acquire) as u32,
        r.active_page.load(Ordering::Acquire),
        core[0],
        core[1],
    ]
}

fn apply_disk_schedule(core: &mut runtime::Core) {
    match storage::load_schedule() {
        Ok(Some(file)) => {
            let _ = core.app.update(Action::Reload(file));
        }
        Ok(None) => {
            let empty =
                core.app.schedule.courses.is_empty() && core.app.schedule.term.name.is_empty();
            if !empty {
                let _ = core.app.update(Action::Reload(ScheduleFile::default()));
            }
        }
        Err(_) => {}
    }
}

fn refresh_clock_and_schedule(core: &mut runtime::Core) {
    if let Some(hint) = clock::read_clock() {
        let _ = core.app.update(Action::Tick(hint));
    }
    // 低频：每 50 次刷新（约 5s）尝试重读快应用 schedule.json。
    let tick = runtime().timer_ticks.fetch_add(1, Ordering::AcqRel) + 1;
    if tick == 1 || tick % 50 == 0 {
        apply_disk_schedule(core);
    }
}

pub fn rebuild(page_index: usize) -> i32 {
    if page_is_current(page_index) != 0 {
        return 0;
    }
    let snapshot = with_core(|core| {
        refresh_clock_and_schedule(core);
        loop_core::ui::render(&core.app)
    });
    match snapshot {
        Ok(snapshot) => ui_backend::apply_snapshot(page_index, &snapshot),
        Err(_) => -1,
    }
}

pub fn rebuild_if_changed(page_index: usize, rendered_generation: u32) -> i32 {
    if page_is_current(page_index) != 0 {
        return 0;
    }
    let snapshot = match try_with_core(|core| {
        refresh_clock_and_schedule(core);
        if core.app.generation == rendered_generation {
            None
        } else {
            Some(loop_core::ui::render(&core.app))
        }
    }) {
        Some(snapshot) => snapshot,
        None => return 0,
    };
    match snapshot {
        None => 0,
        Some(Ok(snapshot)) => ui_backend::apply_snapshot(page_index, &snapshot),
        Some(Err(_)) => -1,
    }
}

fn page_is_current(page_index: usize) -> i32 {
    let current = with_core(|core| core.app.route.page_index());
    if current == page_index { 0 } else { 1 }
}

pub fn sync_resumed_page(page_index: usize) -> i32 {
    let Some(route) = Route::from_page_index(page_index) else {
        return -1;
    };
    with_core(|core| {
        if core.app.route != route {
            if core.app.history.last().copied() == Some(route) {
                core.app.history.pop();
            } else if let Some(position) = core.app.history.iter().rposition(|item| *item == route)
            {
                core.app.history.truncate(position);
            } else {
                core.app.history.clear();
            }
            core.app.route = route;
            core.app.bump();
        }
    });
    0
}

pub fn handle_back(page_index: usize) {
    let should_finish = with_core(|core| {
        if core.app.route.page_index() != page_index {
            return false;
        }
        let _ = core.app.update(Action::Back);
        true
    });
    if should_finish {
        ui_backend::back(page_index);
    }
}

pub fn handle_ui_event(page_index: usize, generation: u32, _key: u32, event_id: u32) {
    if event_id == ui::EVENT_BACK {
        let valid = with_core(|core| core.app.generation == generation);
        if valid {
            handle_back(page_index);
        }
        return;
    }
    let effects = with_core(|core| {
        if core.app.generation != generation {
            return None;
        }
        action_for_event(event_id).map(|action| core.app.update(action))
    });
    let Some(effects) = effects else {
        return;
    };
    execute_effects(effects);
    let _ = rebuild(page_index);
}

fn action_for_event(event_id: u32) -> Option<Action> {
    match event_id {
        ui::EVENT_TODAY => Some(Action::Open(Route::Today)),
        ui::EVENT_WEEK => Some(Action::Open(Route::Week)),
        ui::EVENT_DATA => Some(Action::Open(Route::Data)),
        ui::EVENT_REFRESH => Some(Action::RefreshSchedule),
        ui::EVENT_CLEAR => Some(Action::ClearSchedule),
        event if (ui::EVENT_DAY_BASE + 1..=ui::EVENT_DAY_BASE + 7).contains(&event) => {
            Some(Action::SelectDay((event - ui::EVENT_DAY_BASE) as u8))
        }
        event if event >= ui::EVENT_COURSE_BASE => {
            Some(Action::SelectCourse(event - ui::EVENT_COURSE_BASE))
        }
        _ => None,
    }
}

fn execute_effects(effects: alloc::vec::Vec<Effect>) {
    for effect in effects {
        match effect {
            Effect::Navigate(route) => {
                ui_backend::navigate(route.page_index());
            }
            Effect::ReloadFromDisk => {
                with_core(|core| match storage::load_schedule() {
                    Ok(Some(file)) => {
                        let count = file.courses.len();
                        let _ = core.app.update(Action::Reload(file));
                        let _ = core
                            .app
                            .update(Action::SetDataStatus(format!("已刷新，共 {count} 门课")));
                    }
                    Ok(None) => {
                        let _ = core.app.update(Action::Reload(ScheduleFile::default()));
                        let _ = core
                            .app
                            .update(Action::SetDataStatus(String::from("无本地课表文件")));
                    }
                    Err(_) => {
                        let _ = core
                            .app
                            .update(Action::SetDataStatus(String::from("刷新失败")));
                    }
                });
            }
            Effect::ClearStoredSchedule => match storage::clear_schedule() {
                Ok(()) => {
                    with_core(|core| {
                        let _ = core.app.update(Action::Reload(ScheduleFile::default()));
                        let _ = core
                            .app
                            .update(Action::SetDataStatus(String::from("已清空本地课表")));
                    });
                }
                Err(_) => {
                    with_core(|core| {
                        let _ = core
                            .app
                            .update(Action::SetDataStatus(String::from("清空失败")));
                    });
                }
            },
        }
    }
}
