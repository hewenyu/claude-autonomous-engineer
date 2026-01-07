//! 终端视图组件
//!
//! 渲染 PTY 输出到 TUI 界面

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::App;

/// 渲染终端视图
pub fn render_terminal(frame: &mut Frame, area: Rect, app: &App) {
    // 获取终端输出
    let output_text = if let Ok(state) = app.terminal_state.lock() {
        state.get_text()
    } else {
        String::from("Failed to lock terminal state")
    };

    // 处理 ANSI 转义序列 (简化版 - 直接显示文本)
    // TODO: 使用 vt100 或 ansi-to-tui 进行完整解析
    let cleaned_text = strip_ansi_codes(&output_text);

    // 获取最后 N 行以适应显示区域
    let lines: Vec<&str> = cleaned_text.lines().collect();
    let visible_lines = area.height.saturating_sub(2) as usize; // 减去边框
    let start = lines.len().saturating_sub(visible_lines);
    let display_lines: Vec<Line> = lines[start..]
        .iter()
        .map(|line| Line::from(*line))
        .collect();

    let text = Text::from(display_lines);

    // 创建终端视图
    let terminal_block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(text)
        .block(terminal_block)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));

    frame.render_widget(paragraph, area);
}

/// 简单的 ANSI 转义序列剥离
/// TODO: 替换为完整的 ANSI 解析器
fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC 字符，跳过转义序列
            if chars.peek() == Some(&'[') {
                chars.next(); // 消费 '['
                // 跳过直到遇到字母
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_alphabetic() {
                        break;
                    }
                }
            }
        } else if c == '\r' {
            // 忽略回车符，只保留换行
            continue;
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_codes() {
        let input = "\x1b[32mHello\x1b[0m World";
        let output = strip_ansi_codes(input);
        assert_eq!(output, "Hello World");
    }

    #[test]
    fn test_strip_carriage_return() {
        let input = "Hello\r\nWorld";
        let output = strip_ansi_codes(input);
        assert_eq!(output, "Hello\nWorld");
    }
}
