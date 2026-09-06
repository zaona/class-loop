use alloc::{string::String, vec::Vec};

use crate::{
    Course, ScheduleFile,
    schedule::{ClockHint, NowNext, courses_on_day, now_and_next, term_week},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Route {
    #[default]
    Home,
    Today,
    Week,
    Detail,
    Data,
}

impl Route {
    pub const fn page_index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Today => 1,
            Self::Week => 2,
            Self::Detail => 3,
            Self::Data => 4,
        }
    }

    pub const fn from_page_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Home),
            1 => Some(Self::Today),
            2 => Some(Self::Week),
            3 => Some(Self::Detail),
            4 => Some(Self::Data),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Effect {
    Navigate(Route),
    /// 立即从快应用沙箱重读 schedule.json。
    ReloadFromDisk,
    /// 删除沙箱中的 schedule.json 并清空内存课表。
    ClearStoredSchedule,
}

#[derive(Clone, Debug)]
pub enum Action {
    Boot(ScheduleFile),
    Open(Route),
    Back,
    SelectDay(u8),
    SelectCourse(u32),
    Tick(ClockHint),
    Reload(ScheduleFile),
    RefreshSchedule,
    ClearSchedule,
    SetDataStatus(String),
}

#[derive(Clone, Debug, Default)]
pub struct LoopApp {
    pub route: Route,
    pub history: Vec<Route>,
    pub schedule: ScheduleFile,
    pub clock: Option<ClockHint>,
    pub selected_weekday: u8,
    pub selected_course_id: Option<u32>,
    pub data_status: String,
    pub generation: u32,
}

impl LoopApp {
    pub fn bump(&mut self) {
        self.generation = self.generation.wrapping_add(1).max(1);
    }

    pub fn week(&self) -> u8 {
        match self.clock {
            Some(clock) => term_week(&self.schedule.term, clock.local_day),
            None => 0,
        }
    }

    pub fn today_courses(&self) -> Vec<&Course> {
        let Some(clock) = self.clock else {
            return Vec::new();
        };
        courses_on_day(&self.schedule.courses, self.week(), clock.weekday)
    }

    pub fn day_courses(&self, weekday: u8) -> Vec<&Course> {
        courses_on_day(&self.schedule.courses, self.week(), weekday)
    }

    pub fn selected_course(&self) -> Option<&Course> {
        let id = self.selected_course_id?;
        self.schedule.courses.iter().find(|c| c.id == id)
    }

    pub fn now_next(&self) -> NowNext<'_> {
        match self.clock {
            Some(clock) => now_and_next(&self.schedule.courses, &self.schedule.term, clock),
            None => NowNext {
                now: None,
                next: None,
                remaining_today: 0,
            },
        }
    }

    pub fn update(&mut self, action: Action) -> Vec<Effect> {
        let mut effects = Vec::new();
        match action {
            Action::Boot(file) | Action::Reload(file) => {
                self.schedule = file;
                self.bump();
            }
            Action::Open(route) => {
                if self.route != route {
                    if route == Route::Today {
                        if let Some(clock) = self.clock {
                            self.selected_weekday = clock.weekday;
                        }
                    }
                    if route == Route::Data {
                        self.data_status = if self.schedule.courses.is_empty()
                            && self.schedule.term.name.is_empty()
                        {
                            String::from("尚未导入课表")
                        } else {
                            alloc::format!("当前 {} 门课", self.schedule.courses.len())
                        };
                    }
                    self.history.push(self.route);
                    self.route = route;
                    self.bump();
                    effects.push(Effect::Navigate(route));
                }
            }
            Action::Back => {
                if let Some(previous) = self.history.pop() {
                    self.route = previous;
                    self.bump();
                    effects.push(Effect::Navigate(previous));
                }
            }
            Action::SelectDay(weekday) => {
                self.selected_weekday = weekday;
                self.history.push(self.route);
                self.route = Route::Today;
                self.bump();
                effects.push(Effect::Navigate(Route::Today));
            }
            Action::SelectCourse(id) => {
                self.selected_course_id = Some(id);
                self.history.push(self.route);
                self.route = Route::Detail;
                self.bump();
                effects.push(Effect::Navigate(Route::Detail));
            }
            Action::Tick(clock) => {
                let changed = self.clock != Some(clock);
                self.clock = Some(clock);
                if self.route == Route::Home
                    || self.route == Route::Today && self.selected_weekday == 0
                {
                    self.selected_weekday = clock.weekday;
                }
                if self.selected_weekday == 0 {
                    self.selected_weekday = clock.weekday;
                }
                if changed {
                    self.bump();
                }
            }
            Action::RefreshSchedule => {
                effects.push(Effect::ReloadFromDisk);
            }
            Action::ClearSchedule => {
                effects.push(Effect::ClearStoredSchedule);
            }
            Action::SetDataStatus(status) => {
                self.data_status = status;
                self.bump();
            }
        }
        effects
    }
}
