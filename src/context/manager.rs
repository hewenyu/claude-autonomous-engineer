//! 上下文管理器核心
//!
//! ContextManager - 统一上下文管理（将在阶段 3 实现）

use anyhow::Result;
use std::path::PathBuf;

/// 上下文管理器（占位符）
pub struct ContextManager {
    pub project_root: PathBuf,
}

impl ContextManager {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn get_full_context(&self) -> Result<String> {
        Ok(String::from("Context manager will be implemented in Phase 3"))
    }
}
