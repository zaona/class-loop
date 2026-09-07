//! WakeUp：RRULE WEEKLY + UNTIL/COUNT；DESCRIPTION 首行节次；LOCATION 含校区/教室/教师。

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;

use super::parse::{parse_dt, parse_rrule_map, parse_vevents};
use super::schedule::{
    CourseAccum, CourseKey, emit_schedule, merge_into, normalize_location, resolve_term_name,
    resolve_term_start_from_dates,
};
use super::time::{CivilDate, CivilDateTime, week_index};

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
        let location = normalize_location(&event.location, None);
        let period = period_label(&event.description);
        let rule = event
            .rrule
            .as_deref()
            .map(|raw| parse_weekly_rule(raw, utc_offset_hours))
            .transpose()?;

        for weekday in weekdays_for_event(event, utc_offset_hours)? {
            let weeks = expand_weeks(
                event.start,
                weekday,
                rule.as_ref(),
                &event.exdates,
                term_start_date,
                utc_offset_hours,
            )?;
            let key = CourseKey {
                weekday,
                start_min: event.start.minutes_of_day(),
                end_min: event.end.minutes_of_day(),
                name: event.summary.clone(),
                location: location.clone(),
            };
            for week in weeks {
                merge_into(&mut groups, key.clone(), week, &period);
            }
        }
    }

    emit_schedule(groups, &term_name, term_start_date)
}

fn prodid_hint(text: &str) -> Option<String> {
    let upper = text.to_ascii_uppercase();
    if upper.contains("WAKEUPSCHEDULE") || upper.contains("YZUNE") {
        return None;
    }
    Some("所选文件可能不是 WakeUp 导出的 ICS，转换结果请自行核对。".into())
}

#[derive(Clone, Debug)]
struct WeeklyRule {
    interval: u32,
    until: Option<CivilDateTime>,
    count: Option<u32>,
    byday: Vec<u8>,
}

fn parse_weekly_rule(raw: &str, utc_offset_hours: i32) -> Result<WeeklyRule, String> {
    let rule = parse_rrule_map(raw);
    let freq = rule
        .get("FREQ")
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_else(|| "WEEKLY".into());
    if freq != "WEEKLY" {
        return Ok(WeeklyRule {
            interval: 1,
            until: None,
            count: Some(1),
            byday: Vec::new(),
        });
    }
    let interval = rule
        .get("INTERVAL")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1);
    let until = if let Some(u) = rule.get("UNTIL") {
        Some(parse_dt(u, &HashMap::new(), utc_offset_hours)?)
    } else {
        None
    };
    let count = rule
        .get("COUNT")
        .and_then(|s| s.parse::<u32>().ok())
        .map(|c| c.max(1));
    let mut byday = Vec::new();
    if let Some(days) = rule.get("BYDAY") {
        for token in days.split(',') {
            let token = strip_byday_offset(token.trim()).to_ascii_uppercase();
            if let Some(wd) = byday_map(&token) {
                byday.push(wd);
            }
        }
    }
    Ok(WeeklyRule {
        interval,
        until,
        count,
        byday,
    })
}

fn weekdays_for_event(
    event: &super::parse::RawEvent,
    utc_offset_hours: i32,
) -> Result<Vec<u8>, String> {
    if let Some(raw) = &event.rrule {
        let rule = parse_weekly_rule(raw, utc_offset_hours)?;
        if !rule.byday.is_empty() {
            return Ok(unique_weekdays(rule.byday));
        }
    }
    Ok(vec![event.start.date.weekday_iso()])
}

fn unique_weekdays(days: Vec<u8>) -> Vec<u8> {
    let mut seen = [false; 8];
    let mut out = Vec::new();
    for day in days {
        if (1..=7).contains(&day) && !seen[day as usize] {
            seen[day as usize] = true;
            out.push(day);
        }
    }
    out
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

/// 按真实发生日展开（尊重 UNTIL 墙钟），再映射教学周。
fn expand_weeks(
    start: CivilDateTime,
    weekday: u8,
    rule: Option<&WeeklyRule>,
    exdates: &HashSet<CivilDate>,
    term_start: CivilDate,
    _utc_offset_hours: i32,
) -> Result<Vec<i32>, String> {
    let interval = rule.map(|r| r.interval).unwrap_or(1).max(1);
    let until = rule.and_then(|r| r.until);
    let count = rule.and_then(|r| r.count);

    // 对齐到目标 weekday：若 DTSTART 当日不是该 weekday，找到本周或下周对应日。
    let start_wd = start.date.weekday_iso();
    let delta = (weekday as i32 - start_wd as i32).rem_euclid(7);
    let mut cursor_date = start.date.add_days(delta as i64)?;
    let cursor = CivilDateTime {
        date: cursor_date,
        hour: start.hour,
        minute: start.minute,
        second: start.second,
    };

    let mut weeks = Vec::new();
    let mut emitted = 0u32;
    let mut guard = 0u32;
    let mut current = cursor;
    while guard < 512 {
        guard += 1;
        if let Some(until) = until {
            if current.cmp_key() > until.cmp_key() {
                break;
            }
        }
        if let Some(count) = count {
            if emitted >= count {
                break;
            }
        }
        if !exdates.contains(&current.date) {
            let week = week_index(term_start, current.date)?;
            if week >= 1 {
                weeks.push(week);
            }
        }
        emitted += 1;
        if count.is_none() && until.is_none() {
            break;
        }
        cursor_date = current.date.add_days((7 * interval) as i64)?;
        current = CivilDateTime {
            date: cursor_date,
            hour: current.hour,
            minute: current.minute,
            second: current.second,
        };
    }
    Ok(weeks)
}

fn period_label(description: &str) -> String {
    for line in description.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(label) = match_period(line) {
            return label;
        }
    }
    String::new()
}

fn match_period(line: &str) -> Option<String> {
    if let Some((a, b)) = extract_range(line) {
        return Some(if a == b {
            format!("第{a}节")
        } else {
            format!("第{a}-{b}节")
        });
    }
    if let Some(a) = extract_single(line) {
        return Some(format!("第{a}节"));
    }
    None
}

fn extract_range(line: &str) -> Option<(u32, u32)> {
    let idx = line.find('第')?;
    let rest = &line[idx + '第'.len_utf8()..];
    let (a, rest) = take_digits(rest)?;
    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || "—－-~到至".contains(c));
    let (b, rest) = take_digits(rest)?;
    if rest.trim_start().starts_with('节') {
        Some((a, b))
    } else {
        None
    }
}

fn extract_single(line: &str) -> Option<u32> {
    let idx = line.find('第')?;
    let rest = &line[idx + '第'.len_utf8()..];
    let (a, rest) = take_digits(rest.trim_start())?;
    if rest.trim_start().starts_with('节') {
        Some(a)
    } else {
        None
    }
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
