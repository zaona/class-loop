//! Native page ownership like Lyra; Launcher identity remains Supervisor-assigned.
use super::{firmware::*, runtime::*, storage, ui};
use core::{
    ffi::c_void,
    mem::MaybeUninit,
    sync::atomic::{AtomicU32, Ordering},
};
pub const PAGE_COUNT: usize = 5;
static APP_ID: AtomicU32 = AtomicU32::new(0);
const NAMES: [&[u8]; PAGE_COUNT] = [
    b"loop_home\0",
    b"loop_today\0",
    b"loop_week\0",
    b"loop_detail\0",
    b"loop_data\0",
];
static mut DESCRIPTORS: MaybeUninit<[firmware_page_descriptor; PAGE_COUNT]> = MaybeUninit::uninit();
static mut LIVE: [*mut firmware_page_descriptor; PAGE_COUNT] = [core::ptr::null_mut(); PAGE_COUNT];
pub fn app_id() -> u32 {
    APP_ID.load(Ordering::Acquire)
}
pub fn descriptor(index: usize) -> *mut firmware_page_descriptor {
    unsafe {
        core::ptr::addr_of_mut!(DESCRIPTORS)
            .cast::<firmware_page_descriptor>()
            .add(index)
    }
}
pub fn live_page(index: usize) -> *mut firmware_page_descriptor {
    unsafe {
        core::ptr::addr_of!(LIVE)
            .cast::<*mut firmware_page_descriptor>()
            .add(index)
            .read()
    }
}
fn remember(index: usize, page: *mut firmware_page_descriptor) {
    unsafe {
        core::ptr::addr_of_mut!(LIVE)
            .cast::<*mut firmware_page_descriptor>()
            .add(index)
            .write(page)
    };
}
pub fn install_stage(stage: u32) -> Result<(), i32> {
    match stage {
        1 => {
            if app_id() != 0 {
                return Ok(());
            }
            for (index, name) in NAMES.iter().enumerate() {
                let p = descriptor(index);
                unsafe {
                    p.write(core::mem::zeroed());
                    (*p).page_name = name.as_ptr() as *mut c_void;
                    (*p).page_id = index as u16;
                    (*p).on_signal = on_signal as *const () as *mut c_void;
                    (*p).on_create = on_create as *const () as *mut c_void;
                    (*p).on_resume = on_resume as *const () as *mut c_void;
                    #[cfg(not(any(
                        feature = "target-xiaomi-band-11-4-100-139",
                        feature = "target-xiaomi-band-11-4-100-155",
                        feature = "target-xiaomi-band-11-4-100-179"
                    )))]
                    {
                        // Generated names are stale: +0x5c is PAUSE, +0x64 UI_DESTROY.
                        // Same mapping as Lyra; do not destroy the page on screen-off.
                        (*p).on_destroy = on_pause as *const () as *mut c_void;
                        (*p).extension_callback_100 = on_destroy as *const () as *mut c_void;
                    }
                    #[cfg(any(
                        feature = "target-xiaomi-band-11-4-100-139",
                        feature = "target-xiaomi-band-11-4-100-155",
                        feature = "target-xiaomi-band-11-4-100-179"
                    ))]
                    {
                        (*p).on_pause = on_pause as *const () as *mut c_void;
                        (*p).on_ui_destroy = on_destroy as *const () as *mut c_void;
                    }
                }
            }
            Ok(())
        }
        2 => {
            if app_id() != 0 {
                return Ok(());
            }
            storage::ensure_launcher_icon()?;
            let pages =
                core::array::from_fn::<_, PAGE_COUNT, _>(|i| descriptor(i).cast::<c_void>());
            match crate::module::register_native_pages(
                b"Loop\0",
                b"/data/canopus/m.canopus_loop/appicon_loop.bin\0",
                &pages,
            ) {
                Ok(id) => {
                    APP_ID.store(id as u32, Ordering::Release);
                    runtime().app_state.store(APP_REGISTERED, Ordering::Release);
                    runtime().app_error.store(0, Ordering::Release);
                    Ok(())
                }
                Err(e) => {
                    runtime().app_state.store(APP_FAILED, Ordering::Release);
                    runtime().app_error.store(e, Ordering::Release);
                    Err(e)
                }
            }
        }
        _ => Err(-103),
    }
}
fn index(page: *mut firmware_page_descriptor) -> Option<usize> {
    if page.is_null() {
        return None;
    }
    let i = unsafe { (*page).page_id as usize };
    (i < PAGE_COUNT).then_some(i)
}
extern "C" fn on_signal(_: *mut firmware_page_descriptor, _: u32, _: *mut c_void) -> i32 {
    0
}
extern "C" fn on_create(
    page: *mut firmware_page_descriptor,
    root: *mut c_void,
    _: *mut c_void,
) -> i32 {
    let Some(i) = index(page) else {
        return -1;
    };
    remember(i, page);
    ui::page_create(i, root)
}
extern "C" fn on_resume(page: *mut firmware_page_descriptor) -> i32 {
    let Some(i) = index(page) else {
        return -1;
    };
    remember(i, page);
    ui::page_resume(i)
}
extern "C" fn on_pause(page: *mut firmware_page_descriptor) -> i32 {
    index(page).map_or(-1, ui::page_pause)
}
extern "C" fn on_destroy(page: *mut firmware_page_descriptor) -> i32 {
    let Some(i) = index(page) else {
        return -1;
    };
    remember(i, core::ptr::null_mut());
    ui::page_destroy(i)
}
