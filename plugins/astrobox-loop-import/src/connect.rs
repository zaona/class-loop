use crate::astrobox::psys_host_v4::{
    canopusconnect::{self, Payload},
    device,
};
use crate::state::{self, DeviceInfo};
use loop_protocol::{FRAME_MAX, MODULE_ID};

pub async fn refresh_devices() {
    let devices = device::get_connected_device_list()
        .await
        .into_iter()
        .map(|d| DeviceInfo {
            addr: d.addr,
            name: d.name,
        })
        .collect::<Vec<_>>();
    state::with_state(|s| {
        s.devices = devices;
        if !s.devices.iter().any(|d| d.addr == s.selected_addr) {
            s.selected_addr = s
                .devices
                .first()
                .map(|d| d.addr.clone())
                .unwrap_or_default();
        }
    });
}
pub async fn listen(addr: &str) -> Result<(), String> {
    canopusconnect::listen(addr.to_string(), MODULE_ID.to_string()).await
}
pub async fn send(addr: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > FRAME_MAX {
        return Err("Connect 帧超过 480 字节".to_string());
    }
    canopusconnect::send(
        addr.to_string(),
        MODULE_ID.to_string(),
        Payload::Binary(bytes.to_vec()),
    )
    .await
}
