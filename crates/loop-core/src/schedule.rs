//! 周次与「现在 / 下一节」推算。时间以本地日历日 + 当天分钟数为准。

use alloc::vec::Vec;

use crate::model::{Course, Term};

/// 设备侧传入的时钟快照（由 `clock_gettime` 换算而来）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockHint {
    /// 自 Unix epoch 起的本地日历日序号（按 Asia/Shanghai 墙钟日期）。
    pub local_day: i32,
    /// 当天 0 点起的分钟数 `[0, 1439]`。
    pub minute_of_day: u16,
    /// ISO 风格：1=周一 … 7=周日。
    pub weekday: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowNext<'a> {
    pub now: Option<&'a Course>,
    pub next: Option<&'a Course>,
    pub remaining_today: usize,
}

/// 解析 `YYYY-MM-DD` 为自公元日起的序数日（简化格里高利算法）。
pub fn parse_ymd(date: &str) -> Option<(i32, u8, u8)> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = parse_u16(&bytes[0..4])? as i32;
    let month = parse_u16(&bytes[5..7])? as u8;
    let day = parse_u16(&bytes[8..10])? as u8;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

fn parse_u16(digits: &[u8]) -> Option<u16> {
    let mut value = 0u16;
    for &b in digits {
        if !b.is_ascii_digit() {
            return None;
        }
        value = value.saturating_mul(10).saturating_add((b - b'0') as u16);
    }
    Some(value)
}

/// 格里高利历日期 → 儒略日（正午无关，仅做日差）。
pub fn civil_to_days(year: i32, month: u8, day: u8) -> i32 {
    let y = if month <= 2 { year - 1 } else { year };
    let m = if month <= 2 {
        month as i32 + 12
    } else {
        month as i32
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn term_start_day(term: &Term) -> Option<i32> {
    let (y, m, d) = parse_ymd(&term.start_date)?;
    Some(civil_to_days(y, m, d))
}

/// 教学周：学期起始日所在周为第 1 周；早于起始日返回 0。
pub fn term_week(term: &Term, local_day: i32) -> u8 {
    let Some(start) = term_start_day(term) else {
        return 0;
    };
    if local_day < start {
        return 0;
    }
    let week = ((local_day - start) / 7) + 1;
    if week > 255 { 255 } else { week as u8 }
}

/// 某教学周 + 星期几当天的课程，按开始时间排序。
pub fn courses_on_day<'a>(courses: &'a [Course], week: u8, weekday: u8) -> Vec<&'a Course> {
    let mut list: Vec<&Course> = courses
        .iter()
        .filter(|c| c.weekday == weekday && c.active_in_week(week))
        .collect();
    list.sort_by_key(|c| c.start_min);
    list
}

/// 根据当前时刻给出现在 / 下一节 / 今日剩余节数。
pub fn now_and_next<'a>(courses: &'a [Course], term: &Term, clock: ClockHint) -> NowNext<'a> {
    let week = term_week(term, clock.local_day);
    let today = courses_on_day(courses, week, clock.weekday);
    let mut now = None;
    let mut next = None;
    let mut remaining = 0usize;
    for course in &today {
        if clock.minute_of_day < course.end_min {
            remaining += 1;
        }
        if clock.minute_of_day >= course.start_min && clock.minute_of_day < course.end_min {
            now = Some(*course);
        } else if clock.minute_of_day < course.start_min && next.is_none() {
            next = Some(*course);
        }
    }
    // 若正在上课，「下一节」仍指向之后的第一节。
    if now.is_some() {
        next = today
            .iter()
            .copied()
            .find(|c| c.start_min >= now.unwrap().end_min);
    }
    NowNext {
        now,
        next,
        remaining_today: remaining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Course, Term};
    use alloc::string::String;
    use alloc::vec;

    fn sample_term() -> Term {
        Term {
            name: String::from("2026秋"),
            start_date: String::from("2026-08-31"),
        }
    }

    fn course(id: u32, weekday: u8, start: u16, end: u16, ws: u8, we: u8) -> Course {
        Course {
            id,
            name: alloc::format!("课{id}"),
            location: String::from("A1"),
            weekday,
            start_min: start,
            end_min: end,
            period_label: String::from("第1-2节"),
            weeks_start: ws,
            weeks_end: we,
        }
    }

    #[test]
    fn week_one_starts_on_term_date() {
        let term = sample_term();
        let start = term_start_day(&term).unwrap();
        assert_eq!(term_week(&term, start), 1);
        assert_eq!(term_week(&term, start + 7), 2);
        assert_eq!(term_week(&term, start - 1), 0);
    }

    #[test]
    fn now_and_next_during_class() {
        let term = sample_term();
        let start = term_start_day(&term).unwrap();
        // 2026-08-31 是周一（weekday=1）。
        let courses = vec![course(1, 1, 470, 560, 1, 16), course(2, 1, 575, 670, 1, 16)];
        let clock = ClockHint {
            local_day: start,
            minute_of_day: 500,
            weekday: 1,
        };
        let result = now_and_next(&courses, &term, clock);
        assert_eq!(result.now.map(|c| c.id), Some(1));
        assert_eq!(result.next.map(|c| c.id), Some(2));
        assert_eq!(result.remaining_today, 2);
    }

    #[test]
    fn skips_inactive_weeks() {
        let term = sample_term();
        let start = term_start_day(&term).unwrap();
        let courses = vec![course(1, 1, 470, 560, 10, 12)];
        let clock = ClockHint {
            local_day: start,
            minute_of_day: 480,
            weekday: 1,
        };
        let result = now_and_next(&courses, &term, clock);
        assert!(result.now.is_none());
        assert!(result.next.is_none());
    }
}
