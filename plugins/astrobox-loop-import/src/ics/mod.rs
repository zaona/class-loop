//! ICS → Loop schedule.json。
//!
//! 三种课表 App 导出方言相互独立实现：
//! - [`IcsSource::WakeUp`]：[`wakeup`]
//! - [`IcsSource::WeekDown`]：[`weekdown`]
//! - [`IcsSource::Nexio`]：[`nexio`]

mod nexio;
mod parse;
mod schedule;
mod time;
mod wakeup;
mod weekdown;

use serde_json::Value;

const DEFAULT_UTC_OFFSET_HOURS: i32 = 8;

/// 课表 ICS 导出来源（方言）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IcsSource {
    /// WakeUp 课程表（`PRODID:-//YZune//WakeUpSchedule//EN`）
    #[default]
    WakeUp,
    /// WeekDown 课程表（`PRODID:-//WeekDown//WeekDown Calendar Export//ZH`）
    WeekDown,
    /// Nexio 课程表（`PRODID:-//Nexio Schedule//Course Schedule//CN`）
    Nexio,
}

impl IcsSource {
    pub const ALL: &[Self] = &[Self::WakeUp, Self::WeekDown, Self::Nexio];

    pub fn label(self) -> &'static str {
        match self {
            Self::WakeUp => "WakeUp 课程表",
            Self::WeekDown => "WeekDown 课程表",
            Self::Nexio => "Nexio 课程表",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|item| item.label() == label)
    }

    pub fn supported(self) -> bool {
        matches!(self, Self::WakeUp | Self::WeekDown | Self::Nexio)
    }
}

/// 将 ICS 文本转为 schedule.json 对象。
pub fn convert_ics_text(
    text: &str,
    source: IcsSource,
    term_name: Option<&str>,
    term_start: Option<&str>,
    utc_offset_hours: i32,
) -> Result<Value, String> {
    match source {
        IcsSource::WakeUp => wakeup::convert(text, term_name, term_start, utc_offset_hours),
        IcsSource::WeekDown => weekdown::convert(text, term_name, term_start, utc_offset_hours),
        IcsSource::Nexio => nexio::convert(text, term_name, term_start, utc_offset_hours),
    }
}

pub fn convert_ics_file(
    path: &str,
    source: IcsSource,
    term_name: Option<&str>,
    term_start: Option<&str>,
) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read ICS: {e}"))?;
    if text.len() > 2 * 1024 * 1024 {
        return Err("ICS file exceeds 2 MiB".into());
    }
    convert_ics_text(
        &text,
        source,
        term_name,
        term_start,
        DEFAULT_UTC_OFFSET_HOURS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use schedule::fingerprint_courses;
    use std::path::PathBuf;

    fn data(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join(name);
        std::fs::read_to_string(path).expect(name)
    }

    #[test]
    fn wakeup_minimal_rrule() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nSUMMARY:线性代数A\nLOCATION:啬园校区 JX03-206 王占君\nDESCRIPTION:第1 - 3节\\n啬园校区 JX03-206\\n王占君\nDTSTART;TZID=Asia/Shanghai:20260903T075000\nDTEND;TZID=Asia/Shanghai:20260903T101500\nRRULE:FREQ=WEEKLY;UNTIL=20261223T160000Z;INTERVAL=1\nEND:VEVENT\nEND:VCALENDAR\n";
        let value = convert_ics_text(
            ics,
            IcsSource::WakeUp,
            Some("2026秋"),
            Some("2026-08-31"),
            8,
        )
        .unwrap();
        let courses = value["courses"].as_array().unwrap();
        assert_eq!(courses.len(), 1);
        assert_eq!(courses[0]["name"], "线性代数A");
        assert_eq!(courses[0]["location"], "JX03-206 王占君");
        assert_eq!(courses[0]["weekday"], 4);
        assert_eq!(courses[0]["start_min"], 470);
        assert_eq!(courses[0]["end_min"], 615);
        assert_eq!(courses[0]["period_label"], "第1-3节");
        assert_eq!(courses[0]["weeks_start"], 1);
        assert_eq!(courses[0]["weeks_end"], 16);
    }

    #[test]
    fn weekdown_merges_expanded() {
        let ics = "BEGIN:VCALENDAR\nPRODID:-//WeekDown//WeekDown Calendar Export//ZH\nBEGIN:VEVENT\nSUMMARY:线性代数A\nLOCATION:啬园校区  JX03-206\nDESCRIPTION:教师: 王占君\\n教室: 啬园校区  JX03-206\nDTSTART:20260903T080000\nDTEND:20260903T105000\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:线性代数A\nLOCATION:啬园校区  JX03-206\nDESCRIPTION:教师: 王占君\\n教室: 啬园校区  JX03-206\nDTSTART:20260910T080000\nDTEND:20260910T105000\nEND:VEVENT\nEND:VCALENDAR\n";
        let value = convert_ics_text(
            ics,
            IcsSource::WeekDown,
            Some("2026秋"),
            Some("2026-08-31"),
            8,
        )
        .unwrap();
        let c = &value["courses"].as_array().unwrap()[0];
        assert_eq!(c["location"], "JX03-206 王占君");
        assert_eq!(c["weeks_start"], 1);
        assert_eq!(c["weeks_end"], 2);
    }

    #[test]
    fn nexio_uses_description_week() {
        let ics = "BEGIN:VCALENDAR\nPRODID:-//Nexio Schedule//Course Schedule//CN\nBEGIN:VEVENT\nSUMMARY:形势与政策\nLOCATION:39-A105 李彤\nDESCRIPTION:第7周\nDTSTART:20261015T155000\nDTEND:20261015T172000\nRRULE:FREQ=WEEKLY;COUNT=1\nEND:VEVENT\nBEGIN:VEVENT\nSUMMARY:形势与政策\nLOCATION:39-A105 李彤\nDESCRIPTION:第15周\nDTSTART:20261210T155000\nDTEND:20261210T172000\nRRULE:FREQ=WEEKLY;COUNT=1\nEND:VEVENT\nEND:VCALENDAR\n";
        let value =
            convert_ics_text(ics, IcsSource::Nexio, Some("2026秋"), Some("2026-08-31"), 8)
                .unwrap();
        let courses = value["courses"].as_array().unwrap();
        assert_eq!(courses.len(), 1);
        assert_eq!(courses[0]["weeks_start"], 7);
        assert_eq!(courses[0]["weeks_end"], 15);
        assert_eq!(courses[0]["week_interval"], 8);
        assert_eq!(value["term"]["start_date"], "2026-08-31");
    }

    #[test]
    fn real_three_sources_semantically_match() {
        let wakeup = convert_ics_text(
            &data("wakeup.ics"),
            IcsSource::WakeUp,
            Some("2026秋"),
            Some("2026-08-31"),
            8,
        )
        .unwrap();
        let weekdown = convert_ics_text(
            &data("weekdown.ics"),
            IcsSource::WeekDown,
            Some("2026秋"),
            Some("2026-08-31"),
            8,
        )
        .unwrap();
        let nexio = convert_ics_text(
            &data("nexio.ics"),
            IcsSource::Nexio,
            Some("2026秋"),
            Some("2026-08-31"),
            8,
        )
        .unwrap();

        assert_eq!(wakeup["term"]["start_date"], "2026-08-31");
        assert_eq!(weekdown["term"]["start_date"], "2026-08-31");
        assert_eq!(nexio["term"]["start_date"], "2026-08-31");

        let a = fingerprint_courses(&wakeup);
        let b = fingerprint_courses(&weekdown);
        let c = fingerprint_courses(&nexio);
        assert_eq!(a, b, "WakeUp vs WeekDown mismatch");
        assert_eq!(a, c, "WakeUp vs Nexio mismatch");
        assert_eq!(a.len(), 13);
    }

    #[test]
    fn real_sources_auto_infer_same_term_monday() {
        let wakeup =
            convert_ics_text(&data("wakeup.ics"), IcsSource::WakeUp, None, None, 8).unwrap();
        let weekdown =
            convert_ics_text(&data("weekdown.ics"), IcsSource::WeekDown, None, None, 8).unwrap();
        let nexio = convert_ics_text(&data("nexio.ics"), IcsSource::Nexio, None, None, 8).unwrap();
        assert_eq!(wakeup["term"]["start_date"], weekdown["term"]["start_date"]);
        assert_eq!(wakeup["term"]["start_date"], nexio["term"]["start_date"]);
        assert_eq!(wakeup["term"]["start_date"], "2026-08-31");
    }
}
