//! 墙钟 → `ClockHint`（Asia/Shanghai，无夏令时，UTC+8）。

use canopus_target_private::{canopus_fw_clock_gettime, stock_timespec_t};
use loop_core::ClockHint;

/// NuttX / 固件：`clock_id == 0` 为 CLOCK_REALTIME。
const CLOCK_REALTIME: u32 = 0;
const SHANGHAI_OFFSET_SECS: i64 = 8 * 3600;

pub fn read_clock() -> Option<ClockHint> {
    let mut time = stock_timespec_t {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let result = unsafe { canopus_fw_clock_gettime(CLOCK_REALTIME, core::ptr::addr_of_mut!(time)) };
    if result != 0 || time.tv_sec < 0 || time.tv_nsec < 0 || time.tv_nsec >= 1_000_000_000 {
        return None;
    }
    let local = (time.tv_sec as i64).saturating_add(SHANGHAI_OFFSET_SECS);
    if local < 0 {
        return None;
    }
    let local_day = (local / 86_400) as i32;
    let sod = (local % 86_400) as u32;
    let minute_of_day = (sod / 60) as u16;
    // Unix day 0 = 1970-01-01 周四 = ISO 4；iso = ((day + 3) % 7) + 1。
    let weekday = (((local_day + 3).rem_euclid(7)) + 1) as u8;
    Some(ClockHint {
        local_day,
        minute_of_day,
        weekday,
    })
}
