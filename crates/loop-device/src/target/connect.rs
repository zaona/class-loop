//! All endpoint and timer callbacks run on the Supervisor owner; UI reload is deferred.
use super::{runtime::runtime, storage};
use alloc::vec::Vec;
use canopus_abi::{CONNECT_BINARY, CanopusContext, ContextV1, SVC_CONNECT, SVC_TIME, services_has};
use canopus_runtime::connect::{self, Delivery};
use core::{ffi::c_void, sync::atomic::Ordering};
use loop_protocol::{FRAME_MAX, Receiver};

struct Endpoint {
    ctx: *const CanopusContext,
    receiver: Receiver,
    reply: Option<Vec<u8>>,
    activity: u32,
}
static mut ENDPOINT: Option<Endpoint> = None;
fn endpoint() -> Option<&'static mut Endpoint> {
    // Serialized with the module's message and timer callbacks by the Supervisor.
    unsafe { (*core::ptr::addr_of_mut!(ENDPOINT)).as_mut() }
}
fn now(ctx: *const CanopusContext) -> u32 {
    unsafe { ((*(*ctx).time).monotonic_ms)(ctx.cast()) }
}
pub fn start() -> i32 {
    if endpoint().is_some() {
        return 0;
    }
    let Some(context) = crate::module::context() else {
        return -38;
    };
    let ctx = context as *const CanopusContext;
    if !unsafe { services_has(ctx.cast(), SVC_CONNECT, 1) } || context.connect.is_null() {
        return -38;
    }
    let time = unsafe { &*context.time };
    let id = unsafe { (time.timer_create)(ctx.cast(), 100, 1, tick, core::ptr::null_mut()) };
    if id < 0 {
        return id;
    }
    if unsafe { services_has(ctx.cast(), SVC_TIME, 2) } {
        let rc = unsafe { (time.timer_set_background)(ctx.cast(), id, 1) };
        if rc < 0 {
            unsafe { (time.timer_destroy)(ctx.cast(), id) };
            return rc;
        }
    }
    let rc = unsafe { (time.timer_start)(ctx.cast(), id) };
    if rc < 0 {
        unsafe { (time.timer_destroy)(ctx.cast(), id) };
        return rc;
    }
    unsafe {
        ENDPOINT = Some(Endpoint {
            ctx,
            receiver: Receiver::default(),
            reply: None,
            activity: now(ctx),
        });
    }
    if let Err(error) = connect::listen(ctx, on_message, core::ptr::null_mut()) {
        unsafe {
            ENDPOINT = None;
            (time.timer_destroy)(ctx.cast(), id);
        }
        return error;
    }
    0
}
pub fn abort() {
    if let Some(e) = endpoint() {
        e.receiver.reset();
        e.reply = None;
    }
}
unsafe extern "C" fn on_message(
    _ctx: *const ContextV1,
    kind: u32,
    data: *const c_void,
    len: u32,
    _user: *mut c_void,
) {
    if kind != CONNECT_BINARY || data.is_null() || len as usize > FRAME_MAX {
        return;
    }
    let Some(e) = endpoint() else {
        return;
    };
    let bytes = unsafe { core::slice::from_raw_parts(data.cast::<u8>(), len as usize) };
    if let Some(reply) = e.receiver.handle(bytes, |data| {
        let count = storage::save_schedule(data)?;
        runtime().schedule_dirty.store(true, Ordering::Release);
        Ok(count)
    }) {
        e.activity = now(e.ctx);
        e.reply = Some(reply);
        e.flush();
    }
}
unsafe extern "C" fn tick(_ctx: *const ContextV1, _id: i32, _user: *mut c_void) {
    if let Some(e) = endpoint() {
        if now(e.ctx).wrapping_sub(e.activity) >= 60_000 {
            e.receiver.reset();
            e.reply = None;
        }
        e.flush();
    }
}
impl Endpoint {
    fn flush(&mut self) {
        let Some(reply) = self.reply.as_ref() else {
            return;
        };
        match connect::send_binary(self.ctx, reply) {
            Ok(Delivery::Queued) => self.reply = None,
            Err(-16) => {} // Supervisor send queue is full; retry on the next owner timer tick.
            _ => {
                self.reply = None;
                self.receiver.reset();
            }
        }
    }
}
