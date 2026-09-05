#![cfg_attr(not(feature = "std"), no_std)]

//! Loop 课表核心：与固件无关的模型、周次推算、现在/下一节与 UI snapshot。
//! 设备 crate（`loop-device`）只负责时钟、文件系统与 LVGL 适配。

extern crate alloc;

pub mod app;
pub mod model;
pub mod persistence;
pub mod schedule;
pub mod ui;

pub use app::{Action, Effect, LoopApp, Route};
pub use model::*;
pub use schedule::{ClockHint, NowNext, courses_on_day, now_and_next, term_week};
