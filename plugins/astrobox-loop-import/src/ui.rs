use serde::Deserialize;

use crate::astrobox::psys_host::{
    dialog::{self, FilterConfig, PickConfig},
    ui_v3 as ui,
};
use crate::{interconnect, publish, state};

const EVENT_PICK: &str = "action:file.schedule";
const EVENT_REFRESH: &str = "action:devices.refresh";
const EVENT_PUSH: &str = "action:publish.start";
const EVENT_DEVICE: &str = "input:device";
const EVENT_TERM_NAME: &str = "input:term.name";
const EVENT_TERM_START: &str = "input:term.start";
const EVENT_RECONVERT: &str = "action:ics.reconvert";

#[derive(Default, Deserialize)]
struct UiPayload {
    #[serde(default)]
    value: Option<String>,
}

pub fn render_main_ui(root: &str) {
    state::with_state(|state| state.root = Some(root.to_string()));
    rerender();
}

pub fn rerender() {
    if let Some(root) = state::snapshot().root {
        ui::render(&root, build_root());
    }
}

pub fn on_event(event_id: &str, payload: &str) {
    tracing::info!(event_id, "received Loop Import UI event");
    let payload = serde_json::from_str::<UiPayload>(payload).unwrap_or_default();
    match event_id {
        EVENT_PICK => pick_file(),
        EVENT_REFRESH => {
            interconnect::refresh_devices();
            state::with_state(|state| state.status = "已刷新连接设备。".to_string());
        }
        EVENT_DEVICE => {
            state::with_state(|state| {
                state.selected_addr = payload.value.unwrap_or_default();
            });
            return;
        }
        EVENT_TERM_NAME => {
            state::with_state(|state| {
                state.term_name = payload.value.unwrap_or_default();
            });
            return;
        }
        EVENT_TERM_START => {
            state::with_state(|state| {
                state.term_start = payload.value.unwrap_or_default();
            });
            return;
        }
        EVENT_RECONVERT => {
            if let Err(error) = publish::refresh_prepared_from_state() {
                state::with_state(|state| state.status = format!("重新转换失败：{error}"));
            }
        }
        EVENT_PUSH => start_push(),
        _ => {}
    }
    rerender();
}

fn pick_file() {
    let result = wit_bindgen::block_on(
        dialog::pick_file(
            &PickConfig {
                read: false,
                copy_to: Some("media".to_string()),
            },
            &FilterConfig {
                multiple: false,
                extensions: vec!["ics".to_string(), "json".to_string()],
                default_directory: String::new(),
                default_file_name: String::new(),
            },
        )
        .into_future(),
    );
    if result.name.is_empty() {
        return;
    }
    let path = format!("media/{}", result.name);
    let snapshot = state::snapshot();
    let term_name = state::empty_to_none(&snapshot.term_name);
    let term_start = state::empty_to_none(&snapshot.term_start);
    match publish::prepare_file(&path, &result.name, term_name, term_start) {
        Ok(prepared) => state::with_state(|state| {
            let kind = match prepared.kind {
                state::SourceKind::Ics => "ICS",
                state::SourceKind::Json => "JSON",
            };
            state.status = format!(
                "已解析 {kind}，共 {} 门课。可推送到手表。",
                prepared.course_count
            );
            if prepared.kind == state::SourceKind::Ics {
                if state.term_name.trim().is_empty() {
                    if let Some(name) = prepared
                        .schedule
                        .pointer("/term/name")
                        .and_then(|v| v.as_str())
                    {
                        state.term_name = name.to_string();
                    }
                }
                if state.term_start.trim().is_empty() {
                    if let Some(start) = prepared
                        .schedule
                        .pointer("/term/start_date")
                        .and_then(|v| v.as_str())
                    {
                        state.term_start = start.to_string();
                    }
                }
            }
            state.prepared = Some(prepared);
        }),
        Err(error) => state::with_state(|state| state.status = format!("文件无效：{error}")),
    }
}

fn start_push() {
    let snapshot = state::snapshot();
    if snapshot.busy {
        state::with_state(|state| state.status = "推送进行中，请稍候。".to_string());
        return;
    }
    let Some(prepared) = snapshot.prepared else {
        state::with_state(|state| state.status = "请先选择 .ics 或 schedule.json。".to_string());
        return;
    };
    let result = wit_bindgen::block_on(publish::start(
        &snapshot.selected_addr,
        prepared.schedule,
    ));
    if let Err(error) = result {
        state::with_state(|state| state.status = format!("无法开始推送：{error}"));
    }
}

fn build_root() -> ui::Element {
    let state = state::snapshot();
    let mut root = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .padding(28)
        .gap(18)
        .child(text("Loop 课表导入", 28, "#f4f4f5"))
        .child(text(
            "选择课表 ICS 或 schedule.json，推送到手表 top.zaona.loopimport。",
            14,
            "#a1a1aa",
        ));

    let mut device_select = ui::Element::new(ui::ElementType::Select, None)
        .width_full()
        .prop("default-value", &state.selected_addr)
        .prop("key", EVENT_DEVICE)
        .on(ui::Event::Change, EVENT_DEVICE);
    for device in &state.devices {
        device_select = device_select.child(
            ui::Element::new(ui::ElementType::Option, Some(&device.name))
                .prop("value", &device.addr),
        );
    }

    let device = ui::Element::new(ui::ElementType::Card, None)
        .width_full()
        .padding(18)
        .radius(12)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(10)
        .child(text("目标设备", 18, "#f4f4f5"))
        .child(text(
            "请保持手表上 Loop Import 快应用前台打开。",
            13,
            "#a1a1aa",
        ))
        .child(device_select)
        .child(button("刷新设备", EVENT_REFRESH, "#27272a"));
    root = root.child(device);

    let file_label = match &state.prepared {
        Some(file) => {
            let kind = match file.kind {
                state::SourceKind::Ics => "ICS",
                state::SourceKind::Json => "JSON",
            };
            format!(
                "{} · {} · {} 门课 · {} 字节",
                file.name, kind, file.course_count, file.size
            )
        }
        None => "尚未选择文件".to_string(),
    };
    let mut push_btn = button("推送到手表", EVENT_PUSH, "#16a34a");
    if state.busy {
        push_btn = push_btn.disabled();
    }
    let mut local = ui::Element::new(ui::ElementType::Card, None)
        .width_full()
        .padding(18)
        .radius(12)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .gap(12)
        .child(text("课表文件", 22, "#f4f4f5"))
        .child(text(
            "支持直接选 .ics（插件内转换）或已有 schedule.json。",
            13,
            "#a1a1aa",
        ))
        .child(text(&file_label, 14, "#d4d4d8"))
        .child(button("选择 ICS / JSON", EVENT_PICK, "#2563eb"))
        .child(input(
            "学期名称（可选，ICS 用）",
            &state.term_name,
            EVENT_TERM_NAME,
        ))
        .child(input(
            "学期起始日 YYYY-MM-DD（可选）",
            &state.term_start,
            EVENT_TERM_START,
        ));
    if state
        .prepared
        .as_ref()
        .is_some_and(|item| item.kind == state::SourceKind::Ics)
    {
        local = local.child(button("按学期设置重新转换 ICS", EVENT_RECONVERT, "#4f46e5"));
    }
    local = local.child(push_btn);
    root = root.child(local);

    if !state.status.is_empty() {
        root = root.child(
            ui::Element::new(ui::ElementType::Card, None)
                .width_full()
                .padding(18)
                .radius(12)
                .flex()
                .flex_direction(ui::FlexDirection::Column)
                .gap(8)
                .child(text("状态", 18, "#f4f4f5"))
                .child(text(&state.status, 14, "#d4d4d8")),
        );
    }

    root
}

fn input(placeholder: &str, value: &str, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Input, None)
        .width_full()
        .prop("placeholder", placeholder)
        .prop("default-value", value)
        .prop("key", event_id)
        .on(ui::Event::Input, event_id)
}

fn button(label: &str, event_id: &str, background: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, Some(label))
        .width_full()
        .padding(12)
        .radius(8)
        .bg(background)
        .text_color("#ffffff")
        .on(ui::Event::Click, event_id)
}

fn text(content: &str, size: u32, color: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(content))
        .size(size)
        .text_color(color)
}
