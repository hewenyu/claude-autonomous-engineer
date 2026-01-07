//! 应用状态管理
//!
//! `App` 结构体是 TUI 的核心状态容器，持有:
//! - 当前运行模式
//! - PTY 连接
//! - 终端仿真器 (vt100)
//! - 状态消息

use anyhow::Result;
use portable_pty::{Child, PtySize};
use std::io::Write;
use std::sync::{Arc, Mutex};

use crate::tui::pty::PtyManager;

/// 应用运行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// 正常模式 - 输入发送到 PTY
    Normal,
    /// 命令模式 - TUI 内部命令 (Ctrl+B 进入)
    Command,
    /// 退出确认
    Quitting,
}

/// 应用状态
pub struct App {
    /// 当前运行模式
    pub mode: AppMode,

    /// 是否应该退出
    pub should_quit: bool,

    /// PTY 管理器
    pub pty_manager: PtyManager,

    /// PTY 主端写入器
    pub pty_writer: Option<Box<dyn Write + Send>>,

    /// Claude 子进程
    pub claude_process: Option<Box<dyn Child + Send + Sync>>,

    /// 终端仿真器 (vt100) - 完整处理 ANSI/VT100 序列
    pub terminal_parser: Arc<Mutex<vt100::Parser>>,

    /// 终端尺寸
    pub terminal_size: (u16, u16),

    /// 状态消息（显示在状态栏）
    pub status_message: String,

    /// 输入缓冲区（命令模式）
    pub input_buffer: String,
}

impl App {
    /// 创建新的应用实例
    pub fn new(cols: u16, rows: u16) -> Self {
        // 创建 vt100 解析器，预留状态栏空间
        let parser_rows = rows.saturating_sub(2); // 减去边框和状态栏
        let parser = vt100::Parser::new(parser_rows, cols, 10000); // 10000 行滚动缓冲

        Self {
            mode: AppMode::Normal,
            should_quit: false,
            pty_manager: PtyManager::new(),
            pty_writer: None,
            claude_process: None,
            terminal_parser: Arc::new(Mutex::new(parser)),
            terminal_size: (cols, rows),
            status_message: String::from("Press Ctrl+Q to quit | Ctrl+B for command mode"),
            input_buffer: String::new(),
        }
    }

    /// 启动 Claude 进程
    pub fn spawn_claude(&mut self) -> Result<std::io::BufReader<Box<dyn std::io::Read + Send>>> {
        let (cols, rows) = self.terminal_size;
        let size = PtySize {
            rows: rows.saturating_sub(2), // 减去边框和状态栏
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let (child, reader, writer) = self.pty_manager.spawn_claude(size)?;
        self.claude_process = Some(child);
        self.pty_writer = Some(writer);
        self.status_message = "Claude started | Ctrl+Q to quit".to_string();

        Ok(reader)
    }

    /// 启动 shell 进程 (用于测试)
    pub fn spawn_shell(&mut self) -> Result<std::io::BufReader<Box<dyn std::io::Read + Send>>> {
        let (cols, rows) = self.terminal_size;
        let size = PtySize {
            rows: rows.saturating_sub(2),
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let (child, reader, writer) = self.pty_manager.spawn_shell(size)?;
        self.claude_process = Some(child);
        self.pty_writer = Some(writer);
        self.status_message = "Shell started | Ctrl+Q to quit".to_string();

        Ok(reader)
    }

    /// 发送输入到 PTY
    pub fn send_input(&mut self, data: &[u8]) -> Result<()> {
        if let Some(ref mut writer) = self.pty_writer {
            writer.write_all(data)?;
            writer.flush()?;
        }
        Ok(())
    }

    /// 处理 PTY 输出 - 通过 vt100 解析器处理
    pub fn process_pty_output(&self, data: &[u8]) {
        if let Ok(mut parser) = self.terminal_parser.lock() {
            parser.process(data);
        }
    }

    /// 调整终端大小
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.terminal_size = (cols, rows);

        let parser_rows = rows.saturating_sub(2);
        if let Ok(mut parser) = self.terminal_parser.lock() {
            parser.set_size(parser_rows, cols);
        }

        // 通知 PTY 大小变化
        if let Some(ref mut writer) = self.pty_writer {
            // 发送 SIGWINCH 信号 - portable-pty 会自动处理
            // 这里我们只需要更新内部状态
            let _ = writer.flush();
        }

        Ok(())
    }

    /// 检查进程是否还在运行
    pub fn is_process_running(&mut self) -> bool {
        if let Some(ref mut child) = self.claude_process {
            match child.try_wait() {
                Ok(Some(_)) => false, // 进程已退出
                Ok(None) => true,     // 进程仍在运行
                Err(_) => false,      // 错误，假设已退出
            }
        } else {
            false
        }
    }
}
