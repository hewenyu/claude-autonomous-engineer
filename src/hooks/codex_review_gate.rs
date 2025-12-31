//! Codex Review Gate Hook
//!
//! 提交前自动代码审查（PreToolUse）- 状态感知的差异化审查

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::hooks::codex_executor::execute_codex_review_simple;
use crate::hooks::review_context::ReviewContext;
use crate::hooks::review_parser::Verdict;
use crate::hooks::state_tracker::TaskStateTracker;
use crate::utils::{get_staged_files, read_json};
use crate::Memory;

/// 运行 codex_review_gate hook
///
/// 检测 git commit 命令，根据任务状态转换进行差异化审查
pub fn run_codex_review_gate_hook(project_root: &Path, input: &Value) -> Result<Value> {
    // 提取命令
    let command = extract_command(input);

    // 检查是否是 git commit
    if !is_commit_command(&command) {
        return Ok(json!({
            "decision": "allow"
        }));
    }

    println!("🔍 Codex Review Gate: Analyzing commit...");

    // 获取暂存文件
    let staged_files = match get_staged_files(Some(project_root)) {
        Ok(files) => files,
        Err(_) => {
            println!("   ⚠️  No staged files found, allowing commit");
            return Ok(json!({
                "decision": "allow"
            }));
        }
    };

    if staged_files.is_empty() {
        println!("   ⚠️  No staged files, allowing commit");
        return Ok(json!({
            "decision": "allow"
        }));
    }

    // 加载 memory.json 获取当前任务
    let memory_file = project_root.join(".claude/status/memory.json");
    let memory: Memory = read_json(&memory_file).unwrap_or_default();
    let current_task = &memory.current_task;

    // 如果没有当前任务，使用常规审查
    if current_task.id.is_none() {
        println!("   📝 No current task, skipping review");
        return Ok(json!({
            "decision": "allow",
            "reason": "No active task"
        }));
    }

    // 加载状态追踪器
    let mut state_tracker = TaskStateTracker::load(project_root)?;

    // 检测状态转换
    let is_transition = state_tracker.detect_transition(current_task);

    let review_result = if is_transition {
        // 深度审查模式
        let transition_type = state_tracker.classify_transition(current_task);
        println!("   ⚠️  State Transition Detected: {:?}", transition_type);

        let previous_snapshot = state_tracker
            .get_previous_snapshot(current_task.id.as_ref().unwrap())
            .cloned();

        let context = ReviewContext::build_deep(
            project_root,
            current_task,
            &previous_snapshot,
            &transition_type,
        )?;

        execute_codex_review_simple(&context)
    } else {
        // 常规审查模式
        println!("   📝 Regular Review Mode");

        let context = ReviewContext::build_regular(project_root, current_task)?;

        execute_codex_review_simple(&context)
    };

    // 处理审查结果
    match review_result {
        Ok(result) => {
            match result.verdict {
                Verdict::Pass => {
                    if is_transition && !result.state_transition_valid {
                        // 深度审查时，即使 PASS 也要检查状态转换有效性
                        println!("   ❌ State transition is invalid");
                        return Ok(json!({
                            "decision": "block",
                            "message": result.format_error_message()
                        }));
                    }

                    println!("   ✅ Review PASSED");

                    // 更新状态快照
                    if is_transition {
                        state_tracker.update_snapshot(current_task)?;
                        println!("   💾 State snapshot updated");
                    }

                    Ok(json!({
                        "decision": "allow",
                        "reason": "Code review passed"
                    }))
                }
                Verdict::Warn => {
                    println!("   ⚠️  Review WARNINGS:");
                    for issue in &result.issues {
                        println!("      [WARN] {}", issue.description);
                    }
                    // 警告不阻塞提交
                    Ok(json!({
                        "decision": "allow",
                        "reason": "Code review passed with warnings"
                    }))
                }
                Verdict::Fail => {
                    println!("   ❌ Review FAILED");
                    Ok(json!({
                        "decision": "block",
                        "message": result.format_error_message()
                    }))
                }
            }
        }
        Err(e) => {
            // Codex 命令执行失败，记录错误但允许提交
            eprintln!("   ⚠️  Codex review error: {}", e);
            eprintln!("   ℹ️  Allowing commit (review disabled due to error)");

            Ok(json!({
                "decision": "allow",
                "reason": format!("Review error (allowing commit): {}", e)
            }))
        }
    }
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
