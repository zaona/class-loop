//! Canopus 模块描述符与生命周期回调。
//!
//! `prepare` 跑在 stock `insmod` 的约 7.9 KiB 栈上，必须无分配；
//! 真正的课表初始化推迟到 Manager 下发 `activate` 之后。

use canopus_abi::*;
use canopus_runtime::{status_put_u32, status_writer_publish};
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static RESIDENT: AtomicBool = AtomicBool::new(false);
static LAST_ERROR: AtomicU32 = AtomicU32::new(0);
static CONTEXT: AtomicPtr<CanopusContext> = AtomicPtr::new(core::ptr::null_mut());
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(any(feature = "device", test))]
pub(crate) fn context() -> Option<&'static CanopusContext> {
    // The Supervisor owns the context until reboot; Loop stays resident.
    unsafe { CONTEXT.load(Ordering::Acquire).as_ref() }
}

/// 状态流魔数：ASCII "LOOP"。
const MAGIC: u32 = 0x4C4F_4F50;

const fn pack<const N: usize>(value: &[u8]) -> [u8; N] {
    let mut output = [0; N];
    let mut index = 0;
    while index < value.len() && index < N {
        output[index] = value[index];
        index += 1;
    }
    output
}

#[cfg(feature = "device")]
const TARGET_ID: &[u8] = canopus_target_private::TARGET_ID.as_bytes();
#[cfg(not(feature = "device"))]
const TARGET_ID: &[u8] = b"host-test";

#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_prepare(ctx: *const ContextV1) -> i32 {
    CONTEXT.store(ctx as *mut CanopusContext, Ordering::Release);
    ACTIVE.store(false, Ordering::Release);
    RESIDENT.store(false, Ordering::Release);
    LAST_ERROR.store(0, Ordering::Release);
    // insmod 栈极小：此处只清状态，不做 JSON / 堆分配。
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_activate(ctx: *const ContextV1) -> i32 {
    CONTEXT.store(ctx as *mut CanopusContext, Ordering::Release);
    #[cfg(feature = "device")]
    let result = {
        if required_services(ctx) {
            crate::target::diagnostics::begin();
            crate::target::prepare();
            let rc = crate::target::activate();
            crate::target::diagnostics::mark("activate.end", rc as usize, 0);
            rc
        } else {
            -38
        }
    };
    #[cfg(not(feature = "device"))]
    let result = 0;
    if result == 0 {
        ACTIVE.store(true, Ordering::Release);
        RESIDENT.store(true, Ordering::Release);
    } else {
        LAST_ERROR.store(result as u32, Ordering::Release);
    }
    result
}

#[cfg(feature = "device")]
fn required_services(ctx: *const ContextV1) -> bool {
    let Some(c) = context() else { return false };
    [SVC_FS, SVC_MEM, SVC_TIME, SVC_LAUNCHER]
        .iter()
        .all(|&group| unsafe { services_has(ctx, group, 1) })
        && !c.fs.is_null()
        && !c.mem.is_null()
        && !c.time.is_null()
        && !c.launcher.is_null()
}

#[cfg(any(feature = "device", test))]
pub(crate) fn register_native_pages(
    label: &[u8],
    icon: &[u8],
    pages: &[*mut core::ffi::c_void],
) -> Result<u16, i32> {
    let c = context().ok_or(-38)?;
    let ctx = (c as *const CanopusContext).cast::<ContextV1>();
    if !unsafe { services_has(ctx, SVC_LAUNCHER, 1) } || c.launcher.is_null() {
        return Err(-38);
    }
    let entry = AppEntryV1 {
        struct_size: core::mem::size_of::<AppEntryV1>() as u32,
        label: label.as_ptr().cast(),
        icon: icon.as_ptr().cast(),
        pages: if pages.is_empty() {
            core::ptr::null()
        } else {
            pages.as_ptr()
        },
        page_count: pages.len() as u32,
    };
    let id = unsafe { ((*c.launcher).register_app)(ctx, &entry) };
    if id > 0 && id <= u8::MAX as i32 {
        Ok(id as u16)
    } else {
        Err(if id < 0 { id } else { -103 })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_deactivate(_ctx: *const ContextV1) -> i32 {
    if RESIDENT.load(Ordering::Acquire) {
        RESULT_REBOOT_REQUIRED as i32
    } else {
        ACTIVE.store(false, Ordering::Release);
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_stop(ctx: *const ContextV1) -> i32 {
    canopus_mod_deactivate(ctx)
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_query(writer: *mut StatusWriterV1) -> i32 {
    if writer.is_null() {
        return -1;
    }
    let writer = unsafe { &mut *writer };
    unsafe {
        if !status_put_u32(writer, MAGIC)
            || !status_put_u32(writer, ACTIVE.load(Ordering::Acquire) as u32)
            || !status_put_u32(writer, RESIDENT.load(Ordering::Acquire) as u32)
            || !status_put_u32(writer, LAST_ERROR.load(Ordering::Acquire))
        {
            return -1;
        }
    }
    #[cfg(feature = "device")]
    for value in crate::target::query_status() {
        if !unsafe { status_put_u32(writer, value) } {
            return -1;
        }
    }
    status_writer_publish(writer);
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_publish_native_app(_ctx: *const ContextV1) -> i32 {
    -103
}

#[unsafe(no_mangle)]
pub extern "C" fn canopus_mod_publish_native_app_stage(_ctx: *const ContextV1, stage: u32) -> i32 {
    #[cfg(feature = "device")]
    crate::target::diagnostics::mark("publish.begin", stage as usize, 0);
    #[cfg(feature = "device")]
    let result = crate::target::native_app::install_stage(stage)
        .map(|()| 0)
        .unwrap_or_else(|error| error);
    #[cfg(not(feature = "device"))]
    let result = if matches!(stage, 1 | 2) { 0 } else { -103 };
    #[cfg(feature = "device")]
    crate::target::diagnostics::mark("publish.end", stage as usize, result as usize);
    if result != 0 {
        LAST_ERROR.store(result as u32, Ordering::Release);
    }
    result
}

/// 模块入口：Manager / verifier / 链接器均通过此符号发现模块。
#[unsafe(no_mangle)]
pub static canopus_module_descriptor: ModuleDescriptorV1 = ModuleDescriptorV1 {
    struct_size: core::mem::size_of::<ModuleDescriptorV1>() as u32,
    abi_major: ABI_MAJOR,
    abi_minor: ABI_MINOR,
    flags: FLAG_HAS_NATIVE_APP
        | FLAG_NATIVE_APP_STANDALONE
        | FLAG_REGISTERS_LAUNCHER_ENTRY
        | FLAG_REQUIRES_UI_DISPATCHER
        | FLAG_APP_UNREGISTER_REBOOT_REQUIRED,
    // 与手机插件共用模块 ID；必须与 Canopus.toml 的 module.id 一致。
    module_id: pack(loop_protocol::MODULE_ID.as_bytes()),
    module_version: pack(env!("CARGO_PKG_VERSION").as_bytes()),
    build_id: pack(concat!("loop-", env!("CARGO_PKG_VERSION")).as_bytes()),
    target_id: pack(TARGET_ID),
    prepare: Some(canopus_mod_prepare),
    activate: Some(canopus_mod_activate),
    deactivate: Some(canopus_mod_deactivate),
    stop: Some(canopus_mod_stop),
    query: Some(canopus_mod_query),
    publish_native_app: Some(canopus_mod_publish_native_app),
    publish_native_app_stage: Some(canopus_mod_publish_native_app_stage),
};

#[unsafe(no_mangle)]
pub extern "C" fn canopus_module_descriptor_ptr() -> *const ModuleDescriptorV1 {
    &canopus_module_descriptor
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopus_runtime::status_writer_init;
    static REGISTER_RESULT: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(0);

    unsafe extern "C" fn register_app(_ctx: *const ContextV1, entry: *const AppEntryV1) -> i32 {
        let entry = unsafe { &*entry };
        assert!(matches!(entry.page_count, 0 | 5));
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr(entry.label) }.to_bytes(),
            b"Loop"
        );
        assert_eq!(entry.pages.is_null(), entry.page_count == 0);
        REGISTER_RESULT.load(Ordering::Relaxed)
    }

    #[test]
    fn native_registration_uses_supervisor_id_and_propagates_service_errors() {
        let _lock = TEST_LOCK.lock().unwrap();
        let mut services = ServicesV1Head {
            struct_size: core::mem::size_of::<ServicesV1Head>() as u32,
            group_version: [0; SVC_GROUP_COUNT as usize],
        };
        let launcher = CanopusLauncherObj { register_app };
        let mut ctx: CanopusContext = unsafe { core::mem::zeroed() };
        ctx.struct_size = core::mem::size_of::<CanopusContext>() as u32;
        ctx.services = core::ptr::addr_of!(services).cast();
        ctx.launcher = &launcher;
        canopus_mod_prepare(core::ptr::addr_of!(ctx).cast());
        let pages = [core::ptr::dangling_mut::<core::ffi::c_void>(); 5];
        let register = || {
            register_native_pages(
                b"Loop\0",
                b"/data/canopus/m.canopus_loop/appicon_loop.bin\0",
                &pages,
            )
        };
        assert_eq!(register(), Err(-38)); // Missing service version.
        services.group_version[SVC_LAUNCHER as usize] = 1;
        // Keep the mutation visible through the FFI service-table pointer.
        core::hint::black_box(&services);
        REGISTER_RESULT.store(0xD7, Ordering::Relaxed);
        assert_eq!(register(), Ok(0xD7));
        assert_eq!(
            register_native_pages(b"Loop\0", b"/data/canopus/m.canopus_loop/appicon_loop.bin\0", &[]),
            Ok(0xD7)
        );
        REGISTER_RESULT.store(-16, Ordering::Relaxed);
        assert_eq!(register(), Err(-16));
        REGISTER_RESULT.store(0, Ordering::Relaxed);
        assert_eq!(register(), Err(-103));
        canopus_mod_prepare(core::ptr::null());
        assert_eq!(register(), Err(-38));
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn query_publishes_host_lifecycle_status() {
        let _lock = TEST_LOCK.lock().unwrap();
        let mut bytes = [0u8; 16];
        let mut writer = StatusWriterV1 {
            buf: core::ptr::null_mut(),
            capacity: 0,
            used: 0,
            dropped: 0,
            snap: SnapshotV1 { sequence: 0 },
        };
        assert!(unsafe { status_writer_init(&mut writer, bytes.as_mut_ptr(), bytes.len() as u32) });
        assert_eq!(canopus_mod_prepare(core::ptr::null()), 0);
        assert_eq!(canopus_mod_activate(core::ptr::null()), 0);
        assert_eq!(canopus_mod_query(&mut writer), 0);
        assert_eq!(read_u32(&bytes, 0), MAGIC);
        assert_eq!(read_u32(&bytes, 4), 1);
        assert_eq!(read_u32(&bytes, 8), 1);
    }
}
