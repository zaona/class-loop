//! Loop v1 wording and information order, independent of native widgets.
use crate::{Action, Course, LoopApp, Route, format_hm, weekday_name};
use alloc::{format, string::String, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Today,
    Week,
    Data,
    Day(u8),
    Course(u32),
    Refresh,
    Clear,
}
impl Target {
    pub fn action(self) -> Action {
        match self {
            Self::Today => Action::Open(Route::Today),
            Self::Week => Action::Open(Route::Week),
            Self::Data => Action::Open(Route::Data),
            Self::Day(day) => Action::SelectDay(day),
            Self::Course(id) => Action::SelectCourse(id),
            Self::Refresh => Action::RefreshSchedule,
            Self::Clear => Action::ClearSchedule,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub title: String,
    pub detail: String,
    pub target: Option<Target>,
}
impl Card {
    fn new(title: &str, detail: &str, target: Option<Target>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            target,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageView {
    pub title: String,
    pub summary: String,
    pub cards: Vec<Card>,
}
fn now_value(c: &Course, next: bool) -> String {
    if !c.period_label.is_empty() {
        format!("{} · {} · {}", c.name, c.period_label, c.location)
    } else if next {
        format!("{} · {} {}", c.name, format_hm(c.start_min), c.location)
    } else {
        format!("{} · {}", c.name, c.location)
    }
}
pub fn week_label(app: &LoopApp) -> String {
    if app.schedule.term.start_date.is_empty() {
        "尚未导入课表".into()
    } else if app.clock.is_none() {
        "设备时间不可用".into()
    } else if app.week() == 0 {
        "学期未开始".into()
    } else {
        format!("第{}周", app.week())
    }
}
pub fn present(app: &LoopApp) -> PageView {
    let mut page = PageView::default();
    match app.route {
        Route::Home => {
            page.title = "Loop".into();
            let week = week_label(app);
            page.summary = if app.schedule.term.name.is_empty() {
                week
            } else {
                format!("{} · {}", app.schedule.term.name, week)
            };
            let nn = app.now_next();
            page.cards.push(Card::new(
                "现在",
                &nn.now
                    .map_or_else(|| "暂无课程".into(), |c| now_value(c, false)),
                None,
            ));
            page.cards.push(Card::new(
                "下一节",
                &nn.next
                    .map_or_else(|| "没有下一节".into(), |c| now_value(c, true)),
                None,
            ));
            page.cards.push(Card::new(
                "进度",
                &format!("今日剩余 {} 节", nn.remaining_today),
                None,
            ));
            page.cards
                .push(Card::new("今日课表", "", Some(Target::Today)));
            page.cards.push(Card::new("本周", "", Some(Target::Week)));
            page.cards
                .push(Card::new("数据管理", "", Some(Target::Data)));
        }
        Route::Today => {
            let day = if app.selected_weekday == 0 {
                app.clock.map_or(1, |c| c.weekday)
            } else {
                app.selected_weekday
            };
            page.title = weekday_name(day).into();
            let courses = app.day_courses(day);
            page.summary = if app.schedule.term.start_date.is_empty() {
                "尚未导入课表"
            } else if app.clock.is_none() {
                "设备时间不可用"
            } else if app.week() == 0 {
                "学期未开始"
            } else if courses.is_empty() {
                "今天没课"
            } else {
                "点选查看详情"
            }
            .into();
            for c in courses {
                let period = if c.period_label.is_empty() {
                    format_hm(c.start_min)
                } else {
                    c.period_label.clone()
                };
                page.cards.push(Card::new(
                    &c.name,
                    &format!("{} {}", period, c.location),
                    Some(Target::Course(c.id)),
                ));
            }
        }
        Route::Week => {
            page.title = "本周".into();
            page.summary = "选择一天查看".into();
            for day in 1..=7 {
                let count = app.day_courses(day).len();
                let detail = if count == 0 {
                    "没课".into()
                } else {
                    format!("{count} 节")
                };
                page.cards.push(Card::new(
                    weekday_name(day),
                    &detail,
                    Some(Target::Day(day)),
                ));
            }
        }
        Route::Detail => {
            page.title = "详情".into();
            if let Some(c) = app.selected_course() {
                page.summary = c.name.clone();
                let period = if c.period_label.is_empty() {
                    c.time_range_label()
                } else {
                    format!("{} · {}", c.period_label, c.time_range_label())
                };
                page.cards.push(Card::new("时间", &period, None));
                page.cards
                    .push(Card::new("星期", weekday_name(c.weekday), None));
                page.cards.push(Card::new(
                    "地点",
                    if c.location.is_empty() {
                        "—"
                    } else {
                        &c.location
                    },
                    None,
                ));
                page.cards.push(Card::new("周次", &c.weeks_label(), None));
            } else {
                page.summary = "未找到课程".into();
            }
        }
        Route::Data => {
            page.title = "数据".into();
            page.summary = if app.data_status.is_empty() {
                "管理已导入的课表".into()
            } else {
                app.data_status.clone()
            };
            page.cards
                .push(Card::new("刷新课表", "立即重读文件", Some(Target::Refresh)));
            page.cards.push(Card::new(
                "清空本地课表",
                "删除已导入数据",
                Some(Target::Clear),
            ));
        }
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClockHint, ScheduleFile, Term};
    fn fixture() -> LoopApp {
        let mut app = LoopApp::default();
        app.schedule = ScheduleFile {
            version: 1,
            term: Term {
                name: "秋季".into(),
                start_date: "2026-08-31".into(),
            },
            courses: (1..=60)
                .map(|id| Course {
                    id,
                    name: format!("课程{id}"),
                    weekday: 1,
                    start_min: 480,
                    end_min: 540,
                    weeks_start: 1,
                    weeks_end: 16,
                    ..Course::default()
                })
                .collect(),
        };
        app.clock = Some(ClockHint {
            local_day: crate::schedule::term_start_day(&app.schedule.term).unwrap(),
            minute_of_day: 510,
            weekday: 1,
        });
        app
    }
    #[test]
    fn v1_home_and_data_wording_and_actions() {
        let mut app = fixture();
        let page = present(&app);
        assert_eq!(
            page.cards
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>(),
            ["现在", "下一节", "进度", "今日课表", "本周", "数据管理"]
        );
        assert_eq!(page.cards[0].target, None);
        app.update(Action::Open(Route::Data));
        let page = present(&app);
        assert_eq!(page.cards.len(), 2);
        assert_eq!(page.cards[0].title, "刷新课表");
        assert_eq!(page.cards[0].detail, "立即重读文件");
        assert_eq!(page.cards[1].title, "清空本地课表");
        assert!(matches!(
            page.cards[1].target.unwrap().action(),
            Action::ClearSchedule
        ));
    }
    #[test]
    fn week_hint_all_seven_days_and_full_course_ids() {
        let mut app = fixture();
        app.schedule.courses[59].id = u32::MAX;
        app.update(Action::Open(Route::Week));
        let page = present(&app);
        assert_eq!(page.summary, "选择一天查看");
        assert_eq!(page.cards.len(), 7);
        assert_eq!(page.cards[0].detail, "60 节");
        app.update(page.cards[0].target.unwrap().action());
        let page = present(&app);
        assert_eq!(page.summary, "点选查看详情");
        assert_eq!(page.cards.len(), 60);
        app.update(page.cards[59].target.unwrap().action());
        let page = present(&app);
        assert_eq!(page.summary, "课程60");
        assert_eq!(page.title, "详情");
        assert_eq!(
            page.cards
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>(),
            ["时间", "星期", "地点", "周次"]
        );
        app.update(Action::Back);
        app.update(Action::Back);
        assert_eq!(app.route, Route::Week);
        app.update(Action::SelectDay(2));
        assert_eq!(present(&app).summary, "今天没课");
    }
}
