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
use crate::hooks::state_tracker::TransitionType;
use crate::utils::{get_staged_files, read_json};
use crate::Memory;

fn noop_pretooluse_output() -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse"
        }
    })
}

fn deny_pretooluse(reason: String) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason
        }
    })
}

/// 运行 codex_review_gate hook
///
/// 检测 git commit 命令，根据任务状态转换进行差异化审查
pub fn run_codex_review_gate_hook(project_root: &Path, input: &Value) -> Result<Value> {
    // 提取命令
    let command = extract_command(input);

    // 检查是否是 git commit
    if !is_commit_command(&command) {
        // 不干预其他 Bash 命令，让 Claude Code 自己走权限流程
        return Ok(noop_pretooluse_output());
    }

    eprintln!("🔍 Codex Review Gate: Analyzing commit...");

    // 获取暂存文件
    let staged_files = match get_staged_files(Some(project_root)) {
        Ok(files) => files,
        Err(_) => {
            eprintln!("   ⚠️  No staged files found, allowing commit");
            return Ok(noop_pretooluse_output());
        }
    };

    if staged_files.is_empty() {
        eprintln!("   ⚠️  No staged files, allowing commit");
        return Ok(noop_pretooluse_output());
    }

    // 加载 memory.json 获取当前任务
    let memory_file = project_root.join(".claude/status/memory.json");
    let memory: Memory = read_json(&memory_file).unwrap_or_default();
    let current_task = &memory.current_task;

    // 如果没有当前任务，使用常规审查
    if current_task.id.is_none() {
        eprintln!("   📝 No current task, skipping review");
        return Ok(noop_pretooluse_output());
    }

    // 加载状态追踪器
    let mut state_tracker = TaskStateTracker::load(project_root)?;

    // 如果这是该任务的首次提交，需要先落一份快照，否则后续永远检测不到转换
    let task_id = current_task.id.as_deref().unwrap_or("");
    let has_snapshot =
        !task_id.is_empty() && state_tracker.get_previous_snapshot(task_id).is_some();

    // 检测状态转换
    let is_transition = state_tracker.detect_transition(current_task);

    // 仅对“关键状态变化”触发深度审查，避免频繁误触发导致长周期自动化不稳定。
    let transition_type = if is_transition {
        Some(state_tracker.classify_transition(current_task))
    } else {
        None
    };

    let requires_deep_review = matches!(
        transition_type,
        Some(TransitionType::CompleteTask | TransitionType::BlockTask)
    );

    let review_result = if requires_deep_review {
        // 深度审查模式（只在关键转换时启用）
        let transition_type = transition_type.as_ref().expect("checked above");
        eprintln!("   ⚠️  Critical State Transition Detected: {:?}", transition_type);

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
        eprintln!("   📝 Regular Review Mode");

        let context = ReviewContext::build_regular(project_root, current_task)?;

        execute_codex_review_simple(&context)
    };

    // 处理审查结果
    match review_result {
        Ok(result) => {
            match result.verdict {
                Verdict::Pass => {
                    if requires_deep_review && !result.state_transition_valid {
                        // 深度审查时，即使 PASS 也要检查状态转换有效性（只有显式 NO 才阻塞）
                        eprintln!("   ❌ State transition is invalid");
                        return Ok(deny_pretooluse(result.format_error_message()));
                    }

                    eprintln!("   ✅ Review PASSED");

                    // 更新状态快照：状态转换时更新；首次看到任务也要初始化一份
                    if is_transition || !has_snapshot {
                        state_tracker.update_snapshot(current_task)?;
                        eprintln!("   💾 State snapshot updated");
                    }

                    Ok(noop_pretooluse_output())
                }
                Verdict::Warn => {
                    eprintln!("   ⚠️  Review WARNINGS:");
                    for issue in &result.issues {
                        eprintln!("      [WARN] {}", issue.description);
                    }
                    // 警告不阻塞提交
                    if !has_snapshot {
                        state_tracker.update_snapshot(current_task)?;
                        eprintln!("   💾 State snapshot updated");
                    }
                    Ok(noop_pretooluse_output())
                }
                Verdict::Fail => {
                    eprintln!("   ❌ Review FAILED");
                    Ok(deny_pretooluse(result.format_error_message()))
                }
            }
        }
        Err(e) => {
            // Codex 命令执行失败，记录错误但允许提交
            eprintln!("   ⚠️  Codex review error: {}", e);
            eprintln!("   ℹ️  Allowing commit (review disabled due to error)");

            Ok(noop_pretooluse_output())
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
    command.contains("git commit")
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
        assert_eq!(result["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert!(result["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none());
    }

    #[test]
    fn test_is_commit_command() {
        assert!(is_commit_command("git commit -m 'test'"));
        assert!(!is_commit_command("git push origin main"));
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
