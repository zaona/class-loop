//! 只读/删除适配：操作快应用 `top.zaona.loopimport` 沙箱中的课表。

use alloc::vec::Vec;
use core::ffi::c_void;

use canopus_target_private::{O_RDONLY, nuttx_close, nuttx_open, nuttx_read, nuttx_unlink};
use loop_core::{
    ScheduleFile,
    persistence::{self, IMPORT_ROOT, PersistenceError, SCHEDULE_PATH, Store},
};

const MAX_SCHEDULE_BYTES: usize = 64 * 1024;
const SCHEDULE_TMP_PATH: &str = "/data/files/top.zaona.loopimport/loop/schedule.json.tmp";

pub struct FsStore;

fn c_path(path: &str) -> Result<Vec<u8>, i32> {
    if path.as_bytes().contains(&0) {
        return Err(-1);
    }
    let mut output = Vec::with_capacity(path.len() + 1);
    output.extend_from_slice(path.as_bytes());
    output.push(0);
    Ok(output)
}

/// 仅允许快应用课表目录下的路径。
pub fn resolve_path(path: &str) -> Option<&str> {
    if path.starts_with(IMPORT_ROOT) {
        Some(path)
    } else {
        None
    }
}

fn read_bounded(path: &str, limit: usize) -> Result<Option<Vec<u8>>, i32> {
    let Some(path) = resolve_path(path) else {
        return Ok(None);
    };
    let path = c_path(path)?;
    let fd = unsafe { nuttx_open(path.as_ptr(), O_RDONLY) };
    if fd < 0 {
        return Ok(None);
    }
    let mut output = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let count =
            unsafe { nuttx_read(fd, chunk.as_mut_ptr().cast::<c_void>(), chunk.len() as u32) };
        if count < 0 {
            let _ = unsafe { nuttx_close(fd) };
            return Err(count);
        }
        if count == 0 {
            break;
        }
        if output.len() + count as usize > limit {
            let _ = unsafe { nuttx_close(fd) };
            return Err(-2);
        }
        output.extend_from_slice(&chunk[..count as usize]);
    }
    let result = unsafe { nuttx_close(fd) };
    if result < 0 {
        return Err(result);
    }
    Ok(Some(output))
}

fn unlink_path(path: &str) -> Result<(), i32> {
    let Some(path) = resolve_path(path) else {
        return Err(-1);
    };
    let path = c_path(path)?;
    let result = unsafe { nuttx_unlink(path.as_ptr()) };
    // 文件本就不存在时也视为成功。
    if result < 0 {
        let exists = unsafe { nuttx_open(path.as_ptr(), O_RDONLY) };
        if exists >= 0 {
            let _ = unsafe { nuttx_close(exists) };
            return Err(result);
        }
    }
    Ok(())
}

impl Store for FsStore {
    type Error = i32;

    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        read_bounded(path, MAX_SCHEDULE_BYTES)
    }
}

fn map_error(error: PersistenceError<i32>) -> i32 {
    match error {
        PersistenceError::Storage(error) => error,
        PersistenceError::Json => -5,
        PersistenceError::Version => -6,
    }
}

/// 读取快应用 schedule.json；不存在返回 `Ok(None)`。
pub fn load_schedule() -> Result<Option<ScheduleFile>, i32> {
    persistence::load_schedule(&mut FsStore).map_err(map_error)
}

/// 删除快应用沙箱中的课表文件（含临时文件）。
pub fn clear_schedule() -> Result<(), i32> {
    unlink_path(SCHEDULE_PATH)?;
    let _ = unlink_path(SCHEDULE_TMP_PATH);
    Ok(())
}
