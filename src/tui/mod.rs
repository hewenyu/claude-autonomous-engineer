//! TUI 模块 - 基于 Ratatui 的多智能体会话管理系统
//!
//! 核心组件:
//! - `app`: 应用状态管理
//! - `event`: 事件系统 (键盘、PTY、定时器)
//! - `terminal`: 终端初始化与恢复
//! - `pty`: PTY 进程管理
//! - `ui`: 用户界面组件

pub mod app;
pub mod event;
pub mod pty;
pub mod terminal;
pub mod ui;

// 重导出常用类型
pub use app::{App, AppMode};
pub use event::{Event, EventHandler};
pub use terminal::{init_terminal, install_panic_hook, restore_terminal};
