//! 应用状态管理
//!
//! `App` 结构体是 TUI 的核心状态容器，持有:
//! - 当前运行模式
//! - PTY 连接
//! - 终端仿真器 (vt100)
//! - 状态消息
//! - 上下文管理器 (Phase 2)

use anyhow::Result;
use portable_pty::{Child, PtySize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::context::ContextManager;
use crate::repo_map::service::{RepoMapService, UpdateStatus};
use crate::tui::pty::PtyManager;
use crate::watcher::{FileChange, FileChangeKind};

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

/// 上下文摘要 (Phase 2: 用于 UI 显示)
#[derive(Debug, Clone, Default)]
pub struct ContextSummary {
    /// 当前任务 ID
    pub current_task: Option<String>,
    /// 任务进度 (完成/总数)
    pub progress: (usize, usize),
    /// 最近变更的文件数
    pub recent_changes: usize,
    /// repo_map 是否需要更新
    pub repo_map_stale: bool,
    /// 错误计数
    pub error_count: usize,
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

    // ═══════════════════════════════════════════════════════════════════
    // Phase 2: 上下文引擎
    // ═══════════════════════════════════════════════════════════════════
    /// 项目根目录
    pub project_root: Option<PathBuf>,

    /// 上下文管理器
    pub context_manager: Option<ContextManager>,

    /// 上下文摘要 (用于 UI 显示)
    pub context_summary: ContextSummary,

    /// 待处理的文件变更
    pub pending_file_changes: Vec<FileChange>,

    /// 是否显示上下文面板
    pub show_context_panel: bool,

    /// RepoMap 后台更新服务
    pub repo_map_service: Option<RepoMapService>,
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
            // Phase 2 fields
            project_root: None,
            context_manager: None,
            context_summary: ContextSummary::default(),
            pending_file_changes: Vec::new(),
            show_context_panel: false,
            repo_map_service: None,
        }
    }

    /// 创建带项目上下文的应用实例 (Phase 2)
    pub fn with_project(cols: u16, rows: u16, project_root: PathBuf) -> Self {
        let mut app = Self::new(cols, rows);
        app.project_root = Some(project_root.clone());
        app.context_manager = Some(ContextManager::new(project_root.clone()));
        app.show_context_panel = true;

        // 初始化 RepoMap 服务
        let output_path = project_root.join(".claude/repo_map/structure.toon");
        if let Ok(service) = RepoMapService::start(project_root, output_path) {
            // 启动时触发一次全量更新
            let _ = service.request_full();
            app.repo_map_service = Some(service);
        }

        app.refresh_context_summary();
        app
    }

    /// 刷新上下文摘要 (Phase 2)
    pub fn refresh_context_summary(&mut self) {
        if let Some(ref manager) = self.context_manager {
            // 读取 memory.json 获取当前状态
            let memory_path = manager.project_root.join(".claude/status/memory.json");
            if let Ok(content) = std::fs::read_to_string(&memory_path) {
                if let Ok(memory) = serde_json::from_str::<crate::state::Memory>(&content) {
                    self.context_summary.current_task = memory.current_task.id.clone();
                    self.context_summary.progress =
                        (memory.progress.tasks_completed, memory.progress.tasks_total);
                    self.context_summary.error_count = memory.error_state.error_count as usize;
                }
            }
        }
    }

    /// 处理文件变更事件 (Phase 2)
    pub fn handle_file_changes(&mut self, changes: Vec<FileChange>) {
        let code_changes: Vec<_> = changes
            .iter()
            .filter(|c| c.needs_repo_map_update())
            .cloned()
            .collect();

        if !code_changes.is_empty() {
            self.context_summary.repo_map_stale = true;
            self.context_summary.recent_changes += code_changes.len();

            // 触发增量 repo_map 更新
            if let Some(ref service) = self.repo_map_service {
                let paths: Vec<_> = code_changes.iter().map(|c| c.path.clone()).collect();
                if service.request_incremental(paths).is_ok() {
                    // 请求已发送，标记为更新中
                    self.context_summary.repo_map_stale = false;
                }
            }
        }

        // 如果有状态文件变更，刷新上下文摘要
        let has_state_changes = changes
            .iter()
            .any(|c| matches!(c.kind, FileChangeKind::State));

        if has_state_changes {
            self.refresh_context_summary();
        }

        self.pending_file_changes.extend(changes);
    }

    /// 获取 RepoMap 服务状态 (Phase 2)
    pub fn repo_map_status(&self) -> Option<UpdateStatus> {
        self.repo_map_service.as_ref().map(|s| s.status())
    }

    /// RepoMap 是否正在更新 (Phase 2)
    pub fn is_repo_map_updating(&self) -> bool {
        self.repo_map_service
            .as_ref()
            .map(|s| s.is_updating())
            .unwrap_or(false)
    }

    /// 切换上下文面板显示 (Phase 2)
    pub fn toggle_context_panel(&mut self) {
        self.show_context_panel = !self.show_context_panel;
        if self.show_context_panel {
            self.status_message = "Context panel ON | Ctrl+P to toggle".to_string();
        } else {
            self.status_message = "Context panel OFF | Ctrl+P to toggle".to_string();
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
