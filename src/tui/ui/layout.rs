//! 布局管理
//!
//! 定义 TUI 的整体布局结构
//!
//! Phase 2: 支持可选的上下文面板
//! ```text
//! ┌──────────────────┬───────────────┐
//! │                  │   Context     │
//! │   Terminal       │   Panel       │
//! │                  │   (右侧)      │
//! ├──────────────────┴───────────────┤
//! │           Status Bar             │
//! └──────────────────────────────────┘
//! ```

use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use crate::tui::App;

use super::{render_context_panel, render_status_bar, render_terminal, CONTEXT_PANEL_WIDTH};

/// 渲染主界面
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // 垂直分割: 主内容区域 + 状态栏
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // 主内容区域 (占用剩余空间)
            Constraint::Length(1), // 状态栏 (1 行)
        ])
        .split(area);

    let main_area = vertical_chunks[0];
    let status_area = vertical_chunks[1];

    // Phase 2: 根据是否显示上下文面板决定布局
    if app.show_context_panel {
        // 水平分割: 终端 + 上下文面板
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(40),                     // 终端 (最小 40 列)
                Constraint::Length(CONTEXT_PANEL_WIDTH), // 上下文面板 (固定宽度)
            ])
            .split(main_area);

        render_terminal(frame, horizontal_chunks[0], app);
        render_context_panel(frame, horizontal_chunks[1], app);
    } else {
        // 无上下文面板，终端占满
        render_terminal(frame, main_area, app);
    }

    // 渲染状态栏
    render_status_bar(frame, status_area, app);
}
