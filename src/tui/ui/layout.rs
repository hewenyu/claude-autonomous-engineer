//! 布局管理
//!
//! 定义 TUI 的整体布局结构

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::tui::App;

use super::{render_status_bar, render_terminal};

/// 渲染主界面
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 垂直分割: 终端区域 + 状态栏
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // 终端区域 (占用剩余空间)
            Constraint::Length(1), // 状态栏 (1 行)
        ])
        .split(area);

    // 渲染终端视图
    render_terminal(frame, chunks[0], app);

    // 渲染状态栏
    render_status_bar(frame, chunks[1], app);
}
