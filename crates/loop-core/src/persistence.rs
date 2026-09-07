//! 课表 JSON 加载。
//!
//! 只读快应用 `top.zaona.loopimport` 沙箱中的 schedule.json；
//! 文件缺失时由调用方保持空课表，不内置样例。

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::model::{SCHEDULE_VERSION, ScheduleFile};

/// 快应用 `internal://files/loop` 映射到设备上的目录。
pub const IMPORT_ROOT: &str = "/data/files/top.zaona.loopimport/loop";

/// 原生模块读取的课表清单路径。
pub const SCHEDULE_PATH: &str = "/data/files/top.zaona.loopimport/loop/schedule.json";

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

fn map_parse_error<E>(error: PersistenceError<()>) -> PersistenceError<E> {
    match error {
        PersistenceError::Json => PersistenceError::Json,
        PersistenceError::Version => PersistenceError::Version,
        PersistenceError::Storage(()) => PersistenceError::Json,
    }
}

/// 读取快应用发布的 schedule.json。
///
/// `Ok(None)` 表示文件不存在；非法内容返回 `Err`。
pub fn load_schedule<S: Store>(
    store: &mut S,
) -> Result<Option<ScheduleFile>, PersistenceError<S::Error>> {
    match store.read(SCHEDULE_PATH) {
        Ok(None) => Ok(None),
        Ok(Some(bytes)) => parse_schedule_bytes(&bytes)
            .map(Some)
            .map_err(map_parse_error),
        Err(error) => Err(PersistenceError::Storage(error)),
    }
}
