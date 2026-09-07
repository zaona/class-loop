//! 公历日期 / 本地墙钟时间（无 chrono）。

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CivilDateTime {
    pub date: CivilDate,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl CivilDate {
    pub fn from_ymd(y: i32, m: u32, d: u32) -> Result<Self, String> {
        ordinal(y, m, d)?;
        Ok(Self { y, m, d })
    }

    pub fn parse_ymd(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.len() < 10 {
            return Err(format!("invalid YYYY-MM-DD: {value}"));
        }
        let y: i32 = value[0..4]
            .parse()
            .map_err(|_| format!("invalid year in {value}"))?;
        let m: u32 = value[5..7]
            .parse()
            .map_err(|_| format!("invalid month in {value}"))?;
        let d: u32 = value[8..10]
            .parse()
            .map_err(|_| format!("invalid day in {value}"))?;
        Self::from_ymd(y, m, d)
    }

    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }

    /// ISO：1=周一 … 7=周日。
    pub fn weekday_iso(self) -> u8 {
        let mut y = self.y;
        let m = self.m as i32;
        let d = self.d as i32;
        let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        if m < 3 {
            y -= 1;
        }
        let w = (y + y / 4 - y / 100 + y / 400 + t[(m as usize) - 1] + d) % 7;
        if w == 0 { 7 } else { w as u8 }
    }

    pub fn add_days(self, days: i64) -> Result<Self, String> {
        let mut ord = ordinal(self.y, self.m, self.d)? as i64 + days;
        let mut y = self.y;
        loop {
            let diy = days_in_year(y) as i64;
            if ord > diy {
                ord -= diy;
                y += 1;
                continue;
            }
            if ord <= 0 {
                y -= 1;
                ord += days_in_year(y) as i64;
                continue;
            }
            return date_from_ordinal(y, ord as u32);
        }
    }
}

impl CivilDateTime {
    pub fn minutes_of_day(self) -> u32 {
        self.hour * 60 + self.minute
    }

    pub fn cmp_key(self) -> (CivilDate, u32, u32, u32) {
        (self.date, self.hour, self.minute, self.second)
    }
}

pub fn monday_of_week(day: CivilDate) -> Result<CivilDate, String> {
    let wd = day.weekday_iso();
    day.add_days(-((wd as i64) - 1))
}

pub fn week_index(term_start: CivilDate, day: CivilDate) -> Result<i32, String> {
    let delta = days_between(term_start, day)?;
    if delta < 0 {
        return Ok(0);
    }
    Ok((delta / 7) as i32 + 1)
}

pub fn days_between(a: CivilDate, b: CivilDate) -> Result<i64, String> {
    Ok(gregorian_day(b)? - gregorian_day(a)?)
}

fn gregorian_day(d: CivilDate) -> Result<i64, String> {
    let y = d.y as i64;
    let m = d.m as i64;
    let day = d.d as i64;
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146097 + doe - 719468)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_year(y: i32) -> u32 {
    if is_leap(y) { 366 } else { 365 }
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn ordinal(y: i32, m: u32, d: u32) -> Result<u32, String> {
    if m == 0 || m > 12 || d == 0 || d > days_in_month(y, m) {
        return Err(format!("invalid date {y:04}-{m:02}-{d:02}"));
    }
    static CUM: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut n = CUM[(m as usize) - 1] + d;
    if m > 2 && is_leap(y) {
        n += 1;
    }
    Ok(n)
}

fn date_from_ordinal(y: i32, mut ord: u32) -> Result<CivilDate, String> {
    if ord == 0 || ord > days_in_year(y) {
        return Err(format!("ordinal {ord} out of range for {y}"));
    }
    for m in 1..=12 {
        let dim = days_in_month(y, m);
        if ord <= dim {
            return CivilDate::from_ymd(y, m, ord);
        }
        ord -= dim;
    }
    Err("unreachable".into())
}

pub fn shift_hours(mut dt: CivilDateTime, hours: i32) -> Result<CivilDateTime, String> {
    let mut total = dt.hour as i32 + hours;
    let mut day_delta = 0i64;
    while total >= 24 {
        total -= 24;
        day_delta += 1;
    }
    while total < 0 {
        total += 24;
        day_delta -= 1;
    }
    dt.hour = total as u32;
    if day_delta != 0 {
        dt.date = dt.date.add_days(day_delta)?;
    }
    Ok(dt)
}

pub fn add_minutes(dt: CivilDateTime, minutes: i64) -> Result<CivilDateTime, String> {
    let total = dt.hour as i64 * 60 + dt.minute as i64 + minutes;
    let day_delta = total.div_euclid(24 * 60);
    let tod = total.rem_euclid(24 * 60) as u32;
    Ok(CivilDateTime {
        date: dt.date.add_days(day_delta)?,
        hour: tod / 60,
        minute: tod % 60,
        second: dt.second,
    })
}
