//! Codex Review Gate Hook
//!
//! 提交前自动代码审查（PreToolUse）- 状态感知的差异化审查

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

use crate::hooks::codex_executor::execute_codex_review_simple;
use crate::hooks::review_context::ReviewContext;
use crate::hooks::review_parser::Verdict;
use crate::hooks::state_tracker::TaskStateTracker;
use crate::hooks::state_tracker::TransitionType;
use crate::state::models::ReviewRetryState;
use crate::utils::{get_staged_files, read_json, write_json};
use crate::Memory;

/// 最大审查重试次数
const MAX_REVIEW_RETRIES: u32 = 3;

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

    // 计算当前 staged files 的 hash
    let staged_files_hash = compute_staged_files_hash(project_root)?;

    // 加载 memory.json 获取当前任务
    let memory_file = project_root.join(".claude/status/memory.json");
    let memory: Memory = read_json(&memory_file).unwrap_or_default();
    let current_task = &memory.current_task;

    // 如果没有当前任务，使用常规审查
    if current_task.id.is_none() {
        eprintln!("   📝 No current task, skipping review");
        return Ok(noop_pretooluse_output());
    }

    let task_id = current_task.id.as_deref().unwrap_or("");

    // 加载或初始化重试状态
    let retry_state_file = project_root.join(".claude/status/review_retry_count.json");
    let mut retry_state: ReviewRetryState = read_json(&retry_state_file).unwrap_or_default();

    // 检查是否是同一个任务和相同的代码
    let is_same_attempt =
        retry_state.current_task_id == task_id && retry_state.last_staged_files_hash == staged_files_hash;

    // 加载状态追踪器
    let mut state_tracker = TaskStateTracker::load(project_root)?;

    // 如果这是该任务的首次提交，需要先落一份快照，否则后续永远检测不到转换
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
        eprintln!(
            "   ⚠️  Critical State Transition Detected: {:?}",
            transition_type
        );

        let previous_snapshot = state_tracker
            .get_previous_snapshot(current_task.id.as_ref().unwrap())
            .cloned();

        let context = ReviewContext::build_deep(
            project_root,
            current_task,
            &previous_snapshot,
            transition_type,
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

                    let failure_reason = result.format_error_message();

                    // 更新重试状态
                    if is_same_attempt {
                        // 相同的代码被再次拒绝
                        retry_state.consecutive_failures += 1;
                    } else {
                        // 新的尝试，重置计数
                        retry_state.consecutive_failures = 1;
                        retry_state.current_task_id = task_id.to_string();
                        retry_state.last_staged_files_hash = staged_files_hash;
                        retry_state.failure_reasons.clear();
                    }

                    retry_state.last_failure_timestamp = chrono::Utc::now().to_rfc3339();
                    retry_state.failure_reasons.push(failure_reason.clone());

                    // 保存重试状态
                    let _ = write_json(&retry_state_file, &retry_state);

                    // 检查是否超过重试限制
                    if retry_state.consecutive_failures >= MAX_REVIEW_RETRIES {
                        // 记录到 error_history.json
                        let error_file = project_root.join(".claude/status/error_history.json");
                        let mut errors: Vec<Value> = read_json(&error_file).unwrap_or_default();

                        errors.push(json!({
                            "task": task_id,
                            "kind": "codex_review_failure",
                            "command": "git commit",
                            "error": failure_reason.clone(),
                            "attempted_fix": Value::Null,
                            "resolution": Value::Null,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }));

                        let _ = write_json(&error_file, &errors);

                        // 返回特殊的错误消息
                        Ok(deny_pretooluse(format!(
                            r#"❌ Code Review Failed ({}/{}):

{}

⚠️ RETRY LIMIT EXCEEDED

The same code has been rejected {} times. This suggests a fundamental issue.

Recommended actions:
1. Try a completely different implementation approach
2. Skip review temporarily: export SKIP_CODEX_REVIEW=1 && git commit
3. Mark task as BLOCKED: Edit ROADMAP.md and change [ ] to [!]
4. Review the task requirements in TASK-{}.md

Previous failures:
{}
"#,
                            retry_state.consecutive_failures,
                            MAX_REVIEW_RETRIES,
                            failure_reason,
                            retry_state.consecutive_failures,
                            task_id,
                            retry_state.failure_reasons.join("\n---\n")
                        )))
                    } else {
                        // 正常的失败消息
                        Ok(deny_pretooluse(format!(
                            "❌ Code Review Failed (Attempt {}/{}):\n\n{}\n\n💡 Fix the issues above and try again.",
                            retry_state.consecutive_failures,
                            MAX_REVIEW_RETRIES,
                            failure_reason
                        )))
                    }
                }
            }
        }
        Err(e) => {
            // Fail-closed: if Codex review cannot run, do not allow committing silently.
            eprintln!("   ❌ Codex review error: {}", e);

            Ok(deny_pretooluse(format!(
                r#"❌ Codex review could not be executed, commit blocked.

Error:
{}

Fix:
1) Ensure `codex` is installed and available in PATH
2) Re-run the commit after fixing the review tool

If you intentionally want to bypass the gate, remove/disable the `claude-autonomous hook codex_review_gate`
entry from `.claude/settings.json`."#,
                e
            )))
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

/// 计算 staged files 的 SHA256 hash
///
/// 用于检测代码是否有实质性修改
fn compute_staged_files_hash(project_root: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};

    let output = Command::new("git")
        .arg("diff")
        .arg("--cached")
        .current_dir(project_root)
        .output()?;

    let diff = String::from_utf8_lossy(&output.stdout);
    let mut hasher = Sha256::new();
    hasher.update(diff.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    Ok(hash)
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

    #[cfg(unix)]
    mod commit_tests {
        use super::*;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;
        use std::process::Command;
        use std::sync::{Mutex, OnceLock};

        struct EnvGuard {
            key: &'static str,
            previous: Option<String>,
        }

        impl EnvGuard {
            fn set(key: &'static str, value: String) -> Self {
                let previous = std::env::var(key).ok();
                std::env::set_var(key, value);
                Self { key, previous }
            }
        }

        impl Drop for EnvGuard {
            fn drop(&mut self) {
                if let Some(prev) = self.previous.take() {
                    std::env::set_var(self.key, prev);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }

        fn write_executable(path: &Path, content: &str) {
            fs::write(path, content).unwrap();
            let mut perm = fs::metadata(path).unwrap().permissions();
            perm.set_mode(0o755);
            fs::set_permissions(path, perm).unwrap();
        }

        fn init_git_repo_with_staged_file(root: &Path) {
            let status = Command::new("git")
                .args(["init"])
                .current_dir(root)
                .status()
                .unwrap();
            assert!(status.success());

            fs::write(root.join("file.txt"), "hello\n").unwrap();
            let status = Command::new("git")
                .args(["add", "file.txt"])
                .current_dir(root)
                .status()
                .unwrap();
            assert!(status.success());
        }

        fn write_minimal_memory(root: &Path) {
            fs::create_dir_all(root.join(".claude/status")).unwrap();
            fs::write(
                root.join(".claude/status/memory.json"),
                r#"{ "current_task": { "id": "TASK-001", "status": "IN_PROGRESS", "retry_count": 0, "max_retries": 5 } }"#,
            )
            .unwrap();
        }

        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

        #[test]
        fn test_commit_denied_when_codex_review_errors() {
            let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

            let temp = TempDir::new().unwrap();
            init_git_repo_with_staged_file(temp.path());
            write_minimal_memory(temp.path());

            // Fake `codex` that fails.
            let bin_dir = temp.path().join("bin");
            fs::create_dir_all(&bin_dir).unwrap();
            let codex_path = bin_dir.join("codex");
            write_executable(&codex_path, "#!/bin/sh\n\necho \"boom\" 1>&2\nexit 1\n");

            let _guard = EnvGuard::set(
                "CLAUDE_AUTONOMOUS_CODEX_BIN",
                codex_path.to_string_lossy().to_string(),
            );

            let input = json!({
                "tool_input": { "command": "git commit -m 'x'" }
            });

            let result = run_codex_review_gate_hook(temp.path(), &input).unwrap();
            assert_eq!(result["hookSpecificOutput"]["permissionDecision"], "deny");
            assert!(result["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .contains("commit blocked"));
        }

        #[test]
        fn test_commit_allowed_when_codex_review_passes() {
            let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

            let temp = TempDir::new().unwrap();
            init_git_repo_with_staged_file(temp.path());
            write_minimal_memory(temp.path());

            // Fake `codex` that returns a PASS verdict.
            let bin_dir = temp.path().join("bin");
            fs::create_dir_all(&bin_dir).unwrap();
            let codex_path = bin_dir.join("codex");
            write_executable(
                &codex_path,
                "#!/bin/sh\n\ncat >/dev/null\n\necho \"VERDICT: PASS\"\necho \"ISSUES:\"\nexit 0\n",
            );

            let _guard = EnvGuard::set(
                "CLAUDE_AUTONOMOUS_CODEX_BIN",
                codex_path.to_string_lossy().to_string(),
            );

            let input = json!({
                "tool_input": { "command": "git commit -m 'x'" }
            });

            let result = run_codex_review_gate_hook(temp.path(), &input).unwrap();
            assert_eq!(result["hookSpecificOutput"]["hookEventName"], "PreToolUse");
            assert!(result["hookSpecificOutput"]
                .get("permissionDecision")
                .is_none());
        }
    }
}
