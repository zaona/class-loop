//! Nexio：按周拆成多条 `RRULE;COUNT=1`；DESCRIPTION `第N周`；LOCATION 已含教室/教师。

use std::collections::BTreeMap;

use serde_json::Value;

use super::parse::parse_vevents;
use super::schedule::{
    CourseAccum, CourseKey, emit_schedule, merge_into, normalize_location, resolve_term_name,
};
use super::time::{CivilDate, monday_of_week, week_index};

pub fn convert(
    text: &str,
    term_name: Option<&str>,
    term_start: Option<&str>,
    utc_offset_hours: i32,
) -> Result<Value, String> {
    let events = parse_vevents(text, utc_offset_hours)?;
    if events.is_empty() {
        return Err("ICS 中没有可用的 VEVENT".into());
    }
    if let Some(hint) = prodid_hint(text) {
        tracing::info!("{hint}");
    }

    let term_start_date = resolve_term_start(term_start, &events)?;
    let term_name = resolve_term_name(term_name, term_start_date);

    let mut groups: BTreeMap<CourseKey, CourseAccum> = BTreeMap::new();
    for event in &events {
        let location = normalize_location(&event.location, None);
        let week = match week_from_description(&event.description) {
            Some(week) if week >= 1 => week,
            _ => {
                let week = week_index(term_start_date, event.start.date)?;
                if week < 1 {
                    continue;
                }
                week
            }
        };
        let key = CourseKey {
            weekday: event.start.date.weekday_iso(),
            start_min: event.start.minutes_of_day(),
            end_min: event.end.minutes_of_day(),
            name: event.summary.clone(),
            location,
        };
        merge_into(&mut groups, key, week, "");
    }

    emit_schedule(groups, &term_name, term_start_date)
}

fn prodid_hint(text: &str) -> Option<String> {
    let upper = text.to_ascii_uppercase();
    if upper.contains("NEXIO") {
        return None;
    }
    Some("所选文件可能不是 Nexio 导出的 ICS，转换结果请自行核对。".into())
}

fn week_from_description(description: &str) -> Option<i32> {
    for line in description.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix('第') else {
            continue;
        };
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 {
            continue;
        }
        let week: i32 = rest[..i].parse().ok()?;
        let tail = rest[i..].trim_start();
        if tail.starts_with('周') {
            return Some(week);
        }
    }
    None
}

fn resolve_term_start(
    term_start: Option<&str>,
    events: &[super::parse::RawEvent],
) -> Result<CivilDate, String> {
    if let Some(raw) = term_start {
        let raw = raw.trim();
        if !raw.is_empty() {
            return CivilDate::parse_ymd(raw);
        }
    }
    for event in events {
        if let Some(week) = week_from_description(&event.description) {
            if week >= 1 {
                let week_monday = monday_of_week(event.start.date)?;
                return week_monday.add_days(-((week as i64 - 1) * 7));
            }
        }
    }
    let min = events
        .iter()
        .map(|e| e.start.date)
        .min()
        .ok_or_else(|| "ICS 中没有可用的上课日".to_string())?;
    monday_of_week(min)
}
