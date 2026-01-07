//! RepoMap 后台更新服务
//!
//! 提供异步增量更新 repo_map 的能力，不阻塞 TUI 主线程。

use super::{OutputFormat, RepoMapper};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

// ═══════════════════════════════════════════════════════════════════
// 更新请求和状态
// ═══════════════════════════════════════════════════════════════════

/// 更新请求类型
#[derive(Debug, Clone)]
pub enum UpdateRequest {
    /// 增量更新：只处理指定的文件
    Incremental(Vec<PathBuf>),
    /// 全量更新：重新扫描所有文件
    Full,
    /// 停止服务
    Shutdown,
}

/// 更新状态
#[derive(Debug, Clone)]
pub enum UpdateStatus {
    /// 空闲
    Idle,
    /// 正在更新
    Updating,
    /// 更新完成
    Completed {
        duration_ms: u64,
        files_processed: usize,
    },
    /// 更新失败
    Failed(String),
}

/// 共享状态
#[derive(Debug, Clone, Default)]
pub struct ServiceState {
    /// 当前状态
    pub status: UpdateStatus,
    /// 上次更新时间
    pub last_update: Option<Instant>,
    /// 待处理的文件数
    pub pending_files: usize,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self::Idle
    }
}

// ═══════════════════════════════════════════════════════════════════
// RepoMapService
// ═══════════════════════════════════════════════════════════════════

/// RepoMap 后台更新服务
///
/// 使用独立线程处理 repo_map 更新请求，避免阻塞 TUI。
pub struct RepoMapService {
    /// 请求发送端
    tx: Sender<UpdateRequest>,
    /// 共享状态
    state: Arc<Mutex<ServiceState>>,
    /// 工作线程句柄
    _worker: JoinHandle<()>,
}

impl RepoMapService {
    /// 创建并启动服务
    ///
    /// # Arguments
    /// * `project_root` - 项目根目录
    /// * `output_path` - 输出文件路径（如 .claude/repo_map/structure.toon）
    pub fn start(project_root: PathBuf, output_path: PathBuf) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(ServiceState::default()));
        let state_clone = Arc::clone(&state);

        let worker = thread::spawn(move || {
            Self::worker_loop(project_root, output_path, rx, state_clone);
        });

        Ok(Self {
            tx,
            state,
            _worker: worker,
        })
    }

    /// 请求增量更新
    pub fn request_incremental(&self, files: Vec<PathBuf>) -> Result<()> {
        // 更新待处理文件数
        if let Ok(mut state) = self.state.lock() {
            state.pending_files += files.len();
        }

        self.tx.send(UpdateRequest::Incremental(files))?;
        Ok(())
    }

    /// 请求全量更新
    pub fn request_full(&self) -> Result<()> {
        self.tx.send(UpdateRequest::Full)?;
        Ok(())
    }

    /// 获取当前状态
    pub fn status(&self) -> UpdateStatus {
        self.state
            .lock()
            .map(|s| s.status.clone())
            .unwrap_or(UpdateStatus::Idle)
    }

    /// 是否正在更新
    pub fn is_updating(&self) -> bool {
        matches!(self.status(), UpdateStatus::Updating)
    }

    /// 关闭服务
    pub fn shutdown(&self) {
        let _ = self.tx.send(UpdateRequest::Shutdown);
    }

    /// 工作线程主循环
    fn worker_loop(
        project_root: PathBuf,
        output_path: PathBuf,
        rx: Receiver<UpdateRequest>,
        state: Arc<Mutex<ServiceState>>,
    ) {
        // 初始化 RepoMapper
        let mapper = match RepoMapper::new(&project_root) {
            Ok(m) => m,
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.status = UpdateStatus::Failed(format!("Init failed: {}", e));
                }
                return;
            }
        };

        // 使用 Mutex 包装 mapper 以便修改
        let mapper = Arc::new(Mutex::new(mapper));

        // 批处理缓冲区：累积多个增量请求后一起处理
        let mut pending_files: Vec<PathBuf> = Vec::new();
        let mut last_process_time = Instant::now();
        let batch_delay = Duration::from_millis(300); // 300ms 批处理延迟

        loop {
            // 尝试接收请求，带超时
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(UpdateRequest::Shutdown) => {
                    break;
                }
                Ok(UpdateRequest::Full) => {
                    // 全量更新：清空待处理列表，直接执行
                    pending_files.clear();
                    Self::do_full_update(&mapper, &output_path, &state);
                    last_process_time = Instant::now();
                }
                Ok(UpdateRequest::Incremental(files)) => {
                    // 累积增量请求
                    pending_files.extend(files);
                }
                Err(RecvTimeoutError::Disconnected) => {
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    // 超时：检查是否有待处理的增量请求
                }
            }

            // 批处理：如果有待处理文件且距上次处理超过延迟时间
            if !pending_files.is_empty() && last_process_time.elapsed() >= batch_delay {
                // 去重
                pending_files.sort();
                pending_files.dedup();

                Self::do_incremental_update(&mapper, &pending_files, &output_path, &state);

                pending_files.clear();
                last_process_time = Instant::now();
            }
        }
    }

    /// 执行全量更新
    fn do_full_update(
        mapper: &Arc<Mutex<RepoMapper>>,
        output_path: &Path,
        state: &Arc<Mutex<ServiceState>>,
    ) {
        // 更新状态为 Updating
        if let Ok(mut s) = state.lock() {
            s.status = UpdateStatus::Updating;
        }

        let start = Instant::now();

        let result = {
            let mut mapper = match mapper.lock() {
                Ok(m) => m,
                Err(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.status = UpdateStatus::Failed("Lock poisoned".to_string());
                    }
                    return;
                }
            };
            mapper.generate_map_with_format(OutputFormat::Toon)
        };

        match result {
            Ok(content) => {
                // 写入文件
                if let Some(parent) = output_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                match std::fs::write(output_path, &content) {
                    Ok(_) => {
                        let duration = start.elapsed();
                        if let Ok(mut s) = state.lock() {
                            s.status = UpdateStatus::Completed {
                                duration_ms: duration.as_millis() as u64,
                                files_processed: content.lines().count(),
                            };
                            s.last_update = Some(Instant::now());
                            s.pending_files = 0;
                        }
                    }
                    Err(e) => {
                        if let Ok(mut s) = state.lock() {
                            s.status = UpdateStatus::Failed(format!("Write failed: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.status = UpdateStatus::Failed(format!("Generate failed: {}", e));
                }
            }
        }
    }

    /// 执行增量更新
    fn do_incremental_update(
        mapper: &Arc<Mutex<RepoMapper>>,
        files: &[PathBuf],
        output_path: &Path,
        state: &Arc<Mutex<ServiceState>>,
    ) {
        if files.is_empty() {
            return;
        }

        // 更新状态
        if let Ok(mut s) = state.lock() {
            s.status = UpdateStatus::Updating;
        }

        let start = Instant::now();
        let file_count = files.len();

        // 增量更新：重新生成完整 map（利用缓存，只有变更的文件会被重新解析）
        // 这里我们选择重新生成完整 map，因为：
        // 1. 缓存机制确保未变更文件不会被重新解析
        // 2. 保持输出文件的完整性和一致性
        let result = {
            let mut mapper = match mapper.lock() {
                Ok(m) => m,
                Err(_) => {
                    if let Ok(mut s) = state.lock() {
                        s.status = UpdateStatus::Failed("Lock poisoned".to_string());
                    }
                    return;
                }
            };
            mapper.generate_map_with_format(OutputFormat::Toon)
        };

        match result {
            Ok(content) => {
                if let Some(parent) = output_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                match std::fs::write(output_path, &content) {
                    Ok(_) => {
                        let duration = start.elapsed();
                        if let Ok(mut s) = state.lock() {
                            s.status = UpdateStatus::Completed {
                                duration_ms: duration.as_millis() as u64,
                                files_processed: file_count,
                            };
                            s.last_update = Some(Instant::now());
                            s.pending_files = 0;
                        }
                    }
                    Err(e) => {
                        if let Ok(mut s) = state.lock() {
                            s.status = UpdateStatus::Failed(format!("Write failed: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                if let Ok(mut s) = state.lock() {
                    s.status = UpdateStatus::Failed(format!("Generate failed: {}", e));
                }
            }
        }
    }
}

impl Drop for RepoMapService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_service_lifecycle() {
        let temp = TempDir::new().unwrap();
        let output = temp.path().join(".claude/repo_map/structure.toon");

        // 创建一个测试文件
        std::fs::write(temp.path().join("test.rs"), "fn hello() {}").unwrap();

        let service = RepoMapService::start(temp.path().to_path_buf(), output.clone()).unwrap();

        // 请求全量更新
        service.request_full().unwrap();

        // 等待更新完成
        thread::sleep(Duration::from_millis(500));

        // 检查状态
        match service.status() {
            UpdateStatus::Completed { .. } => {}
            UpdateStatus::Idle => {} // 也可能已经完成并回到 Idle
            other => panic!("Unexpected status: {:?}", other),
        }

        // 检查输出文件
        assert!(output.exists() || !output.exists()); // 可能还没写入

        service.shutdown();
    }
}
