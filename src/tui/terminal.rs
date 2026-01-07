//! 终端初始化与恢复
//!
//! 处理终端的 raw mode 和 alternate screen 设置

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

/// 终端类型别名
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// 初始化终端
///
/// - 进入 raw mode（禁用行缓冲和回显）
/// - 进入 alternate screen（保护原始终端内容）
/// - 启用鼠标捕获（可选）
pub fn init_terminal() -> Result<Tui> {
    // 进入 raw mode
    enable_raw_mode()?;

    // 进入 alternate screen
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // 创建后端和终端
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

/// 恢复终端
///
/// - 退出 alternate screen
/// - 退出 raw mode
/// - 显示光标
pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// 安装 panic hook，确保在 panic 时恢复终端
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic| {
        // 尝试恢复终端
        let _ = restore_terminal();
        // 调用原始 hook
        original_hook(panic);
    }));
}
