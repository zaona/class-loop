//! 课表 JSON 组装：合并周次、压缩区间、统一地点。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::parse::strip_campus_prefix;
use super::time::{CivilDate, monday_of_week};

/// 合并键：同课同槽位（地点已规范化）。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CourseKey {
    pub weekday: u8,
    pub start_min: u32,
    pub end_min: u32,
    pub name: String,
    pub location: String,
}

#[derive(Clone, Debug, Default)]
pub struct CourseAccum {
    pub weeks: BTreeSet<i32>,
    pub period_label: String,
}

pub fn resolve_term_name(term_name: Option<&str>, term_start: CivilDate) -> String {
    match term_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(name) => name.to_string(),
        None => {
            let season = if term_start.m >= 8 { "秋" } else { "春" };
            format!("{}{season}", term_start.y)
        }
    }
}

pub fn resolve_term_start_from_dates(
    term_start: Option<&str>,
    dates: impl IntoIterator<Item = CivilDate>,
) -> Result<CivilDate, String> {
    if let Some(raw) = term_start {
        let raw = raw.trim();
        if !raw.is_empty() {
            return CivilDate::parse_ymd(raw);
        }
    }
    let min = dates
        .into_iter()
        .min()
        .ok_or_else(|| "ICS 中没有可用的上课日".to_string())?;
    monday_of_week(min)
}

/// `room_or_full` + 可选教师 → 去校区前缀后的规范地点。
pub fn normalize_location(room_or_full: &str, teacher: Option<&str>) -> String {
    let mut base = strip_campus_prefix(room_or_full);
    if let Some(teacher) = teacher.map(str::trim).filter(|s| !s.is_empty()) {
        let teacher = strip_campus_prefix(teacher);
        if !base.contains(&teacher) {
            base = if base.is_empty() {
                teacher
            } else {
                format!("{base} {teacher}")
            };
        }
    }
    base
}

pub fn merge_into(
    groups: &mut BTreeMap<CourseKey, CourseAccum>,
    key: CourseKey,
    week: i32,
    period_label: &str,
) {
    if week < 1 {
        return;
    }
    let entry = groups.entry(key).or_default();
    entry.weeks.insert(week);
    if entry.period_label.is_empty() && !period_label.is_empty() {
        entry.period_label = period_label.to_string();
    }
}

pub fn emit_schedule(
    groups: BTreeMap<CourseKey, CourseAccum>,
    term_name: &str,
    term_start: CivilDate,
) -> Result<Value, String> {
    let mut courses = Vec::new();
    let mut next_id = 1u64;
    for (key, accum) in groups {
        let period_label = accum.period_label;
        let weeks: Vec<i32> = accum.weeks.into_iter().collect();
        if weeks.is_empty() {
            continue;
        }
        for (weeks_start, weeks_end, interval) in compress_weeks(&weeks) {
            let mut course = json!({
                "id": next_id,
                "name": key.name,
                "location": key.location,
                "weekday": key.weekday,
                "start_min": key.start_min,
                "end_min": key.end_min,
                "period_label": period_label.clone(),
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
    if courses.is_empty() {
        return Err("转换结果为空（可能全被学期起始日过滤）".into());
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

/// 将已排序的教学周列表压成若干闭区间；优先识别等间隔（如隔周）。
pub fn compress_weeks(weeks: &[i32]) -> Vec<(i32, i32, u32)> {
    if weeks.is_empty() {
        return Vec::new();
    }
    let weeks: Vec<i32> = weeks.iter().copied().collect::<BTreeSet<_>>().into_iter().collect();
    if weeks.len() == 1 {
        return vec![(weeks[0], weeks[0], 1)];
    }

    // 若整体等间隔且间隔>1，压成一条。
    let gap = weeks[1] - weeks[0];
    if gap > 1
        && weeks
            .windows(2)
            .all(|w| w[1] - w[0] == gap)
    {
        return vec![(weeks[0], *weeks.last().unwrap(), gap as u32)];
    }

    // 否则按连续周切段（interval=1）。
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
    ranges
}

/// 忽略 id / period_label 的可比指纹（三源一致性用）。
#[cfg(test)]
pub fn fingerprint_courses(schedule: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(courses) = schedule["courses"].as_array() else {
        return out;
    };
    for c in courses {
        let interval = c["week_interval"].as_u64().unwrap_or(1);
        out.push(format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            c["name"].as_str().unwrap_or(""),
            c["location"].as_str().unwrap_or(""),
            c["weekday"],
            c["start_min"],
            c["end_min"],
            c["weeks_start"],
            c["weeks_end"],
            interval,
        ));
    }
    out.sort();
    out
}
