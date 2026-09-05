//! 课表 JSON 数据模型（由主机 ICS 转换脚本生成，设备端只反序列化）。

use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// 内置样例版本；与 `fixtures/schedule.json` / 覆盖文件 schema 一致。
pub const SCHEDULE_VERSION: u8 = 1;

/// 学期元数据。`start_date` 所在日历周为第 1 教学周。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub name: String,
    /// `YYYY-MM-DD`，按本地日历日解释（Asia/Shanghai）。
    pub start_date: String,
}

/// 一条课程出现记录。同名课不同周段会拆成多条，不合并。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Course {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub location: String,
    /// 1 = 周一 … 7 = 周日。
    pub weekday: u8,
    /// 当天 0 点起的开始分钟（07:50 → 470）。
    pub start_min: u16,
    /// 当天 0 点起的结束分钟。
    pub end_min: u16,
    /// 展示用节次标签，如 `第1-2节`；不参与时间推算。
    #[serde(default)]
    pub period_label: String,
    pub weeks_start: u8,
    pub weeks_end: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleFile {
    pub version: u8,
    pub term: Term,
    #[serde(default)]
    pub courses: Vec<Course>,
}

impl Course {
    /// `HH:MM` 起止文案，供详情页展示。
    pub fn time_range_label(&self) -> alloc::string::String {
        alloc::format!("{}-{}", format_hm(self.start_min), format_hm(self.end_min))
    }

    pub fn weeks_label(&self) -> alloc::string::String {
        if self.weeks_start == self.weeks_end {
            alloc::format!("第{}周", self.weeks_start)
        } else {
            alloc::format!("第{}-{}周", self.weeks_start, self.weeks_end)
        }
    }

    /// 指定教学周是否包含本条。
    pub fn active_in_week(&self, week: u8) -> bool {
        week >= self.weeks_start && week <= self.weeks_end
    }
}

pub fn format_hm(minutes: u16) -> alloc::string::String {
    let h = minutes / 60;
    let m = minutes % 60;
    alloc::format!("{:02}:{:02}", h, m)
}

pub fn weekday_name(weekday: u8) -> &'static str {
    match weekday {
        1 => "周一",
        2 => "周二",
        3 => "周三",
        4 => "周四",
        5 => "周五",
        6 => "周六",
        7 => "周日",
        _ => "—",
    }
}
