use serde_json::Value;

use crate::astrobox::psys_host_v4::{device, interconnect, register};
use crate::state::{self, DeviceInfo};

pub const ROUTE_PACKAGE: &str = "top.zaona.loopimport";
pub const OUTGOING_FRAME_CAPACITY: usize = 48 * 1024;
pub const INCOMING_FRAME_CAPACITY: usize = 8192;

pub struct IncomingMessage {
    pub addr: String,
    pub package: String,
    pub payload: String,
}

pub async fn refresh_devices() {
    let devices = device::get_connected_device_list()
        .await
        .into_iter()
        .map(|item| DeviceInfo {
            addr: item.addr,
            name: item.name,
        })
        .collect::<Vec<_>>();
    state::with_state(|state| {
        state.devices = devices;
        if !state
            .devices
            .iter()
            .any(|item| item.addr == state.selected_addr)
        {
            state.selected_addr = state
                .devices
                .first()
                .map(|item| item.addr.clone())
                .unwrap_or_default();
        }
    });
    register_receivers().await;
}

pub async fn register_receivers() {
    let devices = state::snapshot().devices;
    for device in devices {
        let result =
            register::register_interconnect_recv(device.addr.clone(), ROUTE_PACKAGE.to_string())
                .await;
        if let Err(error) = result {
            tracing::warn!("failed to register receiver for {}: {}", device.addr, error);
        }
    }
}

pub async fn send(addr: &str, value: &Value) -> Result<(), String> {
    let frame = value.to_string();
    if frame.len() > OUTGOING_FRAME_CAPACITY {
        return Err("outgoing interconnect frame exceeds 49152 bytes".to_string());
    }
    interconnect::send_qaic_message(addr.to_string(), ROUTE_PACKAGE.to_string(), frame)
        .await
        .map_err(|error| format!("interconnect send failed: {error}"))
}

pub fn parse_event(raw: &str) -> Option<IncomingMessage> {
    if raw.len() > INCOMING_FRAME_CAPACITY * 2 {
        return None;
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    let addr = value
        .get("addr")
        .or_else(|| value.get("deviceAddr"))
        .and_then(Value::as_str)?
        .to_string();
    let package = value
        .get("pkgName")
        .or_else(|| value.get("package"))
        .or_else(|| value.get("pkg"))
        .and_then(Value::as_str)
        .unwrap_or(ROUTE_PACKAGE)
        .to_string();
    let payload = value
        .get("payloadHex")
        .and_then(Value::as_str)
        .and_then(decode_hex_utf8)
        .or_else(|| {
            value
                .get("payloadText")
                .or_else(|| value.get("data"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            value.get("payload").map(|item| match item {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
        })?;
    if payload.len() > INCOMING_FRAME_CAPACITY {
        return None;
    }
    Some(IncomingMessage {
        addr,
        package,
        payload,
    })
}

fn decode_hex_utf8(input: &str) -> Option<String> {
    if !input.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(input.len() / 2);
    for pair in input.as_bytes().chunks_exact(2) {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes.push(((high << 4) | low) as u8);
    }
    String::from_utf8(bytes).ok()
}
