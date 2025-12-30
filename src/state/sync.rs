//! 状态同步逻辑
//!
//! 从 ROADMAP.md 等文件同步到 memory.json（将在阶段 2 实现）

use anyhow::Result;
use std::path::Path;

/// 从 ROADMAP.md 同步（占位符）
pub fn sync_from_roadmap(_project_root: &Path, _roadmap_path: &Path) -> Result<bool> {
    Ok(false)
}
