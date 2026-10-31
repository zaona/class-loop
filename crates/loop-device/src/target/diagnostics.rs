//! Bounded startup tracing through the context FS service; no heap or raw firmware calls.
//! Each record is synced and closed before the next UI call can fault.
use canopus_abi::{CanopusContext, FS_APPEND, FS_CREATE, FS_WRITE};
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicU32, Ordering},
};

const PATH: &[u8] = b"/data/offlinelog/loop-startup.log\0";
const MAX_RECORDS: u32 = 512;
const MAX_BYTES: i64 = 128 * 1024;
static SEQUENCE: AtomicU32 = AtomicU32::new(0);

struct Line {
    bytes: [u8; 192],
    len: usize,
}
impl Write for Line {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len + text.len();
        if end > self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

pub fn begin() {
    SEQUENCE.store(0, Ordering::Relaxed);
    mark("activate.begin", 0, 0);
}

/// `a` and `b` are opaque pointer/value tokens; tracing never dereferences them.
/// The quota is consumed even when IO fails, so normal UI refresh cannot retry forever.
pub fn mark(stage: &str, a: usize, b: usize) {
    let mut sequence = SEQUENCE.load(Ordering::Relaxed);
    loop {
        if sequence >= MAX_RECORDS {
            return;
        }
        match SEQUENCE.compare_exchange_weak(
            sequence,
            sequence + 1,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(current) => sequence = current,
        }
    }
    let Some(c) = crate::module::context() else {
        return;
    };
    let Some(fs) = (unsafe { c.fs.as_ref() }) else {
        return;
    };
    let ctx = (c as *const CanopusContext).cast();
    let ms = unsafe { c.time.as_ref() }.map_or(0, |time| unsafe { (time.monotonic_ms)(ctx) });
    let mut line = Line {
        bytes: [0; 192],
        len: 0,
    };
    if writeln!(
        line,
        "LOOP_STARTUP v={} target={} seq={} ms={} stage={} a={:#010x} b={:#010x}",
        env!("CARGO_PKG_VERSION"),
        option_env!("CANOPUS_TARGET_ID").unwrap_or("host"),
        sequence,
        ms,
        stage,
        a,
        b
    )
    .is_err()
    {
        return;
    }
    let fd = unsafe { (fs.open)(ctx, PATH.as_ptr().cast(), FS_WRITE | FS_CREATE | FS_APPEND) };
    if fd < 0 {
        return;
    }
    let size = unsafe { (fs.size)(ctx, fd) };
    if size >= 0 && size + line.len as i64 <= MAX_BYTES {
        let mut offset = 0;
        while offset < line.len {
            let written = unsafe {
                (fs.write)(
                    ctx,
                    fd,
                    line.bytes[offset..line.len].as_ptr().cast(),
                    (line.len - offset) as u32,
                )
            };
            if written <= 0 || written as usize > line.len - offset {
                break;
            }
            offset += written as usize;
        }
        let _ = unsafe { (fs.sync)(ctx, fd) };
    }
    let _ = unsafe { (fs.close)(ctx, fd) };
}
