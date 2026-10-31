//! 墙钟 → `ClockHint`（Asia/Shanghai，无夏令时，UTC+8）。

use loop_core::ClockHint;

const SHANGHAI_OFFSET_SECS: i64 = 8 * 3600;

#[cfg(feature = "device")]
pub fn read_clock() -> Option<ClockHint> {
    use canopus_target_private::{canopus_fw_clock_gettime, stock_timespec_t};
    // ds-newdevice-supports currently aliases ctx.time.realtime_ms to uptime.
    // Read the target SDK's approved NuttX CLOCK_REALTIME entry instead.
    const CLOCK_REALTIME: u32 = 0;
    let mut time = stock_timespec_t {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let result = unsafe { canopus_fw_clock_gettime(CLOCK_REALTIME, core::ptr::addr_of_mut!(time)) };
    if result != 0 || time.tv_nsec < 0 || time.tv_nsec >= 1_000_000_000 {
        return None;
    }
    from_unix_seconds(time.tv_sec)
}

pub fn from_unix_seconds(seconds: i64) -> Option<ClockHint> {
    let local = seconds.checked_add(SHANGHAI_OFFSET_SECS)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use loop_core::{Action, Course, LoopApp, Route, ScheduleFile, Term, view};

    #[test]
    fn shanghai_midnight_and_imported_week_use_calendar_date() {
        // 2026-10-31 16:00 UTC = 2026-11-01 00:00 Asia/Shanghai.
        let midnight = from_unix_seconds(1_793_462_400).unwrap();
        assert_eq!(midnight.weekday, 7);
        assert_eq!(midnight.minute_of_day, 0);
        let before = from_unix_seconds(1_793_462_399).unwrap();
        assert_eq!(before.weekday, 6);
        assert_eq!(before.minute_of_day, 1439);
        assert_eq!(midnight.local_day, before.local_day + 1);

        let mut app = LoopApp::default();
        app.update(Action::Boot(ScheduleFile {
            version: 1,
            term: Term {
                name: "2026秋".into(),
                start_date: "2026-08-31".into(),
            },
            courses: alloc::vec![Course {
                id: 1,
                name: "测试课程".into(),
                weekday: 1,
                start_min: 480,
                end_min: 540,
                weeks_start: 1,
                weeks_end: 16,
                ..Course::default()
            }],
        }));
        // Previous adapter interpreted this uptime as an epoch timestamp.
        app.update(Action::Tick(from_unix_seconds(946).unwrap()));
        assert_eq!(app.week(), 0);
        assert!(app.day_courses(1).is_empty());
        app.update(Action::Tick(midnight));
        assert_eq!(app.week(), 9);
        app.update(Action::SelectDay(1));
        let page = view::present(&app);
        assert_eq!(page.cards[0].title, "测试课程");
        assert_eq!(page.cards[0].target, Some(view::Target::Course(1)));
        app.update(Action::Open(Route::Week));
        assert!(view::present(&app).summary.contains("第9周"));
    }
}
