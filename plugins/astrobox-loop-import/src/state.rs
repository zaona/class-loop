use std::sync::{Mutex, OnceLock};

use serde_json::Value;

use crate::ics::IcsSource;

#[derive(Clone, Debug, Default)]
pub struct DeviceInfo {
    pub addr: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct PreparedSchedule {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub schedule: Value,
    pub course_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MainTab {
    #[default]
    Import,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditingTerm {
    Name,
    Start,
}

#[derive(Clone, Debug, Default)]
pub struct UiState {
    pub root: Option<String>,
    /// 宿主渲染区宽度（CSS 像素），0 表示未知。
    pub render_width: u32,
    pub current_tab: MainTab,
    pub ics_source: IcsSource,
    pub devices: Vec<DeviceInfo>,
    pub selected_addr: String,
    pub prepared: Option<PreparedSchedule>,
    pub term_name: String,
    pub term_start: String,
    pub editing_term: Option<EditingTerm>,
    pub term_draft: String,
    pub status: String,
    pub busy: bool,
}

const COMPACT_MAX_WIDTH: u32 = 608;

impl UiState {
    /// 宽度未知时按紧凑布局处理，避免首帧溢出。
    pub fn is_compact_layout(&self) -> bool {
        self.render_width < COMPACT_MAX_WIDTH
    }
}

static STATE: OnceLock<Mutex<UiState>> = OnceLock::new();

pub fn with_state<R>(f: impl FnOnce(&mut UiState) -> R) -> R {
    let mut state = STATE
        .get_or_init(|| Mutex::new(UiState::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut state)
}

pub fn snapshot() -> UiState {
    with_state(|state| state.clone())
}

pub fn empty_to_none(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
