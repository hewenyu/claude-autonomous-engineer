//! 终端视图组件
//!
//! 使用 vt100 解析器渲染 PTY 输出到 TUI 界面

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::App;

/// 渲染终端视图
pub fn render_terminal(frame: &mut Frame, area: Rect, app: &App) {
    // 创建边框
    let terminal_block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // 计算内部区域
    let inner_area = terminal_block.inner(area);

    // 获取 vt100 屏幕内容
    let lines = if let Ok(parser) = app.terminal_parser.lock() {
        let screen = parser.screen();
        render_vt100_screen(screen, inner_area.height as usize)
    } else {
        vec![Line::from("Failed to lock terminal state")]
    };

    let paragraph = Paragraph::new(lines).block(terminal_block);

    frame.render_widget(paragraph, area);
}

/// 将 vt100 屏幕转换为 ratatui Lines
fn render_vt100_screen(screen: &vt100::Screen, visible_rows: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(visible_rows);

    let total_rows = screen.size().0 as usize;
    let _scrollback = screen.scrollback(); // 预留用于滚动功能

    // 计算要显示的行范围
    // 优先显示当前屏幕内容，如果有滚动则显示滚动缓冲区
    for row in 0..visible_rows.min(total_rows) {
        let row_u16 = row as u16;
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut current_text = String::new();
        let mut current_style = Style::default();

        for col in 0..screen.size().1 {
            let cell = screen.cell(row_u16, col);

            if let Some(cell) = cell {
                let cell_style = convert_vt100_style(&cell);

                // 如果样式改变，保存当前 span 并开始新的
                if cell_style != current_style && !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                }

                current_style = cell_style;
                current_text.push(cell.contents().chars().next().unwrap_or(' '));
            } else {
                current_text.push(' ');
            }
        }

        // 添加最后一个 span
        if !current_text.is_empty() {
            // 移除行尾空格以节省空间
            let trimmed = current_text.trim_end();
            if !trimmed.is_empty() {
                spans.push(Span::styled(trimmed.to_string(), current_style));
            }
        }

        lines.push(Line::from(spans));
    }

    // 如果行数不够，用空行填充
    while lines.len() < visible_rows {
        lines.push(Line::from(""));
    }

    lines
}

/// 将 vt100 单元格样式转换为 ratatui Style
fn convert_vt100_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default();

    // 前景色
    style = style.fg(convert_vt100_color(cell.fgcolor()));

    // 背景色
    let bg = cell.bgcolor();
    if bg != vt100::Color::Default {
        style = style.bg(convert_vt100_color(bg));
    }

    // 粗体
    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }

    // 斜体
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }

    // 下划线
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    // 反转
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }

    style
}

/// 将 vt100 颜色转换为 ratatui Color
fn convert_vt100_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(0) => Color::Black,
        vt100::Color::Idx(1) => Color::Red,
        vt100::Color::Idx(2) => Color::Green,
        vt100::Color::Idx(3) => Color::Yellow,
        vt100::Color::Idx(4) => Color::Blue,
        vt100::Color::Idx(5) => Color::Magenta,
        vt100::Color::Idx(6) => Color::Cyan,
        vt100::Color::Idx(7) => Color::White,
        // 亮色 (8-15)
        vt100::Color::Idx(8) => Color::DarkGray,
        vt100::Color::Idx(9) => Color::LightRed,
        vt100::Color::Idx(10) => Color::LightGreen,
        vt100::Color::Idx(11) => Color::LightYellow,
        vt100::Color::Idx(12) => Color::LightBlue,
        vt100::Color::Idx(13) => Color::LightMagenta,
        vt100::Color::Idx(14) => Color::LightCyan,
        vt100::Color::Idx(15) => Color::White,
        // 256 色
        vt100::Color::Idx(n) => Color::Indexed(n),
        // RGB 真彩色
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_vt100_color() {
        assert_eq!(convert_vt100_color(vt100::Color::Default), Color::Reset);
        assert_eq!(convert_vt100_color(vt100::Color::Idx(1)), Color::Red);
        assert_eq!(
            convert_vt100_color(vt100::Color::Rgb(255, 128, 64)),
            Color::Rgb(255, 128, 64)
        );
    }
}
