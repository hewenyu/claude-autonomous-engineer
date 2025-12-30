//! Hook 统一执行器

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, Read};
use std::path::Path;

use super::{
    run_codex_review_gate_hook, run_inject_state_hook, run_loop_driver_hook,
    run_progress_sync_hook,
};

/// 运行指定的 hook
///
/// 这是所有 hook 的统一入口点
pub fn run_hook(hook_name: &str, project_root: &Path, input: Option<&Value>) -> Result<Value> {
    let default_input = json!({});

    match hook_name {
        "inject_state" => run_inject_state_hook(project_root),

        "progress_sync" | "post_write_update" => {
            let input_data = input.unwrap_or(&default_input);
            run_progress_sync_hook(project_root, input_data)
        }

        "codex_review_gate" | "pre_write_check" => {
            let input_data = input.unwrap_or(&default_input);
            run_codex_review_gate_hook(project_root, input_data)
        }

        "loop_driver" => run_loop_driver_hook(project_root),

        _ => {
            // 未知 hook，返回默认响应
            Ok(json!({
                "status": "ok",
                "message": format!("Unknown hook: {}", hook_name)
            }))
        }
    }
}

/// 运行 hook（从 stdin 读取输入）
///
/// 这是命令行调用的入口
pub fn run_hook_from_stdin(hook_name: &str, project_root: &Path) -> Result<Value> {
    // 读取 stdin
    let mut stdin_data = String::new();
    io::stdin().read_to_string(&mut stdin_data)?;

    // 解析 JSON（如果有）
    let input = if stdin_data.trim().is_empty() {
        None
    } else {
        Some(serde_json::from_str(&stdin_data)?)
    };

    run_hook(hook_name, project_root, input.as_ref())
}

/// 打印 hook 输出（格式化为 JSON）
pub fn print_hook_output(output: &Value) {
    println!("{}", serde_json::to_string(output).unwrap_or_else(|_| "{}".to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_run_hook_inject_state() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        let result = run_hook("inject_state", temp.path(), None).unwrap();
        assert!(result["hookSpecificOutput"]["additionalContext"].is_string());
    }

    #[test]
    fn test_run_hook_progress_sync() {
        let temp = TempDir::new().unwrap();
        let input = json!({});

        let result = run_hook("progress_sync", temp.path(), Some(&input)).unwrap();
        assert_eq!(result["status"], "ok");
    }

    #[test]
    fn test_run_hook_codex_review() {
        let temp = TempDir::new().unwrap();
        let input = json!({
            "tool_input": {
                "command": "ls"
            }
        });

        let result = run_hook("codex_review_gate", temp.path(), Some(&input)).unwrap();
        assert_eq!(result["decision"], "allow");
    }

    #[test]
    fn test_run_hook_loop_driver() {
        let temp = TempDir::new().unwrap();
        let result = run_hook("loop_driver", temp.path(), None).unwrap();
        assert_eq!(result["decision"], "block");
    }

    #[test]
    fn test_run_hook_unknown() {
        let temp = TempDir::new().unwrap();
        let result = run_hook("unknown_hook", temp.path(), None).unwrap();
        assert_eq!(result["status"], "ok");
    }
}

