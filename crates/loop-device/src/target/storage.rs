//! CanopusConnect 导入和读取：文件均属于 Supervisor 分配的模块私有目录。

use alloc::vec::Vec;
use core::ffi::c_void;

use canopus_abi::{
    CanopusContext, CanopusFsObj, ContextV1, FS_CREATE, FS_READ, FS_TRUNC, FS_WRITE,
};
use loop_core::{
    ScheduleFile,
    persistence::{self, PersistenceError, SCHEDULE_PATH, Store},
};

const MAX_SCHEDULE_BYTES: usize = loop_protocol::SCHEDULE_MAX;
const SCHEDULE_TMP_PATH: &str = "schedule.json.tmp";

#[repr(align(4))]
struct LauncherIcon([u8; include_bytes!("../../assets/appicon_loop.bin").len()]);
static LAUNCHER_ICON: LauncherIcon = LauncherIcon(*include_bytes!("../../assets/appicon_loop.bin"));

pub struct FsStore;

fn file_system() -> Result<(&'static CanopusFsObj, *const ContextV1), i32> {
    let ctx = crate::module::context().ok_or(-38)?;
    let fs = unsafe { ctx.fs.as_ref() }.ok_or(-38)?;
    Ok((fs, (ctx as *const CanopusContext).cast()))
}

fn fs_open(path: *const u8) -> i32 {
    match file_system() {
        Ok((fs, ctx)) => unsafe { (fs.open)(ctx, path.cast(), FS_READ) },
        Err(error) => error,
    }
}

fn fs_close(fd: i32) -> i32 {
    match file_system() {
        Ok((fs, ctx)) => unsafe { (fs.close)(ctx, fd) },
        Err(error) => error,
    }
}

fn fs_read(fd: i32, data: *mut c_void, size: u32) -> i32 {
    match file_system() {
        Ok((fs, ctx)) => unsafe { (fs.read)(ctx, fd, data, size) },
        Err(error) => error,
    }
}

fn fs_unlink(path: *const u8) -> i32 {
    match file_system() {
        Ok((fs, ctx)) => unsafe { (fs.remove)(ctx, path.cast()) },
        Err(error) => error,
    }
}

/// 图标随 ELF 分发；旧表盘安装器不再负责复制应用资源。
pub fn ensure_launcher_icon() -> Result<(), i32> {
    const ICON: &[u8] = &LAUNCHER_ICON.0;
    const PATH: &[u8] = b"appicon_loop.bin\0";
    let (fs, ctx) = file_system()?;
    let fd = unsafe { (fs.open)(ctx, PATH.as_ptr().cast(), FS_READ) };
    let mut identical = fd >= 0 && unsafe { (fs.size)(ctx, fd) } == ICON.len() as i64;
    if fd >= 0 {
        let mut offset = 0;
        let mut chunk = [0u8; 256];
        while identical && offset < ICON.len() {
            let size = chunk.len().min(ICON.len() - offset);
            let read = unsafe { (fs.read)(ctx, fd, chunk.as_mut_ptr().cast(), size as u32) };
            identical = read == size as i32 && chunk[..size] == ICON[offset..offset + size];
            offset += size;
        }
        let result = unsafe { (fs.close)(ctx, fd) };
        if result < 0 {
            return Err(result);
        }
    }
    if identical {
        return Ok(());
    }
    let fd = unsafe { (fs.open)(ctx, PATH.as_ptr().cast(), FS_WRITE | FS_CREATE | FS_TRUNC) };
    if fd < 0 {
        return Err(fd);
    }
    let mut offset = 0;
    let mut result = Ok(());
    while offset < ICON.len() {
        let written = unsafe {
            (fs.write)(
                ctx,
                fd,
                ICON[offset..].as_ptr().cast(),
                (ICON.len() - offset) as u32,
            )
        };
        if written <= 0 {
            result = Err(if written < 0 { written } else { -5 });
            break;
        }
        offset += written as usize;
    }
    if result.is_ok() {
        let sync = unsafe { (fs.sync)(ctx, fd) };
        if sync < 0 {
            result = Err(sync);
        }
    }
    let close = unsafe { (fs.close)(ctx, fd) };
    result.and(if close < 0 { Err(close) } else { Ok(()) })
}

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
    let fd = fs_open(path.as_ptr());
    if fd < 0 {
        return if fd == -2 { Ok(None) } else { Err(fd) };
    }
    let mut output = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let count = fs_read(fd, chunk.as_mut_ptr().cast::<c_void>(), chunk.len() as u32);
        if count < 0 {
            let _ = fs_close(fd);
            return Err(count);
        }
        if count == 0 {
            break;
        }
        if output.len() + count as usize > limit {
            let _ = fs_close(fd);
            return Err(-2);
        }
        output.extend_from_slice(&chunk[..count as usize]);
    }
    let result = fs_close(fd);
    if result < 0 {
        return Err(result);
    }
    Ok(Some(output))
}

fn unlink_path(path: &str) -> Result<(), i32> {
    let path = c_path(path)?;
    let result = fs_unlink(path.as_ptr());
    if result < 0 && result != -2 {
        return Err(result);
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

/// 读取私有 schedule.json；不存在返回 `Ok(None)`。
pub fn load_schedule() -> Result<Option<ScheduleFile>, i32> {
    persistence::load_schedule(&mut FsStore).map_err(map_error)
}

/// 删除模块私有课表（含临时文件）。
pub fn clear_schedule() -> Result<(), i32> {
    unlink_path(SCHEDULE_PATH)?;
    let _ = unlink_path(SCHEDULE_TMP_PATH);
    Ok(())
}

/// Only acknowledge a complete, valid schedule after sync, close and atomic rename succeed.
pub fn save_schedule(bytes: &[u8]) -> Result<u32, loop_protocol::Status> {
    use loop_protocol::Status;
    let schedule = persistence::parse_schedule_bytes(bytes).map_err(|_| Status::Schedule)?;
    if !persistence::valid_import(&schedule) {
        return Err(Status::Schedule);
    }
    let count = schedule.courses.len() as u32;
    // Release the parsed schedule before writing; the UI reloads on its own next tick.
    drop(schedule);
    let write = || -> Result<(), i32> {
        let (fs, ctx) = file_system()?;
        let temporary = c_path(SCHEDULE_TMP_PATH)?;
        let target = c_path(SCHEDULE_PATH)?;
        let fd = unsafe {
            (fs.open)(
                ctx,
                temporary.as_ptr().cast(),
                FS_WRITE | FS_CREATE | FS_TRUNC,
            )
        };
        if fd < 0 {
            return Err(fd);
        }
        let mut offset = 0;
        let mut result = Ok(());
        while offset < bytes.len() {
            let n = unsafe {
                (fs.write)(
                    ctx,
                    fd,
                    bytes[offset..].as_ptr().cast(),
                    (bytes.len() - offset) as u32,
                )
            };
            if n <= 0 {
                result = Err(if n < 0 { n } else { -5 });
                break;
            }
            offset += n as usize;
        }
        if result.is_ok() {
            let rc = unsafe { (fs.sync)(ctx, fd) };
            if rc < 0 {
                result = Err(rc);
            }
        }
        let close = unsafe { (fs.close)(ctx, fd) };
        result?;
        if close < 0 {
            return Err(close);
        }
        let rc = unsafe { (fs.rename)(ctx, temporary.as_ptr().cast(), target.as_ptr().cast()) };
        if rc < 0 { Err(rc) } else { Ok(()) }
    };
    if write().is_err() {
        let _ = unlink_path(SCHEDULE_TMP_PATH);
        return Err(Status::Io);
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopus_abi::{CanopusFsDirent, CanopusFsStat};
    use core::ffi::c_char;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Disk {
        live: Vec<u8>,
        temporary: Vec<u8>,
        fault: u8,
        synced: bool,
        closed: bool,
    }
    static DISK: Mutex<Disk> = Mutex::new(Disk {
        live: Vec::new(),
        temporary: Vec::new(),
        fault: 0,
        synced: false,
        closed: false,
    });
    const VALID: &[u8] =
        br#"{"version":1,"term":{"name":"Test","start_date":"2026-08-31"},"courses":[]}"#;
    unsafe extern "C" fn open(_: *const ContextV1, path: *const c_char, _: u32) -> i32 {
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr(path) }.to_bytes(),
            b"schedule.json.tmp"
        );
        let mut d = DISK.lock().unwrap();
        d.temporary.clear();
        d.closed = false;
        d.synced = false;
        7
    }
    unsafe extern "C" fn write(_: *const ContextV1, _: i32, data: *const c_void, size: u32) -> i32 {
        let mut d = DISK.lock().unwrap();
        if d.fault == 1 && !d.temporary.is_empty() {
            return -5;
        }
        let count = (size as usize).min(7); // Exercise real short-write handling.
        d.temporary
            .extend_from_slice(unsafe { core::slice::from_raw_parts(data.cast::<u8>(), count) });
        count as i32
    }
    unsafe extern "C" fn sync(_: *const ContextV1, _: i32) -> i32 {
        let mut d = DISK.lock().unwrap();
        d.synced = true;
        if d.fault == 2 { -5 } else { 0 }
    }
    unsafe extern "C" fn close(_: *const ContextV1, _: i32) -> i32 {
        let mut d = DISK.lock().unwrap();
        d.closed = true;
        if d.fault == 3 { -5 } else { 0 }
    }
    unsafe extern "C" fn remove(_: *const ContextV1, path: *const c_char) -> i32 {
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr(path) }.to_bytes(),
            b"schedule.json.tmp"
        );
        DISK.lock().unwrap().temporary.clear();
        0
    }
    unsafe extern "C" fn rename(
        _: *const ContextV1,
        from: *const c_char,
        to: *const c_char,
    ) -> i32 {
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr(from) }.to_bytes(),
            b"schedule.json.tmp"
        );
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr(to) }.to_bytes(),
            b"schedule.json"
        );
        let mut d = DISK.lock().unwrap();
        assert!(d.synced && d.closed);
        if d.fault == 4 {
            return -5;
        }
        d.live = core::mem::take(&mut d.temporary);
        0
    }
    unsafe extern "C" fn read(_: *const ContextV1, _: i32, _: *mut c_void, _: u32) -> i32 {
        -38
    }
    unsafe extern "C" fn seek(_: *const ContextV1, _: i32, _: i64, _: i32) -> i64 {
        -38
    }
    unsafe extern "C" fn size(_: *const ContextV1, _: i32) -> i64 {
        -38
    }
    unsafe extern "C" fn stat(_: *const ContextV1, _: *const c_char, _: *mut CanopusFsStat) -> i32 {
        -38
    }
    unsafe extern "C" fn opendir(_: *const ContextV1, _: *const c_char) -> *mut c_void {
        core::ptr::null_mut()
    }
    unsafe extern "C" fn readdir(
        _: *const ContextV1,
        _: *mut c_void,
        _: *mut CanopusFsDirent,
    ) -> i32 {
        -38
    }
    unsafe extern "C" fn closedir(_: *const ContextV1, _: *mut c_void) -> i32 {
        -38
    }
    unsafe extern "C" fn read_all(
        _: *const ContextV1,
        _: *const c_char,
        _: *mut c_void,
        _: u32,
    ) -> i32 {
        -38
    }
    unsafe extern "C" fn write_all(
        _: *const ContextV1,
        _: *const c_char,
        _: *const c_void,
        _: u32,
    ) -> i32 {
        -38
    }
    unsafe extern "C" fn get_path(_: *const ContextV1, _: *mut c_char, _: u32) -> i32 {
        -38
    }
    fn fs_object() -> CanopusFsObj {
        CanopusFsObj {
            open,
            read,
            write,
            seek,
            tell: size,
            size,
            sync,
            close,
            remove,
            rename,
            stat,
            exists: remove,
            mkdir: remove,
            opendir,
            readdir,
            closedir,
            read_all,
            write_all,
            get_data_path: get_path,
            rmdir: remove,
        }
    }
    #[test]
    fn connect_commit_preserves_old_schedule_on_every_io_failure() {
        let _lock = crate::module::TEST_LOCK.lock().unwrap();
        let fs = fs_object();
        let mut ctx: CanopusContext = unsafe { core::mem::zeroed() };
        ctx.fs = &fs;
        crate::module::canopus_mod_prepare(core::ptr::addr_of!(ctx).cast());
        for fault in 1..=4 {
            *DISK.lock().unwrap() = Disk {
                live: b"previous schedule".to_vec(),
                fault,
                ..Disk::default()
            };
            assert_eq!(save_schedule(VALID), Err(loop_protocol::Status::Io));
            let d = DISK.lock().unwrap();
            assert_eq!(d.live, b"previous schedule");
            assert!(d.temporary.is_empty());
            assert!(d.closed);
        }
        *DISK.lock().unwrap() = Disk {
            live: b"previous schedule".to_vec(),
            ..Disk::default()
        };
        assert_eq!(
            save_schedule(b"invalid json"),
            Err(loop_protocol::Status::Schedule)
        );
        assert_eq!(DISK.lock().unwrap().live, b"previous schedule");
        let impossible_date =
            br#"{"version":1,"term":{"name":"Test","start_date":"2026-02-29"},"courses":[]}"#;
        assert_eq!(
            save_schedule(impossible_date),
            Err(loop_protocol::Status::Schedule)
        );
        assert_eq!(DISK.lock().unwrap().live, b"previous schedule");
        // Exercise the actual phone protocol through to atomic device storage.
        let mut sender = loop_protocol::Sender::new(91, VALID.to_vec()).unwrap();
        let mut receiver = loop_protocol::Receiver::default();
        loop {
            let reply = receiver.handle(sender.current(), save_schedule).unwrap();
            match sender.on_reply(&reply).unwrap() {
                loop_protocol::Progress::Complete(count) => {
                    assert_eq!(count, 0);
                    break;
                }
                loop_protocol::Progress::Send(_) => {}
                loop_protocol::Progress::Ignore => panic!("lost schedule transfer"),
            }
        }
        assert_eq!(DISK.lock().unwrap().live, VALID);
        crate::module::canopus_mod_prepare(core::ptr::null());
    }

    unsafe extern "C" fn trace_open(_: *const ContextV1, path: *const c_char, flags: u32) -> i32 {
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr(path) }.to_bytes(),
            b"/data/offlinelog/loop-startup.log"
        );
        assert_eq!(flags, FS_WRITE | FS_CREATE | canopus_abi::FS_APPEND);
        let mut d = DISK.lock().unwrap();
        d.closed = false;
        d.synced = false;
        7
    }
    unsafe extern "C" fn trace_size(_: *const ContextV1, _: i32) -> i64 {
        let d = DISK.lock().unwrap();
        if d.fault == 5 {
            128 * 1024
        } else {
            d.temporary.len() as i64
        }
    }

    #[test]
    fn startup_trace_preserves_previous_boot_syncs_short_writes_and_stops_at_limits() {
        let _lock = crate::module::TEST_LOCK.lock().unwrap();
        let fs = CanopusFsObj {
            open: trace_open,
            size: trace_size,
            ..fs_object()
        };
        let mut ctx: CanopusContext = unsafe { core::mem::zeroed() };
        ctx.fs = &fs;
        crate::module::canopus_mod_prepare(core::ptr::addr_of!(ctx).cast());
        *DISK.lock().unwrap() = Disk {
            temporary: b"previous crash evidence\n".to_vec(),
            ..Disk::default()
        };
        crate::diagnostics_host::begin();
        crate::diagnostics_host::mark("container.before", 0x1234, 0x5678);
        {
            let d = DISK.lock().unwrap();
            let text = core::str::from_utf8(&d.temporary).unwrap();
            assert!(text.starts_with("previous crash evidence\n"));
            assert!(text.contains(concat!("v=", env!("CARGO_PKG_VERSION"))));
            assert!(text.contains("stage=container.before a=0x00001234 b=0x00005678\n"));
            assert!(d.synced && d.closed);
        }
        for fault in [1, 2, 3, 5] {
            DISK.lock().unwrap().fault = fault;
            let previous = DISK.lock().unwrap().temporary.clone();
            crate::diagnostics_host::mark("io.failure", fault as usize, 0);
            let d = DISK.lock().unwrap();
            assert!(d.closed);
            if matches!(fault, 1 | 5) {
                assert_eq!(d.temporary, previous);
            }
        }
        DISK.lock().unwrap().fault = 0;
        for _ in 0..512 {
            crate::diagnostics_host::mark("budget", 0, 0);
        }
        let previous = DISK.lock().unwrap().temporary.clone();
        crate::diagnostics_host::mark("after.budget", 0, 0);
        assert_eq!(DISK.lock().unwrap().temporary, previous);
        crate::module::canopus_mod_prepare(core::ptr::null());
    }
}
