use crate::astrobox::psys_host_v4::ui as host_ui;
use crate::exports::astrobox::psys_plugin_v4::{event, event::EventType, lifecycle};

mod connect;
mod ics;
mod publish;
mod state;
mod transfer;
mod ui;

wit_bindgen::generate!({
    path: "wit",
    world: "psys-world-v4-canopusconnect",
    generate_all,
});

struct LoopImportPlugin;

impl event::Guest for LoopImportPlugin {
    async fn on_event(event_type: EventType, event_payload: String) -> String {
        match event_type {
            EventType::Timer => {
                transfer::on_timer(&event_payload).await;
                ui::rerender();
            }
            EventType::DeviceAction => {
                connect::refresh_devices().await;
                ui::rerender();
            }
            EventType::InterconnectMessage
            | EventType::PluginMessage
            | EventType::ProviderAction
            | EventType::DeeplinkAction
            | EventType::TransportPacket => {}
        }
        String::new()
    }

    async fn on_ui_event(
        event_id: String,
        _event: host_ui::Event,
        event_payload: String,
    ) -> String {
        ui::on_event(&event_id, &event_payload).await;
        String::new()
    }

    async fn on_ui_render(element_id: String) {
        ui::render_main_ui(&element_id).await;
    }

    async fn on_card_render(_card_id: String) {}
}

impl lifecycle::Guest for LoopImportPlugin {
    async fn on_load() {
        tracing_subscriber::fmt()
            .with_writer(std::io::stdout)
            .with_ansi(false)
            .compact()
            .init();
        tracing::info!(
            "BUILD_INFO time={} user={} branch={} hash={}",
            option_env!("AB_BUILD_TIME").unwrap_or("unknown"),
            option_env!("AB_BUILD_USER").unwrap_or("unknown"),
            option_env!("AB_BUILD_GIT_BRANCH").unwrap_or("unknown"),
            option_env!("AB_BUILD_GIT_HASH").unwrap_or("unknown")
        );
        connect::refresh_devices().await;
        state::with_state(|state| {
            state.status = if state.devices.is_empty() {
                "未发现已连接设备。".to_string()
            } else {
                "请确认手表 Loop 已启用，选择 ICS 来源后导入课表。".to_string()
            };
        });
    }
}

export!(LoopImportPlugin);

impl crate::exports::astrobox::psys_plugin_v4::canopusconnect::Guest for LoopImportPlugin {
    async fn on_message(
        device_addr: String,
        module: String,
        payload: crate::astrobox::psys_host_v4::canopusconnect::Payload,
    ) {
        if let crate::astrobox::psys_host_v4::canopusconnect::Payload::Binary(bytes) = payload {
            transfer::on_frame(&device_addr, &module, &bytes).await;
            ui::rerender();
        }
    }
}
