//! PTY 管理器
//!
//! 负责创建和管理 PTY 进程

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize, PtySystem};
use std::io::{BufReader, Read, Write};

/// PTY 管理器
pub struct PtyManager {
    /// PTY 系统
    pty_system: Box<dyn PtySystem>,
}

impl PtyManager {
    /// 创建新的 PTY 管理器
    pub fn new() -> Self {
        Self {
            pty_system: native_pty_system(),
        }
    }

    /// 启动命令并返回子进程、读取器和写入器
    pub fn spawn_command(
        &self,
        command: &str,
        args: &[&str],
        size: PtySize,
    ) -> Result<(
        Box<dyn Child + Send + Sync>,
        BufReader<Box<dyn Read + Send>>,
        Box<dyn Write + Send>,
    )> {
        // 创建 PTY 对
        let pair = self
            .pty_system
            .openpty(size)
            .context("Failed to open PTY")?;

        // 构建命令
        let mut cmd = CommandBuilder::new(command);
        cmd.args(args);

        // 设置环境变量
        cmd.env("TERM", "xterm-256color");

        // 在 PTY 中启动进程
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;

        // 获取读取器和写入器
        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("Failed to take PTY writer")?;

        Ok((child, BufReader::new(reader), writer))
    }

    /// 启动 Claude 进程
    pub fn spawn_claude(
        &self,
        size: PtySize,
    ) -> Result<(
        Box<dyn Child + Send + Sync>,
        BufReader<Box<dyn Read + Send>>,
        Box<dyn Write + Send>,
    )> {
        // Claude Code CLI 命令
        // 使用交互模式启动
        self.spawn_command("claude", &[], size)
    }

    /// 启动 shell 进程 (用于测试)
    pub fn spawn_shell(
        &self,
        size: PtySize,
    ) -> Result<(
        Box<dyn Child + Send + Sync>,
        BufReader<Box<dyn Read + Send>>,
        Box<dyn Write + Send>,
    )> {
        // 检测可用的 shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        self.spawn_command(&shell, &[], size)
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}
