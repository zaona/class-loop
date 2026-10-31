#![cfg_attr(not(feature = "std"), no_std)]

//! Loop 课表核心：模型、周次推算和页面视图。
//! 设备 crate（`loop-device`）负责时钟、文件系统与 Canopus 系统组件适配。

extern crate alloc;

pub mod app;
pub mod model;
pub mod persistence;
pub mod schedule;
pub mod view;

pub use app::{Action, Effect, LoopApp, Route};
pub use model::*;
pub use schedule::{ClockHint, NowNext, courses_on_day, now_and_next, term_week};
