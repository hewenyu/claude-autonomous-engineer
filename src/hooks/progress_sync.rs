//! Progress Sync Hook
//!
//! 自动从 ROADMAP.md 和任务文件同步进度到 memory.json（PostToolUse）

use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::state::{sync_from_phase_plan, sync_from_roadmap, sync_from_task_file};
use crate::utils::{read_json, write_json};
use crate::Memory;

/// 运行 progress_sync hook
///
/// 检测修改的文件，自动同步状态
pub fn run_progress_sync_hook(project_root: &Path, input: &Value) -> Result<Value> {
    // 获取修改的文件路径
    let file_path = extract_file_path(input);

    let file_path = match file_path {
        Some(p) => p,
        None => {
            return Ok(noop_posttooluse_output());
        }
    };

    // 规范化路径
    let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

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

    // Best-effort: track the most recently modified file and keep a rolling list.
    // This reduces reliance on the model to manually maintain working_context.
    let _ = record_modified_file(project_root, &file_path);

    // Hook 输出必须符合 Claude Code 的 schema（避免 JSON 校验失败）。
    // 该 hook 仅做副作用（同步文件进度），不需要额外上下文注入。
    let _ = (synced, sync_type);
    Ok(noop_posttooluse_output())
}

fn noop_posttooluse_output() -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse"
        }
    })
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

fn record_modified_file(project_root: &Path, file_path: &Path) -> Result<()> {
    let memory_path = project_root.join(".claude/status/memory.json");
    let mut memory: Memory = read_json(&memory_path).unwrap_or_default();

    let path_str = file_path.to_string_lossy().to_string();

    memory.working_context.current_file = Some(path_str.clone());

    if !memory
        .working_context
        .modified_files
        .iter()
        .any(|p| p == &path_str)
    {
        memory.working_context.modified_files.push(path_str.clone());
        // Prevent unbounded growth
        if memory.working_context.modified_files.len() > 50 {
            let start = memory.working_context.modified_files.len() - 50;
            memory.working_context.modified_files =
                memory.working_context.modified_files[start..].to_vec();
        }
    }

    if !memory.active_files.iter().any(|p| p == &path_str) {
        memory.active_files.push(path_str);
        if memory.active_files.len() > 20 {
            let start = memory.active_files.len() - 20;
            memory.active_files = memory.active_files[start..].to_vec();
        }
    }

    memory.last_updated = Some(Utc::now().to_rfc3339());

    write_json(&memory_path, &memory)?;
    Ok(())
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
        assert_eq!(
            result["hookSpecificOutput"]["hookEventName"],
            "PostToolUse"
        );
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

    #[test]
    fn test_record_modified_file_updates_memory() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();
        std::fs::write(
            temp.path().join(".claude/status/memory.json"),
            r#"{ "project": "x", "version": "1", "mode": "autonomous" }"#,
        )
        .unwrap();

        record_modified_file(temp.path(), Path::new("src/lib.rs")).unwrap();

        let mem: Memory = read_json(&temp.path().join(".claude/status/memory.json")).unwrap();
        assert_eq!(
            mem.working_context.current_file,
            Some("src/lib.rs".to_string())
        );
        assert!(mem
            .working_context
            .modified_files
            .iter()
            .any(|p| p == "src/lib.rs"));
    }
}
