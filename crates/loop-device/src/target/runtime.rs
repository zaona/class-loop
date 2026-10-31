//! 设备堆上的 Loop 核心与原子状态。

use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

use loop_core::LoopApp;

pub const APP_NONE: u32 = 0;
pub const APP_REGISTERED: u32 = 1;
pub const APP_FAILED: u32 = 3;

pub struct Core {
    pub app: LoopApp,
}

impl Core {
    unsafe fn initialize_at(pointer: *mut Self) {
        unsafe {
            core::ptr::addr_of_mut!((*pointer).app).write(LoopApp::default());
        }
    }
}

pub struct Runtime {
    pub schedule_dirty: AtomicBool,
    pub connect_error: AtomicI32,
    pub app_state: AtomicU32,
    pub app_error: AtomicI32,
    pub last_error: AtomicI32,
    pub timer_ticks: AtomicU32,
}

impl Runtime {
    const fn new() -> Self {
        Self {
            schedule_dirty: AtomicBool::new(true),
            connect_error: AtomicI32::new(-38),
            app_state: AtomicU32::new(APP_NONE),
            app_error: AtomicI32::new(0),
            last_error: AtomicI32::new(0),
            timer_ticks: AtomicU32::new(0),
        }
    }
}

static RUNTIME: Runtime = Runtime::new();
static CORE_PTR: AtomicUsize = AtomicUsize::new(0);
static CORE_LOCK: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);

pub fn prepare() {
    if READY.load(Ordering::Acquire) {
        return;
    }
    let mut pointer = CORE_PTR.load(Ordering::Acquire) as *mut Core;
    if pointer.is_null() {
        pointer = unsafe { alloc::alloc::alloc(core::alloc::Layout::new::<Core>()) }.cast();
        if pointer.is_null() {
            READY.store(false, Ordering::Release);
            return;
        }
        CORE_PTR.store(pointer as usize, Ordering::Release);
    }
    unsafe {
        Core::initialize_at(pointer);
    }
    CORE_LOCK.store(false, Ordering::Release);
    READY.store(true, Ordering::Release);
}

pub fn runtime() -> &'static Runtime {
    &RUNTIME
}

pub fn initialized() -> bool {
    READY.load(Ordering::Acquire)
}

pub fn with_core<R>(function: impl FnOnce(&mut Core) -> R) -> R {
    while CORE_LOCK
        .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        core::hint::spin_loop();
    }
    let pointer = CORE_PTR.load(Ordering::Acquire) as *mut Core;
    let result = unsafe { function(&mut *pointer) };
    CORE_LOCK.store(false, Ordering::Release);
    result
}

pub fn try_with_core<R>(function: impl FnOnce(&mut Core) -> R) -> Option<R> {
    if CORE_LOCK
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return None;
    }
    let pointer = CORE_PTR.load(Ordering::Acquire) as *mut Core;
    let result = unsafe { function(&mut *pointer) };
    CORE_LOCK.store(false, Ordering::Release);
    Some(result)
}
