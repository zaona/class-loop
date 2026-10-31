//! 课表 JSON 加载。
//!
//! CanopusConnect 发布至模块私有目录的 schedule.json；
//! 文件缺失时由调用方保持空课表，不内置样例。

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::model::{SCHEDULE_VERSION, ScheduleFile};

/// 相对于 Supervisor 分配的模块私有目录。
pub const SCHEDULE_PATH: &str = "schedule.json";

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

/// Accept only schedulable dates, times and week ranges from the importer.
pub fn valid_import(file: &ScheduleFile) -> bool {
    let Some((year, month, day)) = crate::schedule::parse_ymd(&file.term.start_date) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    !file.term.name.trim().is_empty()
        && day <= days
        && file.courses.iter().all(|c| {
            c.id != 0
                && !c.name.trim().is_empty()
                && (1..=7).contains(&c.weekday)
                && c.start_min < c.end_min
                && c.end_min <= 1440
                && c.weeks_start > 0
                && c.weeks_start <= c.weeks_end
                && c.week_interval > 0
        })
}

/// 读取模块私有目录的 schedule.json。
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
