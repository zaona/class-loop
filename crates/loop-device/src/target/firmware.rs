//! Existing exact-target SDK functions; prefab hit targets match Manager's ABI.
pub use canopus_target_private::*;
use core::ffi::c_void;
// Existing LVGL event codes on the five supported targets.
pub const EVENT_PRESSED: u32 = 1;
pub const EVENT_PRESS_LOST: u32 = 3;
pub const BAND_11: bool = cfg!(any(
    feature = "target-xiaomi-band-11-4-100-139",
    feature = "target-xiaomi-band-11-4-100-155",
    feature = "target-xiaomi-band-11-4-100-179"
));
pub unsafe fn row_hits(row: *mut c_void) -> [*mut c_void; 3] {
    let mut hits = [core::ptr::null_mut(); 3];
    hits[2] = unsafe { lvx_list_row_trailing(row) };
    if BAND_11 {
        // manager/target/band11/canopus_band11_abi.h, same three exact targets.
        // Only read live prefab child handles; no private object-field writes.
        hits[0] = unsafe { row.cast::<u8>().add(0x3c).cast::<*mut c_void>().read() };
        hits[1] = unsafe { row.cast::<u8>().add(0x40).cast::<*mut c_void>().read() };
    }
    hits
}
