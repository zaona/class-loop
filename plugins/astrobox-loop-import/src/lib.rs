use wit_bindgen::FutureReader;

use crate::exports::astrobox::psys_plugin::{event_v3 as event, event_v3::EventType, lifecycle};

mod ics;
mod interconnect;
mod publish;
mod state;
mod ui;

wit_bindgen::generate!({
    path: "wit",
    world: "psys-world-v3",
    generate_all,
});

struct LoopImportPlugin;

impl event::Guest for LoopImportPlugin {
    fn on_event(event_type: EventType, event_payload: _rt::String) -> FutureReader<String> {
        match event_type {
            EventType::InterconnectMessage => {
                if let Some(message) = interconnect::parse_event(&event_payload) {
                    wit_bindgen::block_on(publish::handle(
                        &message.addr,
                        &message.package,
                        &message.payload,
                    ));
                    ui::rerender();
                }
            }
            EventType::DeviceAction => {
                interconnect::refresh_devices();
                ui::rerender();
            }
            EventType::PluginMessage
            | EventType::ProviderAction
            | EventType::DeeplinkAction
            | EventType::TransportPacket
            | EventType::Timer => {}
        }
        immediate_string(String::new())
    }

    fn on_ui_event_v3(
        event_id: _rt::String,
        _event: event::Event,
        event_payload: _rt::String,
    ) -> FutureReader<_rt::String> {
        ui::on_event(&event_id, &event_payload);
        immediate_string(String::new())
    }

    fn on_ui_render(element_id: _rt::String) -> FutureReader<()> {
        ui::render_main_ui(&element_id);
        immediate_unit()
    }

    fn on_card_render(_card_id: _rt::String) -> FutureReader<()> {
        immediate_unit()
    }
}

impl lifecycle::Guest for LoopImportPlugin {
    fn on_load() {
        tracing_subscriber::fmt()
            .with_writer(std::io::stdout)
            .with_ansi(false)
            .compact()
            .init();
        interconnect::refresh_devices();
        state::with_state(|state| {
            state.status = if state.devices.is_empty() {
                "未发现已连接设备。".to_string()
            } else {
                "请打开手表上的 Loop Import，然后选择课表 ICS 或 schedule.json。"
                    .to_string()
            };
        });
    }
}

fn immediate_string(value: String) -> FutureReader<String> {
    let (writer, reader) = wit_future::new(String::new);
    wit_bindgen::spawn(async move {
        let _ = writer.write(value).await;
    });
    reader
}

fn immediate_unit() -> FutureReader<()> {
    let (writer, reader) = wit_future::new::<()>(|| ());
    wit_bindgen::spawn(async move {
        let _ = writer.write(()).await;
    });
    reader
}

export!(LoopImportPlugin);
