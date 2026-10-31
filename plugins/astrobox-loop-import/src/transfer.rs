//! No host call is awaited while holding PENDING; Connect replies may be concurrent.
use crate::{astrobox::psys_host_v4::timer, connect, publish, state};
use loop_protocol::{Frame, MODULE_ID, Progress, Sender};
use serde_json::Value;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU32, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

struct Pending {
    addr: String,
    id: u32,
    sender: Sender,
    timer: Option<u64>,
    timer_epoch: u32,
    retries: u8,
}
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
const RETRY_MS: u64 = 5_000;
const PREFIX: &str = "loop-connect:";
fn lock() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}
fn new_id() -> u32 {
    static NEXT: OnceLock<AtomicU32> = OnceLock::new();
    let counter = NEXT.get_or_init(|| {
        AtomicU32::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u32,
        )
    });
    loop {
        let id = counter.fetch_add(1, Ordering::Relaxed);
        if id != 0 {
            return id;
        }
    }
}
fn token(id: u32, frame: &[u8], epoch: u32) -> String {
    let f = Frame::decode(frame).unwrap();
    format!("{PREFIX}{id}:{}:{}:{epoch}", f.op, f.value)
}
fn arm(p: &mut Pending) {
    if let Some(id) = p.timer.take() {
        timer::clear_timer(id);
    }
    p.timer_epoch = p.timer_epoch.wrapping_add(1);
    p.timer = Some(timer::set_timeout(
        RETRY_MS,
        &token(p.id, p.sender.current(), p.timer_epoch),
    ));
}
fn finish(id: u32, message: String) {
    let mut guard = lock();
    if guard.as_ref().is_none_or(|p| p.id != id) {
        return;
    }
    if let Some(timer) = guard.as_mut().and_then(|p| p.timer.take()) {
        timer::clear_timer(timer);
    }
    *guard = None;
    state::with_state(|s| {
        s.busy = false;
        s.status = message;
    });
}
async fn dispatch(addr: String, id: u32, frame: Vec<u8>) -> Result<(), String> {
    let result = connect::send(&addr, &frame).await;
    if let Err(error) = &result {
        // A concurrent reply may already have advanced or completed the transfer.
        let current = lock()
            .as_ref()
            .is_some_and(|p| p.id == id && p.sender.current() == frame);
        if current {
            finish(id, format!("推送失败：{error}"));
            let _ = connect::send(
                &addr,
                &loop_protocol::request(loop_protocol::ABORT, id, 0, &[]),
            )
            .await;
        }
    }
    result
}
pub async fn start(addr: &str, schedule: Value) -> Result<(), String> {
    if addr.is_empty() {
        return Err("请选择已连接设备".into());
    }
    publish::validate_schedule(&schedule)?;
    let id = new_id();
    let sender = Sender::new(id, schedule.to_string().into_bytes())
        .map_err(|_| "课表超过 40960 字节上限".to_string())?;
    {
        let mut guard = lock();
        if guard.is_some() {
            return Err("已有推送进行中".into());
        }
        *guard = Some(Pending {
            addr: addr.into(),
            id,
            sender,
            timer: None,
            timer_epoch: 0,
            retries: 0,
        });
    }
    state::with_state(|s| {
        s.busy = true;
        s.status = "正在连接手表上的 Loop…".into();
    });
    if let Err(error) = connect::listen(addr).await {
        finish(
            id,
            format!("无法连接 Loop：{error}。请确认模块已启用且固件支持 CanopusConnect。"),
        );
        return Err(error);
    }
    let frame = {
        let mut guard = lock();
        let Some(p) = guard.as_mut().filter(|p| p.id == id) else {
            return Ok(());
        };
        arm(p);
        p.sender.current().to_vec()
    };
    dispatch(addr.into(), id, frame).await
}
pub async fn on_frame(addr: &str, module: &str, bytes: &[u8]) {
    if module != MODULE_ID {
        return;
    }
    let (id, next) = {
        let mut guard = lock();
        let Some(p) = guard.as_mut().filter(|p| p.addr == addr) else {
            return;
        };
        let id = p.id;
        match p.sender.on_reply(bytes) {
            Ok(Progress::Ignore) => return,
            Ok(Progress::Complete(count)) => {
                drop(guard);
                finish(
                    id,
                    format!("推送成功，共 {count} 门课。打开 Loop 即可使用。"),
                );
                return;
            }
            Err(error) => {
                drop(guard);
                finish(id, format!("手表拒绝课表：{error:?}"));
                let _ = connect::send(
                    addr,
                    &loop_protocol::request(loop_protocol::ABORT, id, 0, &[]),
                )
                .await;
                return;
            }
            Ok(Progress::Send(frame)) => {
                p.retries = 0;
                arm(p);
                (id, frame)
            }
        }
    };
    state::with_state(|s| s.status = "正在通过 CanopusConnect 推送课表…".into());
    let _ = dispatch(addr.into(), id, next).await;
}
pub async fn on_timer(payload: &str) {
    if !payload.starts_with(PREFIX) {
        return;
    }
    let (addr, id, frame) = {
        let mut guard = lock();
        let Some(p) = guard.as_mut() else {
            return;
        };
        if payload != token(p.id, p.sender.current(), p.timer_epoch) {
            return;
        }
        if p.retries >= 3 {
            let (id, addr) = (p.id, p.addr.clone());
            drop(guard);
            finish(id, "手表响应超时，导入未确认。可重新推送课表。".into());
            let _ = connect::send(
                &addr,
                &loop_protocol::request(loop_protocol::ABORT, id, 0, &[]),
            )
            .await;
            return;
        }
        p.retries += 1;
        arm(p);
        (p.addr.clone(), p.id, p.sender.current().to_vec())
    };
    let _ = dispatch(addr, id, frame).await;
}
