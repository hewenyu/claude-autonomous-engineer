//! UI 组件模块
//!
//! 包含所有 TUI 界面组件

mod layout;
mod status_bar;
mod terminal_view;

pub use layout::render;
pub use status_bar::render_status_bar;
pub use terminal_view::render_terminal;
