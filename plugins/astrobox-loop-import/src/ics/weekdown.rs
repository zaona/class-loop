//! WeekDown：按周展开的独立 VEVENT；DESCRIPTION `教师:` / `教室:`；无 RRULE。

use std::collections::BTreeMap;

use serde_json::Value;

use super::parse::parse_vevents;
use super::schedule::{
    CourseAccum, CourseKey, emit_schedule, merge_into, normalize_location, resolve_term_name,
    resolve_term_start_from_dates,
};
use super::time::week_index;

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

    let term_start_date =
        resolve_term_start_from_dates(term_start, events.iter().map(|e| e.start.date))?;
    let term_name = resolve_term_name(term_name, term_start_date);

    let mut groups: BTreeMap<CourseKey, CourseAccum> = BTreeMap::new();
    for event in &events {
        let teacher = teacher_from_description(&event.description);
        let location = normalize_location(&event.location, teacher.as_deref());
        let week = week_index(term_start_date, event.start.date)?;
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
    if upper.contains("WEEKDOWN") {
        return None;
    }
    Some("所选文件可能不是 WeekDown 导出的 ICS，转换结果请自行核对。".into())
}

fn teacher_from_description(description: &str) -> Option<String> {
    for line in description.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("教师") else {
            continue;
        };
        let rest = rest.trim_start().trim_start_matches([':', '：']).trim();
        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }
    None
}
