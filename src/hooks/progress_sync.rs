//! Progress Sync Hook
//!
//! 自动从 ROADMAP.md 和任务文件同步进度到 memory.json（PostToolUse）

use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::state::{sync_from_phase_plan, sync_from_roadmap, sync_from_task_file};

/// 运行 progress_sync hook
///
/// 检测修改的文件，自动同步状态
pub fn run_progress_sync_hook(project_root: &Path, input: &Value) -> Result<Value> {
    // 获取修改的文件路径
    let file_path = extract_file_path(input);

    let file_path = match file_path {
        Some(p) => p,
        None => {
            return Ok(json!({
                "status": "ok",
                "action": "none"
            }))
        }
    };

    // 规范化路径
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let mut synced = false;
    let mut sync_type = None;

    // 检测文件类型并同步
    if filename == "ROADMAP.md" || file_path.to_string_lossy().contains("ROADMAP.md") {
        synced = sync_from_roadmap(project_root, &file_path).unwrap_or(false);
        sync_type = Some("roadmap");
    } else if filename.contains("TASK-") && filename.ends_with(".md") {
        synced = sync_from_task_file(project_root, &file_path).unwrap_or(false);
        sync_type = Some("task");
    } else if filename.contains("PHASE_PLAN") && filename.ends_with(".md") {
        synced = sync_from_phase_plan(project_root, &file_path).unwrap_or(false);
        sync_type = Some("phase");
    }

    // 输出结果
    if synced {
        Ok(json!({
            "status": "ok",
            "action": "synced",
            "sync_type": sync_type,
            "file": file_path.to_string_lossy(),
            "message": format!("Progress synced from {} file", sync_type.unwrap_or("unknown"))
        }))
    } else {
        Ok(json!({
            "status": "ok",
            "action": "none",
            "file": file_path.to_string_lossy()
        }))
    }
}

/// 从输入中提取文件路径
fn extract_file_path(input: &Value) -> Option<PathBuf> {
    // 尝试从 tool_input 中提取
    if let Some(tool_input) = input.get("tool_input") {
        if let Some(path) = tool_input.get("file_path").and_then(|p| p.as_str()) {
            return Some(PathBuf::from(path));
        }
        if let Some(path) = tool_input.get("path").and_then(|p| p.as_str()) {
            return Some(PathBuf::from(path));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_progress_sync_hook_no_file() {
        let temp = TempDir::new().unwrap();
        let input = json!({});

        let result = run_progress_sync_hook(temp.path(), &input).unwrap();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["action"], "none");
    }

    #[test]
    fn test_extract_file_path() {
        let input = json!({
            "tool_input": {
                "file_path": "/path/to/file.txt"
            }
        });

        let path = extract_file_path(&input).unwrap();
        assert_eq!(path, PathBuf::from("/path/to/file.txt"));
    }
}
