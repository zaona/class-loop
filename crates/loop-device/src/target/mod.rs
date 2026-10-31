//! Loop 目标集成：时钟、课表加载、路由与 Canopus 系统组件。
//!
//! 原生页面生命周期、系统列表组件与真实时钟。

use alloc::{format, string::String};
use core::sync::atomic::Ordering;

use loop_core::{
    Action, Effect, Route, ScheduleFile,
    view::{self, PageView},
};

use runtime::{APP_FAILED, initialized, runtime, try_with_core, with_core};

pub mod clock;
pub mod connect;
pub mod diagnostics;
pub mod firmware;
pub mod native_app;
pub mod runtime;
pub mod storage;
pub mod ui;

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
    let _ = execute_effects(effects);
    let error = connect::start();
    runtime().connect_error.store(error, Ordering::Release);
    0
}

pub fn query_status() -> [u32; 6] {
    let r = runtime();
    if !initialized() {
        return [
            APP_FAILED,
            0,
            r.last_error.load(Ordering::Acquire) as u32,
            0,
            0,
            0,
        ];
    }
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
        core[1],
        core[0],
        core[1],
    ]
}

fn apply_disk_schedule(core: &mut runtime::Core) {
    match storage::load_schedule() {
        Ok(Some(file)) => {
            if core.app.schedule != file {
                let _ = core.app.update(Action::Reload(file));
            }
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
    // 导入完成后的下一次 UI tick 重读私有课表；低频重读保留给手动管理。
    let tick = runtime().timer_ticks.fetch_add(1, Ordering::AcqRel) + 1;
    if runtime().schedule_dirty.swap(false, Ordering::AcqRel) || tick == 1 || tick % 50 == 0 {
        apply_disk_schedule(core);
    }
    if core.app.route == Route::Data && runtime().connect_error.load(Ordering::Acquire) != 0 {
        let message = String::from("当前固件或服务不支持 Connect 导入");
        if core.app.data_status != message {
            let _ = core.app.update(Action::SetDataStatus(message));
        }
    }
}

pub fn ui_maintenance_tick() {
    let _ = try_with_core(refresh_clock_and_schedule);
}
pub fn view_revision(index: usize) -> Option<u32> {
    try_with_core(|core| (core.app.route.page_index() == index).then_some(core.app.generation))
        .flatten()
}
pub fn page_view(index: usize) -> Option<PageView> {
    try_with_core(|core| (core.app.route.page_index() == index).then(|| view::present(&core.app)))
        .flatten()
}
pub fn sync_resumed_page(page_index: usize) -> i32 {
    let Some(route) = Route::from_page_index(page_index) else {
        return -1;
    };
    with_core(|core| {
        core.app.resume_route(route);
    });
    0
}

pub fn handle_back(page_index: usize) {
    let previous = with_core(|core| {
        if core.app.route.page_index() != page_index {
            return None;
        }
        let previous = core.app.navigation_state();
        let _ = core.app.update(Action::Back);
        Some(previous)
    });
    if let Some(previous) = previous {
        let rc = ui::back(page_index);
        if rc < 0 {
            restore_navigation(page_index, previous, rc);
        }
    }
}

pub fn handle_action(page_index: usize, generation: u32, action: Action) {
    let effects = with_core(|core| {
        if core.app.generation != generation || core.app.route.page_index() != page_index {
            return None;
        }
        Some((core.app.navigation_state(), core.app.update(action)))
    });
    let Some((previous, effects)) = effects else {
        return;
    };
    if let Err(rc) = execute_effects(effects) {
        restore_navigation(page_index, previous, rc);
    }
}

fn restore_navigation(index: usize, previous: loop_core::app::NavigationState, error: i32) {
    with_core(|core| core.app.restore_navigation(previous));
    runtime().last_error.store(error, Ordering::Release);
    let _ = ui::page_resume(index);
}

fn execute_effects(effects: alloc::vec::Vec<Effect>) -> Result<(), i32> {
    for effect in effects {
        match effect {
            Effect::Navigate(route) => {
                let rc = ui::navigate(route.page_index());
                if rc < 0 {
                    return Err(rc);
                }
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
            Effect::ClearStoredSchedule => {
                connect::abort();
                match storage::clear_schedule() {
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
                }
            }
        }
    }
    Ok(())
}
