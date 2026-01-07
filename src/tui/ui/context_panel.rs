//! 上下文面板 (Phase 2)
//!
//! 显示项目上下文摘要信息，包括：
//! - 当前任务和进度
//! - 文件变更状态
//! - repo_map 状态
//! - 错误计数

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::App;

/// 上下文面板宽度
pub const CONTEXT_PANEL_WIDTH: u16 = 28;

/// 渲染上下文面板
pub fn render_context_panel(frame: &mut Frame, area: Rect, app: &App) {
    let summary = &app.context_summary;

    // 构建显示内容
    let mut lines = Vec::new();

    // 标题区域
    lines.push(Line::from(vec![
        Span::styled(" Context ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(""));

    // 当前任务
    lines.push(Line::from(vec![
        Span::styled(" Task ", Style::default().fg(Color::Yellow)),
    ]));

    if let Some(ref task_id) = summary.current_task {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(truncate_str(task_id, 22), Style::default().fg(Color::White)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("No active task", Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::from(""));

    // 进度条
    lines.push(Line::from(vec![
        Span::styled(" Progress ", Style::default().fg(Color::Yellow)),
    ]));

    let (completed, total) = summary.progress;
    if total > 0 {
        let pct = (completed as f64 / total as f64 * 100.0) as u8;
        let bar_width = 20;
        let filled = (bar_width as f64 * completed as f64 / total as f64) as usize;

        let bar: String = format!(
            "[{}{}]",
            "█".repeat(filled),
            "░".repeat(bar_width - filled)
        );

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(bar, Style::default().fg(Color::Green)),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{}/{} ({}%)", completed, total, pct),
                Style::default().fg(Color::White),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("No tasks", Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::from(""));

    // 文件变更
    lines.push(Line::from(vec![
        Span::styled(" Changes ", Style::default().fg(Color::Yellow)),
    ]));

    if summary.recent_changes > 0 {
        let change_style = if summary.repo_map_stale {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{} files", summary.recent_changes), change_style),
        ]));

        if summary.repo_map_stale {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("⚠ Map stale", Style::default().fg(Color::Red)),
            ]));
        }
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("No changes", Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::from(""));

    // 错误状态
    lines.push(Line::from(vec![
        Span::styled(" Errors ", Style::default().fg(Color::Yellow)),
    ]));

    if summary.error_count > 0 {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} errors", summary.error_count),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("No errors", Style::default().fg(Color::Green)),
        ]));
    }

    // 底部提示
    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(" Ctrl+P ", Style::default().fg(Color::DarkGray)),
        Span::styled("hide", Style::default().fg(Color::DarkGray)),
    ]));

    // 创建面板
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}

/// 截断字符串
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("short", 10), "short");
        assert_eq!(truncate_str("this is a long string", 10), "this is...");
    }
}
