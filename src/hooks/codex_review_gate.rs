//! Codex Review Gate Hook
//!
//! 提交前自动代码审查（PreToolUse）

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::context::ContextManager;
use crate::utils::get_staged_files;

/// 运行 codex_review_gate hook
///
/// 检测 git commit/push 命令，进行代码审查
pub fn run_codex_review_gate_hook(project_root: &Path, input: &Value) -> Result<Value> {
    // 提取命令
    let command = extract_command(input);

    // 检查是否是 git commit/push
    if !is_commit_command(&command) {
        return Ok(json!({
            "decision": "allow"
        }));
    }

    // 获取暂存文件
    let staged_files = match get_staged_files(Some(project_root)) {
        Ok(files) => files,
        Err(_) => {
            return Ok(json!({
                "decision": "allow"
            }))
        }
    };

    if staged_files.is_empty() {
        return Ok(json!({
            "decision": "allow"
        }));
    }

    // 获取审查上下文
    let ctx = ContextManager::new(project_root.to_path_buf());
    let _review_context = ctx.get_review_context(&staged_files)?;

    // TODO: 实际调用 Codex 审查
    // 目前直接放行
    Ok(json!({
        "decision": "allow",
        "reason": format!("[Review] {} files staged for commit", staged_files.len())
    }))
}

/// 从输入中提取命令
fn extract_command(input: &Value) -> String {
    input
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string()
}

/// 检查是否是提交命令
fn is_commit_command(command: &str) -> bool {
    command.contains("git commit") || command.contains("git push")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_codex_review_gate_non_commit() {
        let temp = TempDir::new().unwrap();
        let input = json!({
            "tool_input": {
                "command": "ls -la"
            }
        });

        let result = run_codex_review_gate_hook(temp.path(), &input).unwrap();
        assert_eq!(result["decision"], "allow");
    }

    #[test]
    fn test_is_commit_command() {
        assert!(is_commit_command("git commit -m 'test'"));
        assert!(is_commit_command("git push origin main"));
        assert!(!is_commit_command("git status"));
        assert!(!is_commit_command("npm install"));
    }

    #[test]
    fn test_extract_command() {
        let input = json!({
            "tool_input": {
                "command": "git commit -m 'test'"
            }
        });

        assert_eq!(extract_command(&input), "git commit -m 'test'");
    }
}
