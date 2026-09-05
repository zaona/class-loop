//! 覆盖课表 JSON 的只读文件系统适配。

use alloc::vec::Vec;
use core::ffi::c_void;

use canopus_target_private::{O_RDONLY, nuttx_close, nuttx_open, nuttx_read};
use loop_core::{
    ScheduleFile,
    persistence::{self, PersistenceError, Store},
};

const MAX_SCHEDULE_BYTES: usize = 64 * 1024;

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

fn read_bounded(path: &str, limit: usize) -> Result<Option<Vec<u8>>, i32> {
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

/// 优先覆盖文件，否则内置样例。返回 `(课表, 是否来自覆盖文件)`。
pub fn load_schedule() -> Result<(ScheduleFile, bool), i32> {
    persistence::load_schedule(&mut FsStore).map_err(map_error)
}
