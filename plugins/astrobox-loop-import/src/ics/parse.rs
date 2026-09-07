//! 通用 ICS 文本展开与 VEVENT 属性解析（不含任何课表方言语义）。

use std::collections::{HashMap, HashSet};

use super::time::{CivilDate, CivilDateTime, add_minutes, shift_hours};

#[derive(Clone, Debug)]
pub struct RawEvent {
    pub summary: String,
    pub location: String,
    pub description: String,
    pub start: CivilDateTime,
    pub end: CivilDateTime,
    pub rrule: Option<String>,
    pub exdates: HashSet<CivilDate>,
}

pub fn parse_vevents(text: &str, utc_offset_hours: i32) -> Result<Vec<RawEvent>, String> {
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
        if dtend.cmp_key() <= dtstart.cmp_key() {
            continue;
        }
        events.push(RawEvent {
            summary: fields
                .get("SUMMARY")
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
            location: fields
                .get("LOCATION")
                .map(|(_, v)| collapse_ws(v))
                .unwrap_or_default(),
            description: fields
                .get("DESCRIPTION")
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
            start: dtstart,
            end: dtend,
            rrule: fields.get("RRULE").map(|(_, v)| v.clone()),
            exdates,
        });
    }
    Ok(events)
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

pub fn parse_dt(
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

pub fn collapse_ws(s: &str) -> String {
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

/// 去掉常见校区前缀，使 WakeUp / WeekDown / Nexio 地点字符串可比。
pub fn strip_campus_prefix(location: &str) -> String {
    let s = collapse_ws(location);
    for prefix in ["啬园校区", "启秀校区", "钟秀校区"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return collapse_ws(rest);
        }
    }
    s
}

pub fn parse_rrule_map(value: &str) -> HashMap<String, String> {
    let mut parts = HashMap::new();
    for item in value.split(';') {
        if let Some((k, v)) = item.split_once('=') {
            parts.insert(k.to_ascii_uppercase(), v.to_string());
        }
    }
    parts
}
