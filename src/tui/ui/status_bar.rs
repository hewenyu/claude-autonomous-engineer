//! 状态栏组件
//!
//! 显示当前模式和状态信息

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::{App, AppMode};

/// 渲染状态栏
pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    // 模式指示器
    let mode_span = match app.mode {
        AppMode::Normal => Span::styled(
            " NORMAL ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        AppMode::Command => Span::styled(
            " COMMAND ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        AppMode::Quitting => Span::styled(
            " QUIT? ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
    };

    // 状态消息
    let status_span = Span::styled(
        format!(" {} ", app.status_message),
        Style::default().fg(Color::White),
    );

    // 命令模式下显示输入缓冲区
    let input_span = if app.mode == AppMode::Command && !app.input_buffer.is_empty() {
        Span::styled(
            format!(" > {} ", app.input_buffer),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    // 组合状态栏
    let line = Line::from(vec![mode_span, status_span, input_span]);

    let status_bar = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(status_bar, area);
}
