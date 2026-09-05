//! 语义 UI snapshot：只使用已在 Band 10 Pro 上验证过的列表控件。

use alloc::format;
use canopus_ui_core::{
    ActionRow, NavigationPage, Snapshot, StatusRow, Text, TextStyle, Tree, UiError, View, view,
};

use crate::{
    LoopApp, Route,
    model::{format_hm, weekday_name},
    persistence::truncate_label,
};

pub const EVENT_BACK: u32 = 1;
pub const EVENT_TODAY: u32 = 2;
pub const EVENT_WEEK: u32 = 3;
pub const EVENT_DAY_BASE: u32 = 100;
pub const EVENT_COURSE_BASE: u32 = 1_000;

#[derive(Clone, Copy)]
pub struct UiEvent(pub u32);
impl From<UiEvent> for u32 {
    fn from(value: UiEvent) -> Self {
        value.0
    }
}

pub fn render(app: &LoopApp) -> Result<Snapshot, UiError> {
    match app.route {
        Route::Home => home(app),
        Route::Today => today(app),
        Route::Week => week(app),
        Route::Detail => detail(app),
    }
}

fn commit(mut tree: Tree, generation: u32) -> Result<Snapshot, UiError> {
    let mut snapshot = tree.commit()?;
    snapshot.generation = generation;
    Ok(snapshot)
}

fn home(app: &LoopApp) -> Result<Snapshot, UiError> {
    let nn = app.now_next();
    let now_name = nn
        .now
        .map(|c| truncate_label(&c.name, 12))
        .unwrap_or_else(|| alloc::string::String::from("暂无课程"));
    let now_detail = nn
        .now
        .map(|c| format!("{} · {}", c.period_label, truncate_label(&c.location, 10)))
        .unwrap_or_else(|| alloc::string::String::from("—"));
    let next_name = nn
        .next
        .map(|c| truncate_label(&c.name, 12))
        .unwrap_or_else(|| alloc::string::String::from("没有下一节"));
    let next_detail = nn
        .next
        .map(|c| {
            format!(
                "{} {}",
                format_hm(c.start_min),
                truncate_label(&c.location, 10)
            )
        })
        .unwrap_or_else(|| alloc::string::String::from("—"));
    let remaining = format!("今日剩余 {} 节", nn.remaining_today);
    let week_label = if app.week() == 0 {
        alloc::string::String::from("学期未开始")
    } else {
        format!("{} · 第{}周", app.schedule.term.name, app.week())
    };

    let view = view!(NavigationPage {
        key: 1,
        title: "Loop",
        children: (
            Text {
                key: 2,
                text: week_label.as_str(),
                style: TextStyle::Description
            },
            StatusRow {
                key: 3,
                label: "现在",
                value: now_name.as_str()
            },
            Text {
                key: 4,
                text: now_detail.as_str(),
                style: TextStyle::Description
            },
            StatusRow {
                key: 5,
                label: "下一节",
                value: next_name.as_str()
            },
            Text {
                key: 6,
                text: next_detail.as_str(),
                style: TextStyle::Description
            },
            StatusRow {
                key: 7,
                label: "进度",
                value: remaining.as_str()
            },
            ActionRow {
                key: 8,
                label: "今日课表",
                detail: "",
                event: UiEvent(EVENT_TODAY),
                enabled: true
            },
            ActionRow {
                key: 9,
                label: "本周",
                detail: "",
                event: UiEvent(EVENT_WEEK),
                enabled: true
            },
        ),
    });
    let mut tree = Tree::begin();
    <_ as View<UiEvent>>::render(&view, &mut tree)?;
    commit(tree, app.generation)
}

fn today(app: &LoopApp) -> Result<Snapshot, UiError> {
    let weekday = if app.selected_weekday == 0 {
        app.clock.map(|c| c.weekday).unwrap_or(1)
    } else {
        app.selected_weekday
    };
    let title = format!("{}", weekday_name(weekday));
    let courses = app.day_courses(weekday);
    let hint = if courses.is_empty() {
        "今天没课"
    } else {
        "点选查看详情"
    };

    // 手动构建：课程行数可变，宏元组装不下。
    let mut tree = Tree::begin();
    tree.navigation_page(1, &title)?;
    tree.text(2, hint, TextStyle::Description)?;
    for (index, course) in courses.iter().enumerate() {
        let key = 10 + index as u32;
        let label = truncate_label(&course.name, 14);
        let detail = format!(
            "{} {}",
            if course.period_label.is_empty() {
                format_hm(course.start_min)
            } else {
                course.period_label.clone()
            },
            truncate_label(&course.location, 8)
        );
        tree.action_row(key, &label, &detail, EVENT_COURSE_BASE + course.id, true)?;
    }
    tree.end()?;
    commit(tree, app.generation)
}

fn week(app: &LoopApp) -> Result<Snapshot, UiError> {
    let mut tree = Tree::begin();
    tree.navigation_page(1, "本周")?;
    tree.text(2, "选择一天查看", TextStyle::Description)?;
    for weekday in 1u8..=7u8 {
        let count = app.day_courses(weekday).len();
        let detail = if count == 0 {
            alloc::string::String::from("没课")
        } else {
            format!("{count} 节")
        };
        tree.action_row(
            10 + weekday as u32,
            weekday_name(weekday),
            &detail,
            EVENT_DAY_BASE + weekday as u32,
            true,
        )?;
    }
    tree.end()?;
    commit(tree, app.generation)
}

fn detail(app: &LoopApp) -> Result<Snapshot, UiError> {
    let Some(course) = app.selected_course() else {
        let view = view!(NavigationPage {
            key: 1,
            title: "详情",
            children: (Text {
                key: 2,
                text: "未找到课程",
                style: TextStyle::Warning
            },),
        });
        let mut tree = Tree::begin();
        <_ as View<UiEvent>>::render(&view, &mut tree)?;
        return commit(tree, app.generation);
    };
    let name = truncate_label(&course.name, 18);
    let location = if course.location.is_empty() {
        alloc::string::String::from("—")
    } else {
        truncate_label(&course.location, 20)
    };
    let period = if course.period_label.is_empty() {
        course.time_range_label()
    } else {
        format!("{} · {}", course.period_label, course.time_range_label())
    };
    let weeks = course.weeks_label();
    let weekday = weekday_name(course.weekday);

    let view = view!(NavigationPage {
        key: 1,
        title: "详情",
        children: (
            Text {
                key: 2,
                text: name.as_str(),
                style: TextStyle::Title
            },
            StatusRow {
                key: 3,
                label: "时间",
                value: period.as_str()
            },
            StatusRow {
                key: 4,
                label: "星期",
                value: weekday
            },
            StatusRow {
                key: 5,
                label: "地点",
                value: location.as_str()
            },
            StatusRow {
                key: 6,
                label: "周次",
                value: weeks.as_str()
            },
        ),
    });
    let mut tree = Tree::begin();
    <_ as View<UiEvent>>::render(&view, &mut tree)?;
    commit(tree, app.generation)
}
