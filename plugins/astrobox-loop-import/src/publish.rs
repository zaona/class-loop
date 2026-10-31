pub use crate::transfer::start;
use crate::{ics, state};
use serde_json::Value;
use std::{fs, path::Path};
pub const MAX_SCHEDULE_BYTES: usize = loop_protocol::SCHEDULE_MAX;
const MAX_ICS_BYTES: u64 = 2 * 1024 * 1024;

pub fn prepare_file(
    path: &str,
    name: &str,
    source: ics::IcsSource,
    term_name: Option<&str>,
    term_start: Option<&str>,
) -> Result<state::PreparedSchedule, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("cannot stat file: {error}"))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("selected file is empty".to_string());
    }
    if !has_ics_extension(name) && !has_ics_extension(path) && !file_looks_like_ics(path)? {
        let shown = if name.is_empty() { path } else { name };
        return Err(format!("不支持的文件类型（{shown}）。请选择 .ics 课表文件"));
    }
    if metadata.len() > MAX_ICS_BYTES {
        return Err("ICS file exceeds 2 MiB".to_string());
    }
    let schedule = ics::convert_ics_file(path, source, term_name, term_start)?;
    validate_schedule(&schedule)?;
    let encoded = schedule.to_string();
    if encoded.len() > MAX_SCHEDULE_BYTES {
        return Err(format!(
            "converted schedule exceeds {MAX_SCHEDULE_BYTES} bytes"
        ));
    }
    let course_count = schedule
        .get("courses")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    Ok(state::PreparedSchedule {
        name: display_ics_name(name, path),
        path: path.to_string(),
        size: metadata.len(),
        schedule,
        course_count,
    })
}

fn has_ics_extension(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("ics"))
        .unwrap_or(false)
}

fn file_looks_like_ics(path: &str) -> Result<bool, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("cannot open file: {error}"))?;
    use std::io::Read;
    let mut buf = [0u8; 256];
    let n = file
        .read(&mut buf)
        .map_err(|error| format!("cannot read file: {error}"))?;
    Ok(bytes_look_like_ics(&buf[..n]))
}

fn bytes_look_like_ics(bytes: &[u8]) -> bool {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return false,
    };
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.len() >= 15 && trimmed[..15].eq_ignore_ascii_case("BEGIN:VCALENDAR")
}

fn display_ics_name(name: &str, path: &str) -> String {
    let base = if name.is_empty() {
        Path::new(path)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or(path)
    } else {
        name
    };
    if has_ics_extension(base) {
        base.to_string()
    } else if base.is_empty() {
        "schedule.ics".to_string()
    } else {
        format!("{base}.ics")
    }
}

pub fn refresh_prepared_from_state() -> Result<(), String> {
    refresh_prepared(false)
}

/// 切换 ICS 来源时：丢弃当前学期字段，按新来源重新自动推断。
pub fn reconvert_prepared_inferred() -> Result<(), String> {
    refresh_prepared(true)
}

fn refresh_prepared(infer_terms: bool) -> Result<(), String> {
    let snapshot = state::snapshot();
    let Some(prepared) = snapshot.prepared else {
        return Ok(());
    };
    let (term_name, term_start) = if infer_terms {
        (None, None)
    } else {
        (
            state::empty_to_none(&snapshot.term_name),
            state::empty_to_none(&snapshot.term_start),
        )
    };
    let next = prepare_file(
        &prepared.path,
        &prepared.name,
        snapshot.ics_source,
        term_name,
        term_start,
    )?;
    state::with_state(|state| {
        state.status = format!(
            "已按 {} 重新转换，共 {} 门课。",
            snapshot.ics_source.label(),
            next.course_count
        );
        state.prepared = Some(next);
    });
    Ok(())
}

fn media_unavailable(path: &str) -> bool {
    match fs::metadata(path) {
        Ok(meta) => !meta.is_file() || meta.len() == 0,
        Err(_) => true,
    }
}

/// 推送前尽量按当前学期重转；若原 ICS 已不可用，则回退已缓存的 schedule。
pub fn schedule_for_push() -> Result<Value, String> {
    let snapshot = state::snapshot();
    let Some(prepared) = snapshot.prepared else {
        return Err("请先选择课表 .ics 文件。".to_string());
    };
    if media_unavailable(&prepared.path) {
        validate_schedule(&prepared.schedule)?;
        return Ok(prepared.schedule);
    }
    let term_name = state::empty_to_none(&snapshot.term_name);
    let term_start = state::empty_to_none(&snapshot.term_start);
    match prepare_file(
        &prepared.path,
        &prepared.name,
        snapshot.ics_source,
        term_name,
        term_start,
    ) {
        Ok(next) => {
            let schedule = next.schedule.clone();
            state::with_state(|state| {
                state.prepared = Some(next);
            });
            Ok(schedule)
        }
        Err(error) => Err(error),
    }
}

pub fn validate_schedule(value: &Value) -> Result<(), String> {
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("schedule.version must be 1".to_string());
    }
    let term = value
        .get("term")
        .filter(|item| item.is_object())
        .ok_or_else(|| "schedule.term missing".to_string())?;
    if term
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        return Err("schedule.term.name missing".to_string());
    }
    if term
        .get("start_date")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        return Err("schedule.term.start_date missing".to_string());
    }
    value
        .get("courses")
        .and_then(Value::as_array)
        .ok_or_else(|| "schedule.courses must be an array".to_string())?;
    Ok(())
}
