#![cfg_attr(feature = "device", no_std)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Loop 设备端 staticlib。
//!
//! - 无 `device` feature：主机侧可测模块生命周期查询。
//! - 有 `device` feature：绑定具体固件 target，走 NuttX / LVGL / launcher。

#[cfg(any(feature = "device", test))]
extern crate alloc;

#[cfg(feature = "device")]
mod allocator {
    use core::alloc::{GlobalAlloc, Layout};

    /// 在用户指针前保存服务堆基址，满足 Rust 的任意对齐要求。
    pub struct FirmwareAllocator;

    unsafe impl GlobalAlloc for FirmwareAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let header = core::mem::size_of::<usize>();
            let total = match layout
                .size()
                .checked_add(layout.align())
                .and_then(|size| size.checked_add(header))
            {
                Some(total) if total <= u32::MAX as usize => total,
                _ => return core::ptr::null_mut(),
            };
            let Some(ctx) = crate::module::context() else {
                return core::ptr::null_mut();
            };
            if ctx.mem.is_null() {
                return core::ptr::null_mut();
            }
            let raw = unsafe {
                ((*ctx.mem).alloc)(
                    (ctx as *const canopus_abi::CanopusContext).cast(),
                    total as u32,
                )
            }
            .cast::<u8>();
            if raw.is_null() {
                return raw;
            }
            let start = unsafe { raw.add(header) } as usize;
            let aligned = (start + layout.align() - 1) & !(layout.align() - 1);
            let output = aligned as *mut u8;
            unsafe { output.sub(header).cast::<usize>().write(raw as usize) };
            output
        }

        unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
            if pointer.is_null() {
                return;
            }
            let header = core::mem::size_of::<usize>();
            let raw =
                unsafe { pointer.sub(header).cast::<usize>().read() } as *mut core::ffi::c_void;
            if let Some(ctx) = crate::module::context() {
                unsafe {
                    ((*ctx.mem).free)((ctx as *const canopus_abi::CanopusContext).cast(), raw)
                };
            }
        }
    }
}

#[cfg(feature = "device")]
#[global_allocator]
static ALLOCATOR: allocator::FirmwareAllocator = allocator::FirmwareAllocator;

mod module;

#[cfg(all(test, not(feature = "device")))]
// The host harness exercises commit/trace paths; the native app uses the other
// storage entry points. Keep dead-code checks enabled in the device build.
#[allow(dead_code)]
#[path = "target/storage.rs"]
mod storage_host;

#[cfg(all(test, not(feature = "device")))]
#[path = "target/diagnostics.rs"]
mod diagnostics_host;

#[cfg(all(test, not(feature = "device")))]
#[path = "target/clock.rs"]
mod clock_host;

#[cfg(feature = "device")]
mod target;

pub use module::*;

#[cfg(all(feature = "device", not(test)))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
