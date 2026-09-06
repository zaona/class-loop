//! ICS → Loop schedule.json（与 `scripts/ics-to-schedule.py` 同语义子集）。

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::{Value, json};

const DEFAULT_UTC_OFFSET_HOURS: i32 = 8;

#[derive(Clone, Debug)]
struct Event {
    summary: String,
    location: String,
    description: String,
    /// Local wall-clock minutes since Unix epoch day start is not used;
    /// we keep civil date + time-of-day.
    start: CivilDateTime,
    end: CivilDateTime,
    until: Option<CivilDate>,
    count: Option<u32>,
    interval: u32,
    byday: Vec<u8>,
    exdates: HashSet<CivilDate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CivilDate {
    y: i32,
    m: u32,
    d: u32,
}

#[derive(Clone, Copy, Debug)]
struct CivilDateTime {
    date: CivilDate,
    hour: u32,
    minute: u32,
    second: u32,
}

impl CivilDate {
    fn from_ymd(y: i32, m: u32, d: u32) -> Result<Self, String> {
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return Err(format!("invalid date {y:04}-{m:02}-{d:02}"));
        }
        // Lightweight validation via ordinal.
        ordinal(y, m, d)?;
        Ok(Self { y, m, d })
    }

    fn parse_ymd(value: &str) -> Result<Self, String> {
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

    fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }

    fn weekday_iso(self) -> u8 {
        // Sakamoto: 0=Sun .. 6=Sat → ISO 1=Mon .. 7=Sun
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

    fn add_days(self, days: i64) -> Result<Self, String> {
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
    fn minutes_of_day(self) -> u32 {
        self.hour * 60 + self.minute
    }
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

fn days_between(a: CivilDate, b: CivilDate) -> Result<i64, String> {
    // Convert to proleptic Gregorian day number.
    Ok(gregorian_day(b)? - gregorian_day(a)?)
}

fn gregorian_day(d: CivilDate) -> Result<i64, String> {
    let y = d.y as i64;
    let m = d.m as i64;
    let day = d.d as i64;
    // Howard Hinnant civil_from_days inverse.
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146097 + doe - 719468)
}

/// 将 ICS 文本转为 schedule.json 对象。
pub fn convert_ics_text(
    text: &str,
    term_name: Option<&str>,
    term_start: Option<&str>,
    utc_offset_hours: i32,
) -> Result<Value, String> {
    let events = parse_ics(text, utc_offset_hours)?;
    if events.is_empty() {
        return Err("ICS 中没有可用的 VEVENT".into());
    }
    let term_start_date = if let Some(raw) = term_start {
        let raw = raw.trim();
        if raw.is_empty() {
            min_event_date(&events)?
        } else {
            CivilDate::parse_ymd(raw)?
        }
    } else {
        min_event_date(&events)?
    };
    let term_name = match term_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => name.to_string(),
        None => {
            let season = if term_start_date.m >= 8 { "秋" } else { "春" };
            format!("{}{season}", term_start_date.y)
        }
    };
    convert_events(&events, &term_name, term_start_date)
}

pub fn convert_ics_file(
    path: &str,
    term_name: Option<&str>,
    term_start: Option<&str>,
) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read ICS: {e}"))?;
    if text.len() > 2 * 1024 * 1024 {
        return Err("ICS file exceeds 2 MiB".into());
    }
    convert_ics_text(&text, term_name, term_start, DEFAULT_UTC_OFFSET_HOURS)
}

fn min_event_date(events: &[Event]) -> Result<CivilDate, String> {
    events
        .iter()
        .map(|e| e.start.date)
        .min()
        .ok_or_else(|| "empty events".into())
}

fn convert_events(
    events: &[Event],
    term_name: &str,
    term_start: CivilDate,
) -> Result<Value, String> {
    let mut courses = Vec::new();
    let mut next_id = 1u64;
    let mut skipped = 0u32;
    for event in events {
        let label = period_label(&event.description, &event.summary);
        let mut emitted = false;
        for weekday in event_weekdays(event) {
            let weeks = occurrence_weeks(event, term_start, weekday)?;
            if weeks.is_empty() {
                continue;
            }
            emitted = true;
            for (weeks_start, weeks_end, interval) in compress_weeks(&weeks, event.interval) {
                let mut course = json!({
                    "id": next_id,
                    "name": event.summary,
                    "location": event.location,
                    "weekday": weekday,
                    "start_min": event.start.minutes_of_day(),
                    "end_min": event.end.minutes_of_day(),
                    "period_label": label,
                    "weeks_start": weeks_start,
                    "weeks_end": weeks_end,
                });
                if interval != 1 {
                    course["week_interval"] = json!(interval);
                }
                courses.push(course);
                next_id += 1;
            }
        }
        if !emitted {
            skipped += 1;
        }
    }
    if courses.is_empty() {
        return Err("转换结果为空（可能全被 EXDATE / 学期过滤）".into());
    }
    if skipped > 0 {
        tracing::warn!(skipped, "skipped events with no weeks");
    }
    Ok(json!({
        "version": 1,
        "term": {
            "name": term_name,
            "start_date": term_start.iso(),
        },
        "courses": courses,
    }))
}

fn unfold(text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\n' {
            if matches!(chars.peek(), Some(' ' | '\t')) {
                chars.next();
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(',') => out.push(','),
                Some(';') => out.push(';'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_nested_components(body: &str) -> String {
    let mut out = Vec::new();
    let mut depth = 0i32;
    for line in body.split('\n') {
        if line.starts_with("BEGIN:") {
            depth += 1;
            continue;
        }
        if line.starts_with("END:") {
            depth = (depth - 1).max(0);
            continue;
        }
        if depth == 0 {
            out.push(line);
        }
    }
    out.join("\n")
}

fn parse_prop(line: &str) -> Option<(String, HashMap<String, String>, String)> {
    let (meta, value) = line.split_once(':')?;
    let mut parts = meta.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let mut params = HashMap::new();
    for part in parts {
        if let Some((k, v)) = part.split_once('=') {
            params.insert(k.to_ascii_uppercase(), v.to_string());
        }
    }
    Some((name, params, value.to_string()))
}

fn parse_dt(
    value: &str,
    params: &HashMap<String, String>,
    utc_offset_hours: i32,
) -> Result<CivilDateTime, String> {
    let value = value.trim();
    let is_date = params
        .get("VALUE")
        .map(|v| v.eq_ignore_ascii_case("DATE"))
        .unwrap_or(false)
        || (!value.contains('T') && value.len() >= 8 && value[..8].bytes().all(|b| b.is_ascii_digit()));
    if is_date {
        let y: i32 = value[0..4].parse().map_err(|_| "bad DATE")?;
        let m: u32 = value[4..6].parse().map_err(|_| "bad DATE")?;
        let d: u32 = value[6..8].parse().map_err(|_| "bad DATE")?;
        return Ok(CivilDateTime {
            date: CivilDate::from_ymd(y, m, d)?,
            hour: 0,
            minute: 0,
            second: 0,
        });
    }
    let (raw, is_utc) = if let Some(stripped) = value.strip_suffix('Z') {
        (stripped, true)
    } else {
        (value, false)
    };
    if raw.len() < 15 {
        return Err(format!("bad DT value: {value}"));
    }
    let y: i32 = raw[0..4].parse().map_err(|_| "bad DT")?;
    let m: u32 = raw[4..6].parse().map_err(|_| "bad DT")?;
    let d: u32 = raw[6..8].parse().map_err(|_| "bad DT")?;
    let hour: u32 = raw[9..11].parse().map_err(|_| "bad DT")?;
    let minute: u32 = raw[11..13].parse().map_err(|_| "bad DT")?;
    let second: u32 = raw[13..15].parse().map_err(|_| "bad DT")?;
    let mut dt = CivilDateTime {
        date: CivilDate::from_ymd(y, m, d)?,
        hour,
        minute,
        second,
    };
    if is_utc {
        dt = shift_hours(dt, utc_offset_hours)?;
    }
    Ok(dt)
}

fn shift_hours(mut dt: CivilDateTime, hours: i32) -> Result<CivilDateTime, String> {
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

fn parse_duration_minutes(value: &str) -> Result<i64, String> {
    let value = value.trim().to_ascii_uppercase();
    if !value.starts_with('P') {
        return Err(format!("unsupported DURATION: {value}"));
    }
    let rest = &value[1..];
    let (date_part, time_part) = if let Some(idx) = rest.find('T') {
        (&rest[..idx], &rest[idx + 1..])
    } else {
        (rest, "")
    };
    let mut minutes = 0i64;
    minutes += take_num_unit(date_part, 'D')? * 24 * 60;
    minutes += take_num_unit(time_part, 'H')? * 60;
    minutes += take_num_unit(time_part, 'M')?;
    minutes += take_num_unit(time_part, 'S')? / 60;
    Ok(minutes)
}

fn take_num_unit(s: &str, unit: char) -> Result<i64, String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] as char == unit {
            let n: i64 = s[start..i]
                .parse()
                .map_err(|_| format!("bad duration number in {s}"))?;
            return Ok(n);
        }
    }
    Ok(0)
}

fn parse_rrule(value: &str) -> HashMap<String, String> {
    let mut parts = HashMap::new();
    for item in value.split(';') {
        if let Some((k, v)) = item.split_once('=') {
            parts.insert(k.to_ascii_uppercase(), v.to_string());
        }
    }
    parts
}

fn byday_map(token: &str) -> Option<u8> {
    match token {
        "MO" => Some(1),
        "TU" => Some(2),
        "WE" => Some(3),
        "TH" => Some(4),
        "FR" => Some(5),
        "SA" => Some(6),
        "SU" => Some(7),
        _ => None,
    }
}

fn strip_byday_offset(token: &str) -> &str {
    let bytes = token.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    &token[i..]
}

fn parse_ics(text: &str, utc_offset_hours: i32) -> Result<Vec<Event>, String> {
    let mut text = text.to_string();
    if text.starts_with('\u{feff}') {
        text = text.trim_start_matches('\u{feff}').to_string();
    }
    let text = unfold(&text);
    let mut events = Vec::new();
    for block in text.split("BEGIN:VEVENT").skip(1) {
        let body = block.split("END:VEVENT").next().unwrap_or("");
        let body = strip_nested_components(body);
        let mut fields: HashMap<String, (HashMap<String, String>, String)> = HashMap::new();
        let mut exdates = HashSet::new();
        for line in body.split('\n') {
            if line.is_empty() {
                continue;
            }
            let Some((name, params, value)) = parse_prop(line) else {
                continue;
            };
            let value = unescape(&value).trim().to_string();
            if name == "EXDATE" {
                for piece in value.split(',') {
                    let piece = piece.trim();
                    if piece.is_empty() {
                        continue;
                    }
                    exdates.insert(parse_dt(piece, &params, utc_offset_hours)?.date);
                }
                continue;
            }
            fields.entry(name).or_insert((params, value));
        }
        let Some((start_params, start_raw)) = fields.get("DTSTART") else {
            continue;
        };
        if !fields.contains_key("SUMMARY") {
            continue;
        }
        // 全天事件跳过
        let is_date_only = start_params
            .get("VALUE")
            .map(|v| v.eq_ignore_ascii_case("DATE"))
            .unwrap_or(false)
            || (!start_raw.contains('T') && start_raw.len() == 8);
        if is_date_only {
            continue;
        }
        let dtstart = parse_dt(start_raw, start_params, utc_offset_hours)?;
        let dtend = if let Some((end_params, end_raw)) = fields.get("DTEND") {
            parse_dt(end_raw, end_params, utc_offset_hours)?
        } else if let Some((_, dur)) = fields.get("DURATION") {
            add_minutes(dtstart, parse_duration_minutes(dur)?)?
        } else {
            add_minutes(dtstart, 45)?
        };
        if !datetime_gt(dtend, dtstart) {
            continue;
        }

        let mut until = None;
        let mut count = None;
        let mut interval = 1u32;
        let mut byday = Vec::new();
        if let Some((_, rule_raw)) = fields.get("RRULE") {
            let rule = parse_rrule(rule_raw);
            let freq = rule
                .get("FREQ")
                .map(|s| s.to_ascii_uppercase())
                .unwrap_or_else(|| "WEEKLY".into());
            if freq != "WEEKLY" {
                interval = 1;
                until = Some(dtstart.date);
                count = Some(1);
            } else {
                interval = rule
                    .get("INTERVAL")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1)
                    .max(1);
                if let Some(u) = rule.get("UNTIL") {
                    until = Some(parse_dt(u, &HashMap::new(), utc_offset_hours)?.date);
                }
                if let Some(c) = rule.get("COUNT") {
                    count = Some(c.parse::<u32>().unwrap_or(1).max(1));
                }
                if let Some(days) = rule.get("BYDAY") {
                    for token in days.split(',') {
                        let token = strip_byday_offset(token.trim()).to_ascii_uppercase();
                        if let Some(wd) = byday_map(&token) {
                            byday.push(wd);
                        }
                    }
                }
            }
        }

        let location = fields
            .get("LOCATION")
            .map(|(_, v)| collapse_ws(v))
            .unwrap_or_default();
        let description = fields
            .get("DESCRIPTION")
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let summary = fields.get("SUMMARY").map(|(_, v)| v.clone()).unwrap_or_default();
        events.push(Event {
            summary,
            location,
            description,
            start: dtstart,
            end: dtend,
            until,
            count,
            interval,
            byday,
            exdates,
        });
    }
    Ok(events)
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::new();
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn datetime_gt(a: CivilDateTime, b: CivilDateTime) -> bool {
    if a.date != b.date {
        return a.date > b.date;
    }
    (a.hour, a.minute, a.second) > (b.hour, b.minute, b.second)
}

fn add_minutes(dt: CivilDateTime, minutes: i64) -> Result<CivilDateTime, String> {
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

fn period_label(description: &str, summary: &str) -> String {
    for text in [description, summary] {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(label) = match_period(line) {
                return label;
            }
        }
    }
    String::new()
}

fn match_period(line: &str) -> Option<String> {
    // 第 a-b 节 / 第 a 节 / a-b 节
    if let Some((a, b)) = extract_range_after(line, &["第"], &["节"]) {
        return Some(if a == b {
            format!("第{a}节")
        } else {
            format!("第{a}-{b}节")
        });
    }
    if let Some(a) = extract_single_after(line, &["第"], &["节"]) {
        return Some(format!("第{a}节"));
    }
    if let Some((a, b)) = extract_range_before(line, &["节"]) {
        return Some(format!("第{a}-{b}节"));
    }
    None
}

fn extract_range_after(line: &str, prefixes: &[&str], suffixes: &[&str]) -> Option<(u32, u32)> {
    for prefix in prefixes {
        if let Some(idx) = line.find(prefix) {
            let rest = &line[idx + prefix.len()..];
            let (a, rest) = take_digits(rest)?;
            let rest = rest.trim_start_matches(|c: char| {
                c.is_whitespace() || matches!(c, '-' | '~' | '—' | '－' | '到' | '至')
            });
            // also handle unicode dash variants already stripped partially
            let rest = rest.trim_start_matches(|c: char| {
                c.is_whitespace() || "—－-~到至".contains(c)
            });
            let (b, rest) = take_digits(rest)?;
            if suffixes.iter().any(|s| rest.trim_start().starts_with(s)) {
                return Some((a, b));
            }
        }
    }
    None
}

fn extract_single_after(line: &str, prefixes: &[&str], suffixes: &[&str]) -> Option<u32> {
    for prefix in prefixes {
        if let Some(idx) = line.find(prefix) {
            let rest = &line[idx + prefix.len()..];
            let (a, rest) = take_digits(rest.trim_start())?;
            if suffixes.iter().any(|s| rest.trim_start().starts_with(s)) {
                // ensure not actually a range (digit-digit already handled)
                return Some(a);
            }
        }
    }
    None
}

fn extract_range_before(line: &str, suffixes: &[&str]) -> Option<(u32, u32)> {
    for suffix in suffixes {
        if let Some(idx) = line.find(suffix) {
            let before = &line[..idx];
            // find last "digits sep digits"
            let bytes = before.as_bytes();
            let mut i = bytes.len();
            while i > 0 && bytes[i - 1].is_ascii_whitespace() {
                i -= 1;
            }
            let end = i;
            while i > 0 && bytes[i - 1].is_ascii_digit() {
                i -= 1;
            }
            if i == end {
                continue;
            }
            let b: u32 = before[i..end].parse().ok()?;
            while i > 0
                && (bytes[i - 1].is_ascii_whitespace()
                    || matches!(bytes[i - 1] as char, '-' | '~'))
            {
                i -= 1;
            }
            // unicode dashes
            let before2 = &before[..i];
            let before2 = before2.trim_end_matches(|c: char| {
                c.is_whitespace() || "—－-~到至".contains(c)
            });
            let bytes2 = before2.as_bytes();
            let mut j = bytes2.len();
            while j > 0 && bytes2[j - 1].is_ascii_digit() {
                j -= 1;
            }
            if j == bytes2.len() {
                continue;
            }
            let a: u32 = before2[j..].parse().ok()?;
            return Some((a, b));
        }
    }
    None
}

fn take_digits(s: &str) -> Option<(u32, &str)> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let n: u32 = s[..i].parse().ok()?;
    Some((n, &s[i..]))
}

fn week_index(term_start: CivilDate, day: CivilDate) -> Result<i32, String> {
    let delta = days_between(term_start, day)?;
    if delta < 0 {
        return Ok(0);
    }
    Ok((delta / 7) as i32 + 1)
}

fn occurrence_weeks(
    event: &Event,
    term_start: CivilDate,
    weekday: u8,
) -> Result<Vec<i32>, String> {
    let mut start_week = week_index(term_start, event.start.date)?;
    if start_week <= 0 {
        start_week = 1;
    }
    let mut weeks = Vec::new();
    if let Some(count) = event.count {
        for i in 0..count {
            weeks.push(start_week + (i as i32) * (event.interval as i32));
        }
    } else if let Some(until) = event.until {
        let mut end_week = week_index(term_start, until)?;
        if end_week <= 0 {
            end_week = start_week;
        }
        let mut w = start_week;
        while w <= end_week {
            weeks.push(w);
            w += event.interval as i32;
        }
    } else {
        weeks.push(start_week);
    }

    if !event.exdates.is_empty() {
        let term_wd = term_start.weekday_iso();
        let delta = (weekday as i32 - term_wd as i32).rem_euclid(7);
        let mut filtered = Vec::new();
        for w in weeks {
            let day = term_start.add_days(((w - 1) * 7 + delta) as i64)?;
            if !event.exdates.contains(&day) {
                filtered.push(w);
            }
        }
        weeks = filtered;
    }
    Ok(weeks.into_iter().filter(|w| *w >= 1).collect())
}

fn compress_weeks(weeks: &[i32], interval: u32) -> Vec<(i32, i32, u32)> {
    if weeks.is_empty() {
        return Vec::new();
    }
    let weeks: Vec<i32> = weeks.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
    if interval <= 1 {
        let mut ranges = Vec::new();
        let mut start = weeks[0];
        let mut prev = weeks[0];
        for &w in &weeks[1..] {
            if w == prev + 1 {
                prev = w;
                continue;
            }
            ranges.push((start, prev, 1));
            start = w;
            prev = w;
        }
        ranges.push((start, prev, 1));
        return ranges;
    }
    let mut ranges = Vec::new();
    let mut start = weeks[0];
    let mut prev = weeks[0];
    for &w in &weeks[1..] {
        if w == prev + interval as i32 {
            prev = w;
            continue;
        }
        ranges.push((start, prev, interval));
        start = w;
        prev = w;
    }
    ranges.push((start, prev, interval));
    ranges
}

fn event_weekdays(event: &Event) -> Vec<u8> {
    if event.byday.is_empty() {
        return vec![event.start.date.weekday_iso()];
    }
    let mut seen = [false; 8];
    let mut out = Vec::new();
    for &day in &event.byday {
        if (1..=7).contains(&day) && !seen[day as usize] {
            seen[day as usize] = true;
            out.push(day);
        }
    }
    if out.is_empty() {
        vec![event.start.date.weekday_iso()]
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_weekly_rrule() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:线性代数A\nLOCATION:JX03-206\nDESCRIPTION:第1-3节\\n周1-17\nDTSTART:20260903T075000\nDTEND:20260903T101500\nRRULE:FREQ=WEEKLY;UNTIL=20261224T235959;INTERVAL=1;BYDAY=TH\nEND:VEVENT\nEND:VCALENDAR\n";
        let value = convert_ics_text(ics, Some("2026秋"), Some("2026-08-31"), 8).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["term"]["name"], "2026秋");
        let courses = value["courses"].as_array().unwrap();
        assert_eq!(courses.len(), 1);
        assert_eq!(courses[0]["name"], "线性代数A");
        assert_eq!(courses[0]["weekday"], 4);
        assert_eq!(courses[0]["start_min"], 470);
        assert_eq!(courses[0]["period_label"], "第1-3节");
        assert_eq!(courses[0]["weeks_start"], 1);
    }

    #[test]
    fn expands_multi_byday() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:体育\nLOCATION:操场\nDESCRIPTION:第1-2节\nDTSTART:20260901T080000\nDTEND:20260901T094000\nRRULE:FREQ=WEEKLY;UNTIL=20261220T235959;INTERVAL=1;BYDAY=MO,WE,FR\nEND:VEVENT\nEND:VCALENDAR\n";
        let value = convert_ics_text(ics, Some("2026秋"), Some("2026-08-31"), 8).unwrap();
        let courses = value["courses"].as_array().unwrap();
        assert_eq!(courses.len(), 3);
        let weekdays: Vec<u64> = courses
            .iter()
            .map(|c| c["weekday"].as_u64().unwrap())
            .collect();
        assert_eq!(weekdays, vec![1, 3, 5]);
        assert!(courses.iter().all(|c| c["name"] == "体育"));
    }
}
