use std::{
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

use crate::{ics, interconnect, state};

pub const PROTOCOL_VERSION: u64 = 1;
pub const MAX_SCHEDULE_BYTES: usize = 40_960;
const MAX_ICS_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Awaiting {
    Hello,
    Ack,
}

struct Pending {
    addr: String,
    id: String,
    schedule: Value,
    awaiting: Awaiting,
}

static PENDING: OnceLock<Mutex<Option<Pending>>> = OnceLock::new();

fn pending() -> &'static Mutex<Option<Pending>> {
    PENDING.get_or_init(|| Mutex::new(None))
}

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
        return Err(format!(
            "不支持的文件类型（{shown}）。请选择 .ics 课表文件"
        ));
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

pub async fn start(addr: &str, schedule: Value) -> Result<(), String> {
    if addr.is_empty() {
        return Err("请选择已连接设备".to_string());
    }
    if pending()
        .lock()
        .unwrap_or_else(|item| item.into_inner())
        .is_some()
    {
        return Err("已有推送进行中".to_string());
    }

    validate_schedule(&schedule)?;
    let probe = json!({
        "tag": "loop-import-publish",
        "version": PROTOCOL_VERSION,
        "id": "probe",
        "schedule": schedule,
    });
    if probe.to_string().len() > interconnect::OUTGOING_FRAME_CAPACITY {
        return Err("serialized publish frame exceeds 49152 bytes".to_string());
    }

    let id = new_id();
    {
        *pending().lock().unwrap_or_else(|item| item.into_inner()) = Some(Pending {
            addr: addr.to_string(),
            id: id.clone(),
            schedule,
            awaiting: Awaiting::Hello,
        });
    }
    state::with_state(|state| {
        state.busy = true;
        state.status = "正在连接 Loop Import 快应用…".to_string();
    });

    if let Err(error) = interconnect::send(
        addr,
        &json!({ "tag": "loop-import-hello", "version": PROTOCOL_VERSION }),
    )
    .await
    {
        finish_error(&error);
        return Err(error);
    }
    Ok(())
}

pub async fn handle(addr: &str, package: &str, payload: &str) {
    if !package.is_empty() && package != interconnect::ROUTE_PACKAGE {
        tracing::warn!(%package, "ignore interconnect from unexpected package");
        return;
    }
    let Some(value) = parse_payload_object(payload) else {
        tracing::warn!("ignore interconnect payload that is not JSON object");
        return;
    };
    let Some(tag) = value.get("tag").and_then(Value::as_str) else {
        return;
    };
    if !tag.starts_with("loop-import-") {
        return;
    }

    let result = match tag {
        "loop-import-hello" => handle_hello(addr, &value).await,
        "loop-import-ack" => handle_ack(addr, &value),
        "loop-import-error" => {
            let code = value.get("code").and_then(Value::as_str).unwrap_or("error");
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("quick app rejected publish");
            Err(format!("{code}: {message}"))
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        finish_error(&error);
    }
}

fn parse_payload_object(payload: &str) -> Option<Value> {
    let mut value: Value = serde_json::from_str(payload).ok()?;
    // 兼容快应用误把 JSON.stringify 结果再交给系统序列化的双重编码。
    for _ in 0..2 {
        match value {
            Value::String(inner) => {
                value = serde_json::from_str(&inner).ok()?;
            }
            Value::Object(_) => return Some(value),
            _ => return None,
        }
    }
    matches!(value, Value::Object(_)).then_some(value)
}

fn json_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_i64().map(|n| n as u64))
            .or_else(|| item.as_f64().map(|n| n as u64))
            .or_else(|| item.as_str()?.parse().ok())
    })
}

fn json_bool_truthy(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .map(|item| match item {
            Value::Bool(flag) => *flag,
            Value::Number(number) => number.as_u64() == Some(1) || number.as_i64() == Some(1),
            Value::String(text) => {
                let text = text.trim();
                text.eq_ignore_ascii_case("true") || text == "1"
            }
            _ => false,
        })
        .unwrap_or(false)
}

async fn handle_hello(addr: &str, value: &Value) -> Result<(), String> {
    // 插件发出的 hello 只有 tag/version；快应用回包带 ok / maxScheduleBytes。
    let is_response = json_bool_truthy(value, "ok")
        || value.get("maxScheduleBytes").is_some();
    if !is_response {
        return Ok(());
    }
    let max_bytes = json_u64(value, "maxScheduleBytes").unwrap_or(MAX_SCHEDULE_BYTES as u64) as usize;
    if json_u64(value, "version") != Some(PROTOCOL_VERSION) {
        return Err("快应用不支持 Loop Import v1".to_string());
    }

    let (id, schedule) = {
        let mut guard = pending().lock().unwrap_or_else(|item| item.into_inner());
        let Some(item) = guard.as_mut() else {
            return Ok(());
        };
        if item.awaiting != Awaiting::Hello {
            return Err("意外的 hello 响应".to_string());
        }
        if !item.addr.is_empty() && item.addr != addr {
            // 某些固件回包 addr 格式不同；仍接受并改用回包 addr 继续发送。
            tracing::warn!(
                expected = %item.addr,
                got = %addr,
                "hello addr mismatch; continuing with reply addr"
            );
            item.addr = addr.to_string();
        }
        let encoded = item.schedule.to_string();
        if encoded.len() > max_bytes.min(MAX_SCHEDULE_BYTES) {
            return Err("课表超过快应用声明的大小上限".to_string());
        }
        item.awaiting = Awaiting::Ack;
        (item.id.clone(), item.schedule.clone())
    };

    state::with_state(|state| {
        state.status = "正在推送课表…".to_string();
    });
    interconnect::send(
        addr,
        &json!({
            "tag": "loop-import-publish",
            "version": PROTOCOL_VERSION,
            "id": id,
            "schedule": schedule,
        }),
    )
    .await
}

fn handle_ack(addr: &str, value: &Value) -> Result<(), String> {
    let courses = value
        .get("courses")
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    {
        let mut guard = pending().lock().unwrap_or_else(|item| item.into_inner());
        let Some(item) = guard.as_ref() else {
            return Ok(());
        };
        if item.addr != addr || item.awaiting != Awaiting::Ack {
            return Err("意外的 ack 响应".to_string());
        }
        if value.get("id").and_then(Value::as_str) != Some(item.id.as_str()) {
            return Err("ack ID 不匹配".to_string());
        }
        *guard = None;
    }
    state::with_state(|state| {
        state.busy = false;
        state.status = match courses {
            Some(n) => format!("推送成功，共 {n} 门课。打开 Loop 即可使用。"),
            None => "推送成功。打开 Loop 即可使用。".to_string(),
        };
    });
    Ok(())
}

fn finish_error(error: &str) {
    *pending().lock().unwrap_or_else(|item| item.into_inner()) = None;
    state::with_state(|state| {
        state.busy = false;
        state.status = format!("推送失败：{error}");
    });
}

pub fn validate_schedule(value: &Value) -> Result<(), String> {
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("schedule.version must be 1".to_string());
    }
    let term = value
        .get("term")
        .filter(|item| item.is_object())
        .ok_or_else(|| "schedule.term missing".to_string())?;
    if term.get("name").and_then(Value::as_str).unwrap_or("").is_empty() {
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

fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:032x}")
}
