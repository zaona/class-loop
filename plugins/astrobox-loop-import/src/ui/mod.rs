mod icons;

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
const EVENT_TAB_IMPORT: &str = "tab:import";
const EVENT_TAB_SETTINGS: &str = "tab:settings";
const EVENT_OPEN_HELP: &str = "action:open.help";

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
        EVENT_TAB_IMPORT => {
            let changed = state::with_state(|state| {
                if state.current_tab != state::MainTab::Import {
                    state.current_tab = state::MainTab::Import;
                    true
                } else {
                    false
                }
            });
            if changed {
                rerender();
            }
        }
        EVENT_TAB_SETTINGS => {
            let changed = state::with_state(|state| {
                if state.current_tab != state::MainTab::Settings {
                    state.current_tab = state::MainTab::Settings;
                    true
                } else {
                    false
                }
            });
            if changed {
                rerender();
            }
        }
        EVENT_PICK => pick_file(),
        EVENT_REFRESH => {
            interconnect::refresh_devices();
            state::with_state(|state| state.status = "已刷新连接设备。".to_string());
            rerender();
        }
        EVENT_DEVICE => {
            let value = payload.value.unwrap_or_default();
            state::with_state(|state| {
                if let Some(device) = state.devices.iter().find(|item| item.name == value) {
                    state.selected_addr = device.addr.clone();
                } else if state.devices.iter().any(|item| item.addr == value) {
                    state.selected_addr = value;
                }
            });
        }
        EVENT_TERM_NAME => {
            state::with_state(|state| {
                state.term_name = payload.value.unwrap_or_default();
            });
        }
        EVENT_TERM_START => {
            state::with_state(|state| {
                state.term_start = payload.value.unwrap_or_default();
            });
        }
        EVENT_RECONVERT => {
            if let Err(error) = publish::refresh_prepared_from_state() {
                state::with_state(|state| state.status = format!("重新转换失败：{error}"));
            }
            rerender();
        }
        EVENT_PUSH => {
            start_push();
            rerender();
        }
        EVENT_OPEN_HELP => show_help(),
        _ => {}
    }
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
    rerender();
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

fn show_help() {
    wit_bindgen::block_on(async move {
        let _ = dialog::show_dialog(
            dialog::DialogType::Alert,
            dialog::DialogStyle::Website,
            &dialog::DialogInfo {
                title: "使用说明".to_string(),
                content: "1. 在手表上打开 Loop Import 并保持前台\n2. 选择已连接的目标设备\n3. 选择课表 .ics 或 schedule.json\n4. 如有需要填写学期名称与起始日\n5. 点击「推送到手表」".to_string(),
                buttons: vec![dialog::DialogButton {
                    id: "ok".to_string(),
                    primary: true,
                    content: "确定".to_string(),
                }],
            },
        )
        .await;
    });
}

fn build_root() -> ui::Element {
    let snapshot = state::snapshot();
    let content = match snapshot.current_tab {
        state::MainTab::Import => build_import_tab(&snapshot),
        state::MainTab::Settings => build_settings_tab(),
    };
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .padding(20)
        .child(build_tabs(&snapshot))
        .child(content)
}

fn build_tabs(snapshot: &state::UiState) -> ui::Element {
    let tabs_list = ui::Element::new(ui::ElementType::TabsList, None)
        .flex()
        .bg("#1E1E1F")
        .radius(999)
        .padding(4)
        .gap(4)
        .child(build_tab_trigger(
            "导入课表",
            icons::send_tab_svg(),
            snapshot.current_tab == state::MainTab::Import,
            EVENT_TAB_IMPORT,
        ))
        .child(build_tab_trigger(
            "设置",
            icons::settings_tab_svg(),
            snapshot.current_tab == state::MainTab::Settings,
            EVENT_TAB_SETTINGS,
        ));

    ui::Element::new(ui::ElementType::TabsRoot, None)
        .flex()
        .justify_center()
        .margin_bottom(20)
        .child(tabs_list)
}

fn build_import_tab(snapshot: &state::UiState) -> ui::Element {
    let mut root = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full();

    root = root
        .child(build_file_card(snapshot).margin_bottom(8))
        .child(build_device_card(snapshot).margin_bottom(8))
        .child(
            build_input_card(
                icons::notebook_svg(),
                "学期名称",
                "ICS 转换时使用，可留空",
                &snapshot.term_name,
                EVENT_TERM_NAME,
            )
            .margin_bottom(8),
        )
        .child(
            build_input_card(
                icons::calendar_svg(),
                "学期起始日",
                "格式 YYYY-MM-DD，可留空",
                &snapshot.term_start,
                EVENT_TERM_START,
            )
            .margin_bottom(8),
        );

    if snapshot
        .prepared
        .as_ref()
        .is_some_and(|item| item.kind == state::SourceKind::Ics)
    {
        root = root.child(
            build_icon_text_button("按学期设置重新转换", icons::convert_svg(), EVENT_RECONVERT)
                .bg("#2A2A2A")
                .margin_bottom(8),
        );
    }

    if !snapshot.status.is_empty() {
        root = root.child(
            build_settings_card(
                icons::info_svg(),
                "状态",
                Some(snapshot.status.as_str()),
                None,
                None,
            )
            .margin_bottom(8),
        );
    }

    let push_label = if snapshot.busy {
        "推送中..."
    } else {
        "推送到手表"
    };
    let mut push = build_icon_text_button(push_label, icons::send_tab_svg(), EVENT_PUSH)
        .bg("#0090FF26")
        .text_color("#0090FF");
    if snapshot.busy {
        push = push.disabled();
    }
    root.child(push)
}

fn build_settings_tab() -> ui::Element {
    let build_time = format_beijing_time(option_env!("AB_BUILD_TIME").unwrap_or("unknown"));
    let build_user = option_env!("AB_BUILD_USER").unwrap_or("unknown");
    let build_branch = option_env!("AB_BUILD_GIT_BRANCH").unwrap_or("unknown");
    let build_hash = short_git_hash(option_env!("AB_BUILD_GIT_HASH").unwrap_or("unknown"));

    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .gap(8)
        .child(build_section_title("更多内容"))
        .child(
            build_settings_card(
                icons::help_svg(),
                "使用说明",
                Some("操作步骤与导入注意事项"),
                Some(build_more_link_icon()),
                Some(EVENT_OPEN_HELP),
            )
            .margin_bottom(10),
        )
        .child(build_section_title("构建信息"))
        .child(build_settings_card(
            icons::time_svg(),
            "构建时间",
            None,
            Some(build_value_text(&build_time)),
            None,
        ))
        .child(build_settings_card(
            icons::user_svg(),
            "构建用户",
            None,
            Some(build_value_text(build_user)),
            None,
        ))
        .child(build_settings_card(
            icons::branch_svg(),
            "当前分支",
            None,
            Some(build_value_text(build_branch)),
            None,
        ))
        .child(build_settings_card(
            icons::hash_svg(),
            "当前hash",
            None,
            Some(build_value_text(&build_hash)),
            None,
        ))
}

fn build_file_card(snapshot: &state::UiState) -> ui::Element {
    let desc = match &snapshot.prepared {
        Some(file) => {
            let kind = match file.kind {
                state::SourceKind::Ics => "ICS",
                state::SourceKind::Json => "JSON",
            };
            format!(
                "{} · {} · {} 门课 · {}",
                file.name,
                kind,
                file.course_count,
                format_bytes(file.size)
            )
        }
        None => "选择 .ics 或 schedule.json".to_string(),
    };
    let arrow = ui::Element::new(ui::ElementType::Svg, Some(&icons::chevron_right_svg()))
        .width(18)
        .height(18)
        .text_color("#888888");
    build_settings_card(
        icons::file_svg(),
        "课表文件",
        Some(desc.as_str()),
        Some(arrow),
        Some(EVENT_PICK),
    )
}

fn build_device_card(snapshot: &state::UiState) -> ui::Element {
    let selected_name = snapshot
        .devices
        .iter()
        .find(|item| item.addr == snapshot.selected_addr)
        .map(|item| item.name.as_str())
        .unwrap_or("未连接设备");

    let mut select = ui::Element::new(ui::ElementType::Select, Some(selected_name))
        .on(ui::Event::Change, EVENT_DEVICE)
        .radius(8)
        .padding_left(12)
        .padding_right(12)
        .bg("#2A2A2A")
        .size(14);

    if snapshot.devices.is_empty() {
        select = select.child(ui::Element::new(ui::ElementType::Option, Some("未连接设备")));
    } else {
        for device in &snapshot.devices {
            select =
                select.child(ui::Element::new(ui::ElementType::Option, Some(&device.name)));
        }
    }

    let refresh = ui::Element::new(ui::ElementType::Button, None)
        .without_default_styles()
        .on(ui::Event::Click, EVENT_REFRESH)
        .width(36)
        .height(36)
        .radius(999)
        .bg("#2A2A2A")
        .flex()
        .align_center()
        .justify_center()
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icons::refresh_svg()))
                .width(18)
                .height(18),
        );

    build_settings_card(
        icons::device_svg(),
        "目标设备",
        Some("请保持手表 Loop Import 前台打开"),
        Some(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .align_center()
                .child(select)
                .child(refresh.margin_left(8)),
        ),
        None,
    )
}

fn build_input_card(
    icon_svg: String,
    title: &str,
    desc: &str,
    value: &str,
    event_id: &str,
) -> ui::Element {
    let header = build_settings_row(icon_svg, title, Some(desc), None);
    // 与天气插件一致：用 content 承载当前值，而不是 default-value prop。
    let input = ui::Element::new(ui::ElementType::Input, Some(value))
        .on(ui::Event::Change, event_id)
        .on(ui::Event::Input, event_id)
        .radius(12)
        .bg("#2A2A2A")
        .height(40)
        .width_full()
        .padding_left(12)
        .padding_right(12)
        .size(14);
    ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .width_full()
        .bg("#1E1E1F")
        .radius(18)
        .padding_left(12)
        .padding_right(12)
        .padding_top(10)
        .padding_bottom(12)
        .child(header.margin_bottom(8))
        .child(input)
}

fn build_settings_card(
    icon_svg: String,
    title: &str,
    desc: Option<&str>,
    right: Option<ui::Element>,
    click_event: Option<&str>,
) -> ui::Element {
    let mut row = build_settings_row(icon_svg, title, desc, right)
        .bg("#1E1E1F")
        .radius(18)
        .padding_left(12)
        .padding_right(12)
        .padding_top(10)
        .padding_bottom(10);
    if let Some(event_id) = click_event {
        row = row.on(ui::Event::Click, event_id);
    }
    row
}

fn build_settings_row(
    icon_svg: String,
    title: &str,
    desc: Option<&str>,
    right: Option<ui::Element>,
) -> ui::Element {
    let icon = ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
        .width(22)
        .height(22)
        .text_color("#FFFFFF");
    let icon_wrap = ui::Element::new(ui::ElementType::Div, None)
        .width(22)
        .height(22)
        .flex()
        .align_center()
        .justify_center()
        .child(icon);

    let mut text_col = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Column)
        .flex_grow(1.0)
        .child(ui::Element::new(ui::ElementType::P, Some(title)).size(15));
    if let Some(desc_text) = desc {
        text_col = text_col.child(
            ui::Element::new(ui::ElementType::P, Some(desc_text))
                .size(13)
                .text_color("#888888"),
        );
    }

    let mut row = ui::Element::new(ui::ElementType::Div, None)
        .flex()
        .flex_direction(ui::FlexDirection::Row)
        .align_center()
        .width_full()
        .child(icon_wrap)
        .child(text_col.margin_left(10));

    if let Some(right_el) = right {
        row = row.child(
            ui::Element::new(ui::ElementType::Div, None)
                .flex()
                .align_center()
                .justify_end()
                .margin_left(10)
                .child(right_el),
        );
    }
    row
}

fn build_tab_trigger(label: &str, icon_svg: String, is_active: bool, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::TabsTrigger, None)
        .without_default_styles()
        .on(ui::Event::Click, event_id)
        .radius(999)
        .padding_top(10)
        .padding_bottom(10)
        .padding_left(14)
        .padding_right(14)
        .bg(if is_active { "#2A2A2A" } else { "#1E1E1F" })
        .text_color(if is_active { "#FFFFFF" } else { "#BBBBBB" })
        .flex()
        .align_center()
        .gap(5)
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
                .width(22)
                .height(22),
        )
        .child(ui::Element::new(ui::ElementType::Span, Some(label)).size(14))
}

fn build_icon_text_button(label: &str, icon_svg: String, event_id: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::Button, None)
        .without_default_styles()
        .on(ui::Event::Click, event_id)
        .radius(18)
        .padding(14)
        .bg("#2A2A2A")
        .width_full()
        .flex()
        .align_center()
        .child(
            ui::Element::new(ui::ElementType::Svg, Some(&icon_svg))
                .width(22)
                .height(22),
        )
        .child(
            ui::Element::new(ui::ElementType::Span, Some(label))
                .size(14)
                .margin_left(8),
        )
}

fn build_section_title(text: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(text))
        .size(13)
        .text_color("#888888")
        .margin_left(12)
}

fn build_more_link_icon() -> ui::Element {
    ui::Element::new(ui::ElementType::Svg, Some(&icons::more_link_svg()))
        .width(18)
        .height(18)
        .text_color("#0088FF")
}

fn build_value_text(value: &str) -> ui::Element {
    ui::Element::new(ui::ElementType::P, Some(value))
        .size(13)
        .text_color("#BBBBBB")
}

fn format_bytes(size: u64) -> String {
    if size < 1024 {
        format!("{size} B")
    } else {
        format!("{} KB", (size + 512) / 1024)
    }
}

fn short_git_hash(hash: &str) -> String {
    let trimmed = hash.trim();
    if trimmed.is_empty() || trimmed == "unknown" {
        return "unknown".to_string();
    }
    trimmed.chars().take(7).collect()
}

fn format_beijing_time(raw: &str) -> String {
    if let Some((y, m, d, hh, mm, ss)) = parse_iso_utc(raw) {
        let (y2, m2, d2, hh2) = add_hours(y, m, d, hh, 8);
        return format!("{y2:04}-{m2:02}-{d2:02}_{hh2:02}:{mm:02}:{ss:02}");
    }
    raw.to_string()
}

fn parse_iso_utc(raw: &str) -> Option<(i32, i32, i32, i32, i32, i32)> {
    if raw.len() < 19 {
        return None;
    }
    let base = &raw[..19];
    let mut parts = base.split('T');
    let date = parts.next()?;
    let time = parts.next()?;
    let mut dparts = date.split('-');
    let y: i32 = dparts.next()?.parse().ok()?;
    let m: i32 = dparts.next()?.parse().ok()?;
    let d: i32 = dparts.next()?.parse().ok()?;
    let mut tparts = time.split(':');
    let hh: i32 = tparts.next()?.parse().ok()?;
    let mm: i32 = tparts.next()?.parse().ok()?;
    let ss: i32 = tparts.next()?.parse().ok()?;
    Some((y, m, d, hh, mm, ss))
}

fn add_hours(mut y: i32, mut m: i32, mut d: i32, mut hh: i32, add: i32) -> (i32, i32, i32, i32) {
    hh += add;
    while hh >= 24 {
        hh -= 24;
        d += 1;
        let dim = days_in_month(y, m);
        if d > dim {
            d = 1;
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
    }
    (y, m, d, hh)
}

fn days_in_month(y: i32, m: i32) -> i32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}
