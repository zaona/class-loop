//! 课表 JSON 加载与路径安全检查。

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

use crate::model::{SCHEDULE_VERSION, ScheduleFile};

/// 设备端覆盖文件约定路径（快应用或调试推送可写入此处）。
pub const SCHEDULE_OVERRIDE_PATH: &str = "/data/files/com.canopus.loop/schedule.json";
pub const PACKAGE_FILES_ROOT: &str = "/data/files/com.canopus.loop";

/// 编译进固件的样例课表（由 `scripts/ics-to-schedule.py` 生成）。
pub const BUILTIN_SCHEDULE_JSON: &str = include_str!("../../../fixtures/schedule.json");

pub trait Store {
    type Error;
    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceError<E> {
    Storage(E),
    Json,
    Version,
}

#[derive(Serialize, Deserialize)]
struct WireFile {
    version: u8,
    term: crate::model::Term,
    #[serde(default)]
    courses: Vec<crate::model::Course>,
}

pub fn parse_schedule_bytes(bytes: &[u8]) -> Result<ScheduleFile, PersistenceError<()>> {
    let file: WireFile = serde_json::from_slice(bytes).map_err(|_| PersistenceError::Json)?;
    if file.version != SCHEDULE_VERSION {
        return Err(PersistenceError::Version);
    }
    Ok(ScheduleFile {
        version: file.version,
        term: file.term,
        courses: file.courses,
    })
}

pub fn load_builtin() -> Result<ScheduleFile, PersistenceError<()>> {
    parse_schedule_bytes(BUILTIN_SCHEDULE_JSON.as_bytes())
}

/// 优先读覆盖文件；缺失或损坏时回退到内置样例。
pub fn load_schedule<S: Store>(
    store: &mut S,
) -> Result<(ScheduleFile, bool), PersistenceError<S::Error>> {
    match store.read(SCHEDULE_OVERRIDE_PATH) {
        Ok(Some(bytes)) => match parse_schedule_bytes(&bytes) {
            Ok(file) => Ok((file, true)),
            Err(PersistenceError::Json) => Err(PersistenceError::Json),
            Err(PersistenceError::Version) => Err(PersistenceError::Version),
            Err(PersistenceError::Storage(_)) => unreachable!(),
        },
        Ok(None) => load_builtin()
            .map(|file| (file, false))
            .map_err(|e| match e {
                PersistenceError::Json => PersistenceError::Json,
                PersistenceError::Version => PersistenceError::Version,
                PersistenceError::Storage(()) => PersistenceError::Json,
            }),
        Err(error) => Err(PersistenceError::Storage(error)),
    }
}

pub fn truncate_label(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}
