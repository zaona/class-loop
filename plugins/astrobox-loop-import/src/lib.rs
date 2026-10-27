use crate::astrobox::psys_host_v4::ui as host_ui;
use crate::exports::astrobox::psys_plugin_v4::{event, event::EventType, lifecycle};

mod ics;
mod interconnect;
mod publish;
mod state;
mod ui;

wit_bindgen::generate!({
    path: "wit",
    world: "psys-world-v4",
    generate_all,
});

struct LoopImportPlugin;

impl event::Guest for LoopImportPlugin {
    async fn on_event(event_type: EventType, event_payload: String) -> String {
        match event_type {
            EventType::InterconnectMessage => {
                if let Some(message) = interconnect::parse_event(&event_payload) {
                    publish::handle(&message.addr, &message.package, &message.payload).await;
                    ui::rerender();
                }
            }
            EventType::DeviceAction => {
                interconnect::refresh_devices().await;
                ui::rerender();
            }
            EventType::PluginMessage
            | EventType::ProviderAction
            | EventType::DeeplinkAction
            | EventType::TransportPacket
            | EventType::Timer => {}
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
        interconnect::refresh_devices().await;
        state::with_state(|state| {
            state.status = if state.devices.is_empty() {
                "未发现已连接设备。".to_string()
            } else {
                "请打开手表上的 Loop Import，选择 ICS 来源后导入课表。".to_string()
            };
        });
    }
}

export!(LoopImportPlugin);
