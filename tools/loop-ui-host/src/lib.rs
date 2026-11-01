//! Execute the actual native descriptors and stock-widget renderer without a device.
#![cfg(test)]
extern crate alloc;
use loop_core::{Action, ClockHint, Course, Effect, LoopApp, Route, ScheduleFile, Term};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    ffi::c_void,
    sync::atomic::{AtomicI32, AtomicU32},
};
#[path = "../../../crates/loop-device/src/target/native_app.rs"]
pub mod native_app;
#[path = "../../../crates/loop-device/src/target/ui/mod.rs"]
pub mod ui;
type Callback = extern "C" fn(*mut c_void);
#[derive(Default)]
struct Widget {
    parent: usize,
    text: String,
    detail: String,
    hidden: bool,
    width: i32,
    height: i32,
    y: i32,
    trailing: u8,
    hits: [usize; 3],
    callback: Option<Callback>,
    cookie: usize,
    scroll: i32,
}
#[derive(Default)]
struct Host {
    next: usize,
    widgets: BTreeMap<usize, Widget>,
    timers: BTreeMap<usize, Callback>,
    stack: Vec<(usize, usize)>,
    moves: Vec<(u32, u32)>,
    finishes: usize,
    fail_navigation: bool,
    fail_finish: bool,
    clears: usize,
    pending: Option<ScheduleFile>,
    disk: Option<ScheduleFile>,
    maintenance: usize,
}
thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::default());
    static APP: RefCell<LoopApp> = RefCell::new(LoopApp::default());
}
fn host<R>(f: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|h| f(&mut h.borrow_mut()))
}
fn app<R>(f: impl FnOnce(&mut LoopApp) -> R) -> R {
    APP.with(|a| f(&mut a.borrow_mut()))
}
fn widget(parent: usize, text: String, detail: String) -> *mut c_void {
    host(|h| {
        h.next += 1;
        let id = h.next;
        h.widgets.insert(
            id,
            Widget {
                parent,
                text,
                detail,
                ..Widget::default()
            },
        );
        id as *mut c_void
    })
}
fn with_widget<R>(p: *mut c_void, f: impl FnOnce(&mut Widget) -> R) -> R {
    host(|h| {
        f(h.widgets
            .get_mut(&(p as usize))
            .expect("access to destroyed native widget"))
    })
}
unsafe fn string(p: *const u8) -> String {
    unsafe {
        std::ffi::CStr::from_ptr(p.cast())
            .to_str()
            .unwrap()
            .to_owned()
    }
}
pub mod firmware {
    #![allow(non_camel_case_types)]
    use super::*;
    pub const BAND_11: bool = cfg!(feature = "target-xiaomi-band-11-4-100-155");
    pub const TRAILING_NONE: u8 = 0;
    pub const TRAILING_FORWARD: u8 = 3;
    pub const EVENT_ALL: u32 = 0;
    pub const EVENT_CLICKED: u32 = 7;
    pub const EVENT_PRESSED: u32 = 1;
    pub const EVENT_PRESS_LOST: u32 = 3;
    pub const ALIGN_TOP_MID: u32 = 2;
    pub const ALIGN_OUT_BOTTOM_MID: u32 = 14;
    pub const CONTENT_WIDTH: i32 = if BAND_11 { 212 } else { 336 };
    pub const CONTENT_HEIGHT: i32 = if BAND_11 { 444 } else { 424 };
    pub const CONTENT_TOP_OFFSET: i32 = if BAND_11 { 68 } else { 56 };
    pub const ROW_GAP: i32 = 8;
    pub const STYLE_MISANS_REGULAR_24: usize = 24;
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct firmware_page_descriptor {
        pub page_name: *mut c_void,
        pub page_id: u16,
        pub app_id: u16,
        pub on_signal: *mut c_void,
        pub on_create: *mut c_void,
        pub on_resume: *mut c_void,
        pub on_pause: *mut c_void,
        pub on_destroy: *mut c_void,
        pub extension_callback_100: *mut c_void,
        pub on_ui_destroy: *mut c_void,
    }
    pub unsafe fn lvx_content_create(p: *mut c_void) -> *mut c_void {
        widget(p as usize, String::new(), String::new())
    }
    pub unsafe fn lvx_object_set_size(p: *mut c_void, w: i32, h: i32) {
        with_widget(p, |v| {
            v.width = w;
            v.height = h;
        });
    }
    pub unsafe fn lvx_object_align(p: *mut c_void, _: u32, _: i32, y: i32) {
        with_widget(p, |v| v.y = y);
    }
    pub unsafe fn lvx_object_set_content_pad_bottom(p: *mut c_void, value: i32, _: u32) {
        assert_eq!(value, 32);
        with_widget(p, |_| ());
    }
    pub unsafe fn lvx_page_title_create(
        p: *mut c_void,
        s: *const u8,
        mode: u32,
        cb: *const (),
        _: *mut c_void,
    ) -> *mut c_void {
        assert_eq!(mode != 0, !cb.is_null());
        let object = widget(p as usize, unsafe { string(s) }, String::new());
        if !cb.is_null() {
            with_widget(object, |v| {
                v.callback = Some(unsafe { std::mem::transmute::<*const (), Callback>(cb) })
            });
        }
        object
    }
    pub unsafe fn lvx_label_create(p: *mut c_void) -> *mut c_void {
        widget(p as usize, String::new(), String::new())
    }
    pub unsafe fn lvx_style_apply(p: *mut c_void, s: *const c_void, _: u32, _: u32) -> i32 {
        assert_eq!(s as usize, 24);
        with_widget(p, |_| ());
        0
    }
    pub unsafe fn lvx_label_set_long_mode(p: *mut c_void, mode: u32) {
        assert_eq!(mode, 0);
        with_widget(p, |_| ());
    }
    pub unsafe fn lvx_label_set_text(p: *mut c_void, s: *const u8) {
        with_widget(p, |v| v.text = unsafe { string(s) });
    }
    pub unsafe fn lvx_align_to(p: *mut c_void, base: *mut c_void, align: u32, _: i32, y: i32) {
        let top = with_widget(base, |v| {
            if align == ALIGN_OUT_BOTTOM_MID {
                v.y + v.height
            } else {
                0
            }
        });
        with_widget(p, |v| v.y = top + y);
    }
    pub unsafe fn lvx_set_hidden(p: *mut c_void, hidden: u32) {
        with_widget(p, |v| v.hidden = hidden != 0);
    }
    pub unsafe fn lvx_list_row_create(
        p: *mut c_void,
        s: *const u8,
        d: *const u8,
        trailing: u8,
    ) -> *mut c_void {
        assert!(
            trailing == 0 || trailing == 3,
            "use the stock trailing ABI, not row-kind enums"
        );
        let object = widget(p as usize, unsafe { string(s) }, unsafe { string(d) });
        let hits =
            std::array::from_fn(|_| widget(object as usize, String::new(), String::new()) as usize);
        with_widget(object, |v| {
            v.hits = hits;
            v.trailing = trailing;
        });
        object
    }
    pub unsafe fn lvx_list_row_update(
        p: *mut c_void,
        _: *const u8,
        s: *const u8,
        d: *const u8,
        _: i32,
        _: u8,
    ) -> i32 {
        with_widget(p, |v| {
            v.text = unsafe { string(s) };
            v.detail = unsafe { string(d) };
        });
        0
    }
    pub unsafe fn lvx_list_row_trailing(p: *mut c_void) -> *mut c_void {
        with_widget(p, |v| v.hits[2] as *mut c_void)
    }
    pub unsafe fn row_hits(p: *mut c_void) -> [*mut c_void; 3] {
        with_widget(p, |v| v.hits.map(|i| i as *mut c_void))
    }
    pub unsafe fn lvx_event_add(p: *mut c_void, cb: Callback, _: u32, cookie: *mut c_void) {
        with_widget(p, |v| {
            assert!(v.callback.is_none());
            v.callback = Some(cb);
            v.cookie = cookie as usize;
        });
    }
    pub unsafe fn lvx_event_get_code(p: *mut c_void) -> u32 {
        unsafe { (*p.cast::<Event>()).code }
    }
    pub unsafe fn lvx_event_get_user_data(p: *mut c_void) -> usize {
        unsafe { (*p.cast::<Event>()).cookie }
    }
    pub unsafe fn lvx_timer_create(cb: Callback, ms: u32, _: *mut c_void) -> *mut c_void {
        assert_eq!(ms, 100);
        host(|h| {
            h.next += 1;
            let id = h.next;
            h.timers.insert(id, cb);
            id as *mut c_void
        })
    }
    pub unsafe fn lvx_timer_delete(p: *mut c_void) {
        assert!(host(|h| h.timers.remove(&(p as usize))).is_some());
    }
    pub unsafe fn activity_navigate(key: u32, _: u32, _: u32, _: u32) -> i32 {
        assert_eq!(key >> 16, 0xd5);
        if host(|h| std::mem::take(&mut h.fail_navigation)) {
            return -1;
        }
        let source = host(|h| {
            h.stack
                .last()
                .map(|&(p, _)| unsafe { (*(p as *const firmware_page_descriptor)).page_id as u32 })
                .unwrap_or(0)
        });
        host(|h| h.moves.push((source, key & 0xffff)));
        open((key & 0xffff) as usize);
        0
    }
    pub unsafe fn activity_finish(argument: *mut firmware_page_descriptor) -> i32 {
        if host(|h| std::mem::take(&mut h.fail_finish)) {
            return -1;
        }
        let p = host(|h| h.stack.last().unwrap().0 as *mut firmware_page_descriptor);
        let expected = if BAND_11 {
            (0xd5usize << 16) | unsafe { (*p).page_id as usize }
        } else {
            p as usize
        };
        assert_eq!(
            argument as usize, expected,
            "finish must use the live descriptor / Band 11 key"
        );
        pop();
        0
    }
}
struct Event {
    code: u32,
    cookie: usize,
}
pub mod runtime {
    use super::*;
    pub const APP_REGISTERED: u32 = 1;
    pub const APP_FAILED: u32 = 2;
    pub struct Runtime {
        pub app_state: AtomicU32,
        pub app_error: AtomicI32,
    }
    static R: Runtime = Runtime {
        app_state: AtomicU32::new(0),
        app_error: AtomicI32::new(0),
    };
    pub fn runtime() -> &'static Runtime {
        &R
    }
}
pub mod storage {
    pub fn ensure_launcher_icon() -> Result<(), i32> {
        Ok(())
    }
}
pub mod module {
    use super::*;
    pub fn register_native_pages(_: &[u8], _: &[u8], pages: &[*mut c_void]) -> Result<u16, i32> {
        assert_eq!(pages.len(), 5);
        for (i, &p) in pages.iter().enumerate() {
            unsafe {
                let d = &mut *p.cast::<firmware::firmware_page_descriptor>();
                assert_eq!(d.page_id as usize, i);
                d.app_id = 0xd5;
            }
        }
        Ok(0xd5)
    }
}
pub fn sync_resumed_page(i: usize) -> i32 {
    app(|a| a.resume_route(Route::from_page_index(i).unwrap()));
    0
}
pub fn view_revision(i: usize) -> Option<u32> {
    app(|a| (a.route.page_index() == i).then_some(a.generation))
}
pub fn page_view(i: usize) -> Option<loop_core::view::PageView> {
    app(|a| (a.route.page_index() == i).then(|| loop_core::view::present(a)))
}
pub fn ui_maintenance_tick() {
    host(|h| h.maintenance += 1);
    if let Some(s) = host(|h| h.pending.take()) {
        app(|a| {
            a.update(Action::Reload(s));
        });
    }
}
pub fn handle_action(i: usize, g: u32, action: Action) {
    assert_eq!(view_revision(i), Some(g));
    let (previous, effects) = app(|a| (a.navigation_state(), a.update(action)));
    for effect in effects {
        match effect {
            Effect::Navigate(r) => {
                if ui::navigate(r.page_index()) < 0 {
                    app(|a| a.restore_navigation(previous));
                    assert_eq!(ui::page_resume(i), 0);
                    return;
                }
            }
            Effect::ReloadFromDisk => {
                let s = host(|h| h.disk.clone()).unwrap_or_default();
                let n = s.courses.len();
                app(|a| {
                    a.update(Action::Reload(s));
                    a.update(Action::SetDataStatus(format!("已刷新，共 {n} 门课")));
                });
            }
            Effect::ClearStoredSchedule => {
                host(|h| {
                    h.clears += 1;
                    h.disk = None;
                });
                app(|a| {
                    a.update(Action::Reload(ScheduleFile::default()));
                    a.update(Action::SetDataStatus("已清空本地课表".into()));
                });
            }
        }
    }
}
pub fn handle_back(i: usize) {
    let previous = app(|a| {
        let previous = a.navigation_state();
        a.update(Action::Back);
        previous
    });
    if ui::back(i) < 0 {
        app(|a| a.restore_navigation(previous));
        assert_eq!(ui::page_resume(i), 0);
    }
}
fn lifecycle(p: usize, callback: *mut c_void) {
    if !callback.is_null() {
        let f: extern "C" fn(*mut firmware::firmware_page_descriptor) -> i32 =
            unsafe { std::mem::transmute(callback) };
        assert_eq!(f(p as *mut _), 0);
    }
}
fn pause_callback(p: usize) -> *mut c_void {
    let d = unsafe { &*(p as *const firmware::firmware_page_descriptor) };
    if firmware::BAND_11 {
        d.on_pause
    } else {
        d.on_destroy
    }
}
fn open(index: usize) {
    if let Some(p) = host(|h| h.stack.last().map(|x| x.0)) {
        lifecycle(p, pause_callback(p));
    }
    let descriptor = unsafe { *native_app::descriptor(index) };
    let p = Box::into_raw(Box::new(descriptor));
    assert_ne!(
        p,
        native_app::descriptor(index),
        "model the firmware's descriptor copy"
    );
    let root = widget(0, String::new(), String::new());
    host(|h| h.stack.push((p as usize, root as usize)));
    let create: extern "C" fn(
        *mut firmware::firmware_page_descriptor,
        *mut c_void,
        *mut c_void,
    ) -> i32 = unsafe { std::mem::transmute((*p).on_create) };
    assert_eq!(create(p, root, std::ptr::null_mut()), 0);
    lifecycle(p as usize, unsafe { (*p).on_resume });
}
fn pop() {
    let (p, root) = host(|h| h.stack.pop().unwrap());
    lifecycle(p, pause_callback(p));
    let d = unsafe { &*(p as *const firmware::firmware_page_descriptor) };
    lifecycle(
        p,
        if firmware::BAND_11 {
            d.on_ui_destroy
        } else {
            d.extension_callback_100
        },
    );
    host(|h| {
        h.finishes += 1;
        let mut owned = vec![root];
        loop {
            let n = owned.len();
            for (&id, w) in &h.widgets {
                if owned.contains(&w.parent) && !owned.contains(&id) {
                    owned.push(id);
                }
            }
            if n == owned.len() {
                break;
            }
        }
        for id in owned {
            h.widgets.remove(&id);
        }
    });
    unsafe { drop(Box::from_raw(p as *mut firmware::firmware_page_descriptor)) };
    if let Some(p) = host(|h| h.stack.last().map(|x| x.0)) {
        lifecycle(p, unsafe {
            (*(p as *const firmware::firmware_page_descriptor)).on_resume
        });
    }
}
fn on_current(h: &Host, id: usize) -> bool {
    let Some((_, root)) = h.stack.last() else {
        return false;
    };
    let mut p = id;
    while let Some(w) = h.widgets.get(&p) {
        if w.hidden {
            return false;
        }
        if p == *root {
            return true;
        }
        p = w.parent;
    }
    false
}
fn find(label: &str) -> usize {
    host(|h| {
        h.widgets
            .iter()
            .find(|&(id, w)| (w.text == label || w.detail == label) && on_current(h, *id))
            .map(|(&id, _)| id)
            .unwrap_or_else(|| panic!("missing {label}"))
    })
}
fn press(label: &str, child: Option<usize>) -> (Callback, usize) {
    let id = find(label);
    let (cb, cookie) = host(|h| {
        let id = child.map_or(id, |i| h.widgets[&id].hits[i]);
        let w = &h.widgets[&id];
        (w.callback.unwrap(), w.cookie)
    });
    send_event(cb, cookie, firmware::EVENT_PRESSED);
    (cb, cookie)
}
fn send_event(cb: Callback, cookie: usize, code: u32) {
    let mut e = Event { code, cookie };
    cb((&mut e as *mut Event).cast());
}
fn click_part(label: &str, child: Option<usize>) {
    let (cb, cookie) = press(label, child);
    send_event(cb, cookie, firmware::EVENT_CLICKED);
}
fn click(label: &str) {
    click_part(label, None);
}
fn title_back() {
    let id = host(|h| {
        let root = h.stack.last().unwrap().1;
        h.widgets
            .iter()
            .find(|(_, w)| w.parent == root && w.callback.is_some())
            .map(|(&i, _)| i)
            .unwrap()
    });
    let cb = host(|h| h.widgets[&id].callback.unwrap());
    cb(std::ptr::null_mut());
}
fn tick() {
    let timers = host(|h| h.timers.iter().map(|(&i, &cb)| (i, cb)).collect::<Vec<_>>());
    for (i, cb) in timers {
        cb(i as *mut c_void);
    }
}
fn fixture(count: u32) -> LoopApp {
    let mut a = LoopApp::default();
    a.schedule = ScheduleFile {
        version: 1,
        term: Term {
            name: "秋季学期".into(),
            start_date: "2026-08-31".into(),
        },
        courses: (1..=count)
            .map(|id| Course {
                id,
                name: format!("课程{id}"),
                location: "教学楼402".into(),
                weekday: 1,
                start_min: 480,
                end_min: 540,
                weeks_start: 1,
                weeks_end: 16,
                ..Course::default()
            })
            .collect(),
    };
    a.clock = Some(ClockHint {
        local_day: loop_core::schedule::term_start_day(&a.schedule.term).unwrap(),
        minute_of_day: 510,
        weekday: 1,
    });
    a.bump();
    a
}
#[test]
fn every_v1_button_native_stack_hints_imports_and_lifecycle() {
    app(|a| *a = fixture(60));
    host(|h| h.disk = Some(fixture(60).schedule));
    native_app::install_stage(1).unwrap();
    native_app::install_stage(2).unwrap();
    open(0);
    native_app::install_stage(1).unwrap();
    assert_eq!(unsafe { (*native_app::descriptor(0)).app_id }, 0xd5);
    assert_eq!(host(|h| h.timers.len()), 1);
    find("现在");
    find("下一节");
    find("进度");
    host(|h| h.fail_navigation = true);
    click("本周");
    assert_eq!(app(|a| a.route), Route::Home);
    assert!(app(|a| a.history.is_empty()));
    assert_eq!(host(|h| h.timers.len()), 1);
    let before = host(|h| h.maintenance);
    tick();
    assert!(host(|h| h.maintenance) > before);
    click_part("本周", Some(0));
    let summary = find("选择一天查看");
    let first = find("周一");
    host(|h| {
        assert!(
            h.widgets[&first].y >= h.widgets[&summary].height,
            "hint must occupy visible space above rows"
        )
    });
    for day in 1..=7 {
        click(loop_core::weekday_name(day));
        assert_eq!(app(|a| a.selected_weekday), day);
        if day == 1 {
            find("点选查看详情");
        } else {
            find("今天没课");
        }
        pop();
        assert_eq!(app(|a| a.route), Route::Week);
        find("选择一天查看");
    }
    assert!(
        host(|h| h.moves.contains(&(2, 1))),
        "Week to Today is a push even though its route number is smaller"
    );
    // Rejected SelectDay must also restore selection and the complete history.
    let previous_day = app(|a| a.selected_weekday);
    let previous_history = app(|a| a.history.clone());
    host(|h| h.fail_navigation = true);
    click("周一");
    assert_eq!(app(|a| a.route), Route::Week);
    assert_eq!(app(|a| a.selected_weekday), previous_day);
    assert_eq!(app(|a| a.history.clone()), previous_history);
    assert_eq!(host(|h| h.timers.len()), 1);
    click_part("周一", Some(1));
    let course_row = find("课程1");
    let body = host(|h| h.widgets[&course_row].parent);
    host(|h| h.widgets.get_mut(&body).unwrap().scroll = 400);
    assert_eq!(
        host(|h| h
            .widgets
            .iter()
            .filter(|(id, w)| w.text.starts_with("课程")
                && w.callback.is_some()
                && on_current(h, **id))
            .count()),
        60
    );
    host(|h| h.fail_navigation = true);
    click("课程60");
    assert_eq!(app(|a| a.route), Route::Today);
    assert_eq!(app(|a| a.selected_course_id), None);
    click_part("课程60", Some(2));
    assert_eq!(app(|a| a.selected_course_id), Some(60));
    find("详情");
    find("时间");
    find("星期");
    find("地点");
    find("周次");
    let previous_history = app(|a| a.history.clone());
    host(|h| h.fail_finish = true);
    title_back();
    assert_eq!(app(|a| a.route), Route::Detail);
    assert_eq!(app(|a| a.history.clone()), previous_history);
    assert_eq!(host(|h| h.timers.len()), 1);
    title_back();
    assert_eq!(app(|a| a.route), Route::Today);
    assert_eq!(
        host(|h| h.widgets[&body].scroll),
        400,
        "return must keep the source list's scroll position"
    );
    // Timer renders a different course into the pressed slot before CLICKED.
    let (cb, cookie) = press("课程1", Some(0));
    let mut replacement = fixture(3).schedule;
    replacement.courses[0].id = 101;
    replacement.courses[0].name = "替换后的课程".into();
    host(|h| h.pending = Some(replacement));
    tick();
    send_event(cb, cookie, firmware::EVENT_CLICKED);
    assert_eq!(app(|a| a.route), Route::Today);
    assert_eq!(app(|a| a.selected_course_id), Some(60));
    // A cancelled press and a duplicate/bubbled CLICKED must not navigate.
    let (cb, cookie) = press("替换后的课程", None);
    send_event(cb, cookie, firmware::EVENT_PRESS_LOST);
    send_event(cb, cookie, firmware::EVENT_CLICKED);
    assert_eq!(app(|a| a.route), Route::Today);
    click("替换后的课程");
    send_event(cb, cookie, firmware::EVENT_CLICKED);
    assert_eq!(host(|h| h.stack.len()), 4);
    title_back();
    host(|h| h.pending = Some(fixture(60).schedule));
    tick();
    // Import arrives after PRESS but before maintenance in CLICKED.
    host(|h| h.pending = Some(fixture(3).schedule));
    click("课程60");
    assert_eq!(app(|a| a.route), Route::Today);
    find("课程3");
    pop();
    title_back();
    assert_eq!(app(|a| a.route), Route::Home);
    click("今日课表");
    assert_eq!(app(|a| a.selected_weekday), 1);
    title_back();
    click("数据管理");
    click("刷新课表");
    find("已刷新，共 60 门课");
    click("清空本地课表");
    assert_eq!(host(|h| h.clears), 1);
    find("已清空本地课表");
    title_back();
    find("暂无课程");
    find("没有下一节");
    // Screen off pauses without destroying the root; wake restarts one timer.
    let p = host(|h| h.stack.last().unwrap().0);
    lifecycle(p, pause_callback(p));
    assert!(host(|h| h.timers.is_empty()));
    let maintenance = host(|h| h.maintenance);
    tick();
    assert_eq!(host(|h| h.maintenance), maintenance);
    lifecycle(p, unsafe {
        (*(p as *const firmware::firmware_page_descriptor)).on_resume
    });
    assert_eq!(host(|h| h.timers.len()), 1);
    let mut a = fixture(1);
    a.schedule.courses[0].name = "长课程名称".repeat(45);
    a.schedule.courses[0].location = "教学楼地址".repeat(45);
    host(|h| h.pending = Some(a.schedule));
    tick();
    click("今日课表");
    let name = app(|a| a.schedule.courses[0].name.clone());
    click(&name);
    find(&name);
    let location = app(|a| a.schedule.courses[0].location.clone());
    let id = find("地点");
    host(|h| assert_eq!(h.widgets[&id].detail, location));
    while host(|h| !h.stack.is_empty()) {
        pop();
    }
    assert!(host(|h| h.timers.is_empty()));
    assert!(host(|h| h.widgets.is_empty()));
    open(0);
    find("Loop");
    pop();
}
