//! 文件监听模块
//!
//! 监听项目文件变更，触发 repo_map 和上下文更新。
//! 这是第二阶段上下文引擎的核心组件。

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crate::repo_map::is_supported_extension;

// ═══════════════════════════════════════════════════════════════════
// 文件变更事件
// ═══════════════════════════════════════════════════════════════════

/// 文件变更类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeKind {
    /// 代码文件变更 (需要更新 repo_map)
    Code,
    /// 状态文件变更 (需要更新上下文)
    State,
    /// 配置文件变更
    Config,
    /// 其他文件
    Other,
}

/// 文件变更事件
#[derive(Debug, Clone)]
pub struct FileChange {
    /// 变更的文件路径
    pub path: PathBuf,
    /// 变更类型
    pub kind: FileChangeKind,
}

impl FileChange {
    /// 从路径推断变更类型
    pub fn from_path(path: PathBuf) -> Self {
        let kind = Self::classify_path(&path);
        Self { path, kind }
    }

    /// 分类文件路径
    fn classify_path(path: &Path) -> FileChangeKind {
        let path_str = path.to_string_lossy();

        // 状态文件
        if path_str.contains(".claude/status")
            || path_str.ends_with("memory.json")
            || path_str.ends_with("ROADMAP.md")
        {
            return FileChangeKind::State;
        }

        // 配置文件
        if path_str.ends_with("config.toml")
            || path_str.ends_with("settings.json")
            || path_str.ends_with(".gitignore")
        {
            return FileChangeKind::Config;
        }

        // 代码文件：使用 repo_map 支持的扩展名列表
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if is_supported_extension(ext) {
                return FileChangeKind::Code;
            }
        }

        FileChangeKind::Other
    }

    /// 是否需要更新 repo_map
    pub fn needs_repo_map_update(&self) -> bool {
        matches!(self.kind, FileChangeKind::Code)
    }

    /// 是否需要更新上下文
    pub fn needs_context_update(&self) -> bool {
        matches!(self.kind, FileChangeKind::Code | FileChangeKind::State)
    }
}

// ═══════════════════════════════════════════════════════════════════
// FileWatcher
// ═══════════════════════════════════════════════════════════════════

/// 文件监听器
///
/// 使用 notify crate 监听文件系统变更，
/// 通过 debounce 避免短时间内大量事件。
pub struct FileWatcher {
    /// 项目根目录
    project_root: PathBuf,
    /// debouncer (持有 watcher)
    _debouncer: Debouncer<RecommendedWatcher>,
    /// 事件接收端
    rx: Receiver<Result<Vec<DebouncedEvent>, notify::Error>>,
}

impl FileWatcher {
    /// 创建新的文件监听器
    ///
    /// # Arguments
    /// * `project_root` - 项目根目录
    /// * `debounce_ms` - 防抖延迟（毫秒）
    pub fn new(project_root: PathBuf, debounce_ms: u64) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut debouncer = new_debouncer(Duration::from_millis(debounce_ms), tx)?;

        // 监听项目根目录
        debouncer
            .watcher()
            .watch(&project_root, RecursiveMode::Recursive)?;

        Ok(Self {
            project_root,
            _debouncer: debouncer,
            rx,
        })
    }

    /// 创建默认配置的监听器 (500ms debounce)
    pub fn default(project_root: PathBuf) -> Result<Self> {
        Self::new(project_root, 500)
    }

    /// 尝试获取文件变更事件（非阻塞）
    ///
    /// 返回自上次调用以来的所有变更事件
    pub fn poll_changes(&self) -> Vec<FileChange> {
        let mut changes = Vec::new();

        // 非阻塞接收所有待处理事件
        while let Ok(result) = self.rx.try_recv() {
            match result {
                Ok(events) => {
                    for event in events {
                        // 过滤隐藏目录（如 .git）
                        if self.should_ignore(&event.path) {
                            continue;
                        }
                        changes.push(FileChange::from_path(event.path));
                    }
                }
                Err(e) => {
                    eprintln!("File watcher error: {:?}", e);
                }
            }
        }

        // 去重（同一文件可能有多个事件）
        changes.dedup_by(|a, b| a.path == b.path);
        changes
    }

    /// 阻塞等待文件变更事件
    ///
    /// 返回下一批变更事件
    pub fn wait_for_changes(&self) -> Result<Vec<FileChange>> {
        let result = self.rx.recv()?;

        match result {
            Ok(events) => {
                let changes: Vec<_> = events
                    .into_iter()
                    .filter(|e| !self.should_ignore(&e.path))
                    .map(|e| FileChange::from_path(e.path))
                    .collect();
                Ok(changes)
            }
            Err(e) => Err(anyhow::anyhow!("File watcher error: {:?}", e)),
        }
    }

    /// 判断是否应该忽略该路径
    fn should_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // 忽略 .git 目录
        if path_str.contains("/.git/") || path_str.contains("\\.git\\") {
            return true;
        }

        // 忽略 target 目录（Rust 编译输出）
        if path_str.contains("/target/") || path_str.contains("\\target\\") {
            return true;
        }

        // 忽略 node_modules
        if path_str.contains("/node_modules/") || path_str.contains("\\node_modules\\") {
            return true;
        }

        // 忽略临时文件
        if path_str.ends_with(".swp") || path_str.ends_with(".tmp") || path_str.ends_with("~") {
            return true;
        }

        false
    }

    /// 获取项目根目录
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_change_classification() {
        // 代码文件
        let rust_file = FileChange::from_path(PathBuf::from("/project/src/main.rs"));
        assert_eq!(rust_file.kind, FileChangeKind::Code);
        assert!(rust_file.needs_repo_map_update());

        let py_file = FileChange::from_path(PathBuf::from("/project/app.py"));
        assert_eq!(py_file.kind, FileChangeKind::Code);

        // 状态文件
        let memory = FileChange::from_path(PathBuf::from("/project/.claude/status/memory.json"));
        assert_eq!(memory.kind, FileChangeKind::State);
        assert!(memory.needs_context_update());
        assert!(!memory.needs_repo_map_update());

        let roadmap = FileChange::from_path(PathBuf::from("/project/.claude/status/ROADMAP.md"));
        assert_eq!(roadmap.kind, FileChangeKind::State);

        // 配置文件
        let config = FileChange::from_path(PathBuf::from("/project/config.toml"));
        assert_eq!(config.kind, FileChangeKind::Config);

        // 其他文件
        let readme = FileChange::from_path(PathBuf::from("/project/README.md"));
        assert_eq!(readme.kind, FileChangeKind::Other);
    }

    #[test]
    fn test_should_ignore() {
        let watcher_result = FileWatcher::new(PathBuf::from("/tmp"), 100);

        // 如果创建成功则测试忽略逻辑
        if let Ok(watcher) = watcher_result {
            assert!(watcher.should_ignore(Path::new("/project/.git/objects/abc")));
            assert!(watcher.should_ignore(Path::new("/project/target/debug/main")));
            assert!(watcher.should_ignore(Path::new("/project/node_modules/lodash/index.js")));
            assert!(watcher.should_ignore(Path::new("/project/file.swp")));

            assert!(!watcher.should_ignore(Path::new("/project/src/main.rs")));
            assert!(!watcher.should_ignore(Path::new("/project/.claude/status/memory.json")));
        }
    }
}
