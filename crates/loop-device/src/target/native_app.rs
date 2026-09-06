//! 原生应用注册：固定 8-bit app id、Launcher 入口与五页描述符。
//!
//! app 注册（stage 1）与 Launcher 发布（stage 2）刻意拆开，
//! 以便 miwear 先处理 app-registry 事件再持久化 Launcher。

use core::sync::atomic::Ordering;

use canopus_target_private::*;

use super::runtime::*;
use super::ui_backend;

/// 与 Lyra `0x00CC` 错开，避免 app 冲突。
pub const APP_ID: u16 = 0x00CD;
pub const PAGE_COUNT: usize = 5;
pub const PAGE_HOME: usize = 0;

pub const PACKAGE_NAME: &[u8] = b"com.canopus.loop\0";
pub const DISPLAY_NAME: &[u8] = b"Loop\0";
pub const LAUNCHER_ICON: &[u8] = b"/data/canopus/appicon_loop.bin\0";
const PAGE_NAMES: [&[u8]; PAGE_COUNT] = [
    b"loop_home\0",
    b"loop_today\0",
    b"loop_week\0",
    b"loop_detail\0",
    b"loop_data\0",
];

static mut APP_DESCRIPTOR: core::mem::MaybeUninit<launcher_app_descriptor> =
    core::mem::MaybeUninit::uninit();
static mut PAGE_DESCRIPTORS: core::mem::MaybeUninit<[firmware_page_descriptor; PAGE_COUNT]> =
    core::mem::MaybeUninit::uninit();

pub fn page_descriptor_ptr(index: usize) -> *mut firmware_page_descriptor {
    unsafe {
        core::ptr::addr_of_mut!(PAGE_DESCRIPTORS)
            .cast::<firmware_page_descriptor>()
            .add(index)
    }
}

extern "C" fn launcher_display_name() -> *const u8 {
    DISPLAY_NAME.as_ptr()
}

fn c_str_equal(a: *const u8, expected: &[u8]) -> bool {
    if a.is_null() || expected.last() != Some(&0) {
        return false;
    }
    let mut i = 0usize;
    while i < expected.len() {
        if unsafe { *a.add(i) } != expected[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn app_descriptor_init() {
    unsafe {
        core::ptr::write_bytes(
            core::ptr::addr_of_mut!(APP_DESCRIPTOR).cast::<u8>(),
            0,
            core::mem::size_of::<launcher_app_descriptor>(),
        );
        let app = &mut *core::ptr::addr_of_mut!(APP_DESCRIPTOR).cast::<launcher_app_descriptor>();
        app.package_name = PACKAGE_NAME.as_ptr() as *mut core::ffi::c_void;
        app.launcher_icon_resource = LAUNCHER_ICON.as_ptr() as *mut core::ffi::c_void;
        app.app_id = APP_ID;
        app.launcher_metadata_callback =
            launcher_display_name as *const () as *mut core::ffi::c_void;
    }
}

fn descriptor_init(index: usize, name: &[u8], page_id: u16) {
    let descriptor = page_descriptor_ptr(index);
    unsafe {
        core::ptr::write_bytes(
            descriptor.cast::<u8>(),
            0,
            core::mem::size_of::<firmware_page_descriptor>(),
        );
        (*descriptor).page_name = name.as_ptr() as *mut core::ffi::c_void;
        (*descriptor).page_id = page_id;
        (*descriptor).app_id = APP_ID;
        (*descriptor).on_signal = page_on_signal as *const () as *mut core::ffi::c_void;
        (*descriptor).on_create = page_on_create as *const () as *mut core::ffi::c_void;
        (*descriptor).on_resume = page_on_resume as *const () as *mut core::ffi::c_void;
        (*descriptor).on_pause = page_on_pause as *const () as *mut core::ffi::c_void;
        (*descriptor).on_destroy = page_on_destroy as *const () as *mut core::ffi::c_void;
    }
}

pub fn install_stage(stage: u32) -> Result<(), i32> {
    let r = runtime();
    let existing = unsafe { app_lookup(APP_ID) };

    if stage == 1 {
        if !existing.is_null() {
            let package: *const u8 =
                unsafe { core::ptr::read(existing.cast::<u8>().add(8) as *const *const u8) };
            if !c_str_equal(package, PACKAGE_NAME) {
                r.app_error.store(-101, Ordering::Release);
                r.app_state.store(APP_FAILED, Ordering::Release);
                return Err(-101);
            }
            r.app_state.store(APP_REGISTERED, Ordering::Release);
            r.app_error.store(0, Ordering::Release);
            return Ok(());
        }

        app_descriptor_init();
        for (index, name) in PAGE_NAMES.iter().enumerate() {
            descriptor_init(index, name, index as u16);
        }

        let pages: [*mut firmware_page_descriptor; PAGE_COUNT] =
            core::array::from_fn(page_descriptor_ptr);
        let install_result = unsafe {
            app_install(
                core::ptr::addr_of_mut!(APP_DESCRIPTOR).cast::<launcher_app_descriptor>(),
                pages.as_ptr(),
                PAGE_COUNT as u32,
            )
        };
        r.app_install_result
            .store(install_result, Ordering::Release);
        let installed = unsafe { app_lookup(APP_ID) };
        if installed.is_null() {
            r.app_error.store(-100, Ordering::Release);
            r.app_state.store(APP_FAILED, Ordering::Release);
            return Err(-100);
        }
        let package: *const u8 =
            unsafe { core::ptr::read(installed.cast::<u8>().add(8) as *const *const u8) };
        if !c_str_equal(package, PACKAGE_NAME) {
            r.app_error.store(-101, Ordering::Release);
            r.app_state.store(APP_FAILED, Ordering::Release);
            return Err(-101);
        }
        r.app_state.store(APP_REGISTERED, Ordering::Release);
        r.app_error.store(0, Ordering::Release);
        return Ok(());
    }

    if stage == 2 {
        if existing.is_null() {
            r.app_error.store(-102, Ordering::Release);
            r.app_state.store(APP_FAILED, Ordering::Release);
            return Err(-102);
        }
        let package: *const u8 =
            unsafe { core::ptr::read(existing.cast::<u8>().add(8) as *const *const u8) };
        if !c_str_equal(package, PACKAGE_NAME) {
            r.app_error.store(-101, Ordering::Release);
            r.app_state.store(APP_FAILED, Ordering::Release);
            return Err(-101);
        }
        if r.app_state.load(Ordering::Acquire) == APP_OK {
            return Ok(());
        }
        let launcher_result = unsafe { launcher_add(APP_ID) };
        r.launcher_add_result
            .store(launcher_result, Ordering::Release);
        r.app_state.store(APP_OK, Ordering::Release);
        r.app_error.store(0, Ordering::Release);
        return Ok(());
    }

    Err(-103)
}

fn page_id_of(page: *mut firmware_page_descriptor) -> usize {
    if page.is_null() {
        return usize::MAX;
    }
    usize::from(unsafe { (*page).page_id })
}

extern "C" fn page_on_signal(
    _page: *mut firmware_page_descriptor,
    _event: u32,
    _payload: *mut core::ffi::c_void,
) -> i32 {
    0
}

extern "C" fn page_on_create(
    page: *mut firmware_page_descriptor,
    root: *mut core::ffi::c_void,
    _start_data: *mut core::ffi::c_void,
) -> i32 {
    let index = page_id_of(page);
    if index >= PAGE_COUNT {
        return -1;
    }
    runtime().active_page.store(index as u32, Ordering::Release);
    ui_backend::page_create(index, root)
}

extern "C" fn page_on_resume(page: *mut firmware_page_descriptor) -> i32 {
    let index = page_id_of(page);
    if index >= PAGE_COUNT {
        return -1;
    }
    runtime().active_page.store(index as u32, Ordering::Release);
    ui_backend::page_resume(index)
}

extern "C" fn page_on_pause(page: *mut firmware_page_descriptor) -> i32 {
    let index = page_id_of(page);
    if index >= PAGE_COUNT {
        return -1;
    }
    ui_backend::page_pause(index)
}

extern "C" fn page_on_destroy(page: *mut firmware_page_descriptor) -> i32 {
    let index = page_id_of(page);
    if index >= PAGE_COUNT {
        return -1;
    }
    ui_backend::page_destroy(index)
}
