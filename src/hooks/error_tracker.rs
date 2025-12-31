//! Error Tracker Hook
//!
//! PostToolUse (Bash) hook: record failed commands into error_history.json
//! and increment retry_count in memory.json (best-effort).

use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::path::Path;

use crate::state::Memory;
use crate::utils::{read_json, write_json};

/// Run error_tracker hook
pub fn run_error_tracker_hook(project_root: &Path, input: &Value) -> Result<Value> {
    match extract_outcome(input) {
        ExecutionOutcome::Unknown => Ok(json!({
            "status": "ok",
            "action": "none"
        })),
        ExecutionOutcome::Success { command } => {
            // Best-effort: if a command succeeds, mark matching unresolved errors as resolved.
            let memory_file = project_root.join(".claude/status/memory.json");
            let memory: Memory = read_json(&memory_file).unwrap_or_default();
            let Some(task_id) = memory.current_task.id.clone() else {
                return Ok(json!({
                    "status": "ok",
                    "action": "none"
                }));
            };

            let resolved = resolve_matching_errors(project_root, &task_id, &command)?;
            if resolved > 0 {
                Ok(json!({
                    "status": "ok",
                    "action": "resolved",
                    "resolved": resolved,
                }))
            } else {
                Ok(json!({
                    "status": "ok",
                    "action": "none"
                }))
            }
        }
        ExecutionOutcome::Failure(failure) => {
            // Load memory (optional) to bind errors to the current task.
            let memory_file = project_root.join(".claude/status/memory.json");
            let mut memory: Memory = read_json(&memory_file).unwrap_or_default();

            let task_id = memory
                .current_task
                .id
                .clone()
                .unwrap_or_else(|| "unknown".to_string());

            // Append into error_history.json (create if missing).
            let error_file = project_root.join(".claude/status/error_history.json");
            let mut errors: Vec<Value> = read_json(&error_file).unwrap_or_default();

            errors.push(json!({
                "task": task_id,
                "kind": failure.kind,
                "command": failure.command,
                "error": failure.message,
                "attempted_fix": failure.attempted_fix,
                "resolution": Value::Null,
                "timestamp": Utc::now().to_rfc3339(),
            }));

            write_json(&error_file, &errors)?;

            // Increment retry counter only for "command_failure" (not expected test failures).
            if memory.current_task.id.is_some() && failure.increment_retry {
                memory.current_task.retry_count = memory.current_task.retry_count.saturating_add(1);
                memory.current_task.last_updated = Some(Utc::now().to_rfc3339());
                let _ = write_json(&memory_file, &memory);
            }

            Ok(json!({
                "status": "ok",
                "action": "recorded",
                "kind": failure.kind,
                "incremented_retry": failure.increment_retry,
            }))
        }
    }
}

#[derive(Debug)]
struct FailureInfo {
    kind: &'static str,
    command: Option<String>,
    message: String,
    attempted_fix: Option<String>,
    increment_retry: bool,
}

#[derive(Debug)]
enum ExecutionOutcome {
    Success { command: String },
    Failure(FailureInfo),
    Unknown,
}

fn extract_outcome(input: &Value) -> ExecutionOutcome {
    let command = input
        .get("tool_input")
        .and_then(|t| t.get("command"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    // Determine success/failure.
    let exit_code = input
        .pointer("/tool_output/exit_code")
        .and_then(|v| v.as_i64())
        .or_else(|| input.pointer("/tool_result/exit_code").and_then(|v| v.as_i64()))
        .or_else(|| input.pointer("/tool_output/code").and_then(|v| v.as_i64()))
        .or_else(|| input.pointer("/tool_result/code").and_then(|v| v.as_i64()))
        .or_else(|| input.get("exit_code").and_then(|v| v.as_i64()));

    let success = input
        .pointer("/tool_output/success")
        .and_then(|v| v.as_bool())
        .or_else(|| input.pointer("/tool_result/success").and_then(|v| v.as_bool()))
        .or_else(|| exit_code.map(|c| c == 0));

    if success == Some(true) {
        return ExecutionOutcome::Success { command };
    }

    // If we can't confidently say it's a failure, do nothing (avoid false positives).
    if success.is_none() && exit_code.is_none() {
        return ExecutionOutcome::Unknown;
    }

    let stderr = extract_text_field(input, &["tool_output", "stderr"])
        .or_else(|| extract_text_field(input, &["tool_result", "stderr"]))
        .or_else(|| extract_text_field(input, &["tool_output", "error"]))
        .or_else(|| extract_text_field(input, &["tool_result", "error"]))
        .unwrap_or_default();

    let stdout = extract_text_field(input, &["tool_output", "stdout"])
        .or_else(|| extract_text_field(input, &["tool_result", "stdout"]))
        .or_else(|| extract_text_field(input, &["tool_output", "output"]))
        .or_else(|| extract_text_field(input, &["tool_result", "output"]))
        .unwrap_or_default();

    let raw_message = [stderr.trim(), stdout.trim()]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("Command failed");

    let kind = classify_failure_kind(&command, raw_message);
    let increment_retry = kind == "command_failure";
    let command_truncated = if command.trim().is_empty() {
        None
    } else {
        Some(truncate_for_log(command.trim(), 500))
    };

    ExecutionOutcome::Failure(FailureInfo {
        kind,
        command: command_truncated.clone(),
        message: truncate_for_log(raw_message, 2000),
        attempted_fix: command_truncated.map(|c| format!("command: {}", c)),
        increment_retry,
    })
}

fn classify_failure_kind(command: &str, message: &str) -> &'static str {
    let cmd = command.to_lowercase();
    let msg = message;

    let is_test_cmd = cmd.contains("pytest")
        || cmd.contains("cargo test")
        || cmd.contains("go test")
        || cmd.contains("npm test")
        || cmd.contains("pnpm test")
        || cmd.contains("yarn test");

    if !is_test_cmd {
        return "command_failure";
    }

    // Heuristics: treat compilation/import/runtime errors as "command_failure",
    // otherwise treat as "test_failure" (expected during TDD).
    let looks_like_compile = msg.contains("could not compile")
        || msg.contains("error[E")
        || msg.contains("error:")
        || msg.contains("Compilation failed");

    let looks_like_runtime = msg.contains("Traceback (most recent call last)")
        || msg.contains("ModuleNotFoundError")
        || msg.contains("ImportError")
        || msg.contains("SyntaxError");

    if looks_like_compile || looks_like_runtime {
        "command_failure"
    } else {
        "test_failure"
    }
}

fn extract_text_field(root: &Value, path: &[&str]) -> Option<String> {
    let mut cur = root;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn clamp_to_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let cut = clamp_to_char_boundary(s, max);
    format!("{}…(truncated)", &s[..cut])
}

fn resolve_matching_errors(project_root: &Path, task_id: &str, command: &str) -> Result<usize> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Ok(0);
    }

    let cmd_key = truncate_for_log(cmd, 500);

    let error_file = project_root.join(".claude/status/error_history.json");
    let mut errors: Vec<Value> = read_json(&error_file).unwrap_or_default();

    let mut resolved = 0usize;
    for err in errors.iter_mut().rev() {
        let unresolved = err.get("resolution").is_none() || err["resolution"].is_null();
        if !unresolved {
            continue;
        }

        let err_task = err.get("task").and_then(|t| t.as_str()).unwrap_or("");
        if err_task != task_id {
            continue;
        }

        let matches_command = err
            .get("command")
            .and_then(|c| c.as_str())
            .map(|c| c == cmd_key)
            .unwrap_or(false)
            || err
                .get("attempted_fix")
                .and_then(|a| a.as_str())
                .map(|a| a.contains(&cmd_key))
                .unwrap_or(false);

        if !matches_command {
            continue;
        }

        err["resolution"] = json!({
            "message": format!("command succeeded: {}", cmd_key),
            "timestamp": Utc::now().to_rfc3339(),
        });
        resolved += 1;
    }

    if resolved > 0 {
        write_json(&error_file, &errors)?;
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_failure_success_noop() {
        let input = json!({
            "tool_input": { "command": "echo ok" },
            "tool_output": { "exit_code": 0, "stdout": "ok" }
        });
        assert!(matches!(
            extract_outcome(&input),
            ExecutionOutcome::Success { .. }
        ));
    }

    #[test]
    fn test_extract_failure_records_error() {
        let input = json!({
            "tool_input": { "command": "false" },
            "tool_output": { "exit_code": 1, "stderr": "boom" }
        });
        let ExecutionOutcome::Failure(failure) = extract_outcome(&input) else {
            panic!("expected Failure outcome");
        };
        assert_eq!(failure.kind, "command_failure");
        assert!(failure.increment_retry);
    }

    #[test]
    fn test_run_error_tracker_creates_error_history() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        // Minimal memory.json for retry counting
        std::fs::write(
            temp.path().join(".claude/status/memory.json"),
            r#"{ "current_task": { "id": "TASK-001", "status": "IN_PROGRESS", "retry_count": 0, "max_retries": 5 } }"#,
        )
        .unwrap();

        let input = json!({
            "tool_input": { "command": "false" },
            "tool_output": { "exit_code": 1, "stderr": "boom" }
        });

        let result = run_error_tracker_hook(temp.path(), &input).unwrap();
        assert_eq!(result["action"], "recorded");

        let errors: Vec<Value> =
            read_json(&temp.path().join(".claude/status/error_history.json")).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["task"], "TASK-001");
    }

    #[test]
    fn test_run_error_tracker_resolves_matching_error_on_success() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        std::fs::write(
            temp.path().join(".claude/status/memory.json"),
            r#"{ "current_task": { "id": "TASK-001", "status": "IN_PROGRESS", "retry_count": 0, "max_retries": 5 } }"#,
        )
        .unwrap();

        let fail_input = json!({
            "tool_input": { "command": "cargo build" },
            "tool_output": { "exit_code": 101, "stderr": "error: could not compile" }
        });
        run_error_tracker_hook(temp.path(), &fail_input).unwrap();

        let success_input = json!({
            "tool_input": { "command": "cargo build" },
            "tool_output": { "exit_code": 0, "stdout": "Finished" }
        });
        let result = run_error_tracker_hook(temp.path(), &success_input).unwrap();
        assert_eq!(result["action"], "resolved");

        let errors: Vec<Value> =
            read_json(&temp.path().join(".claude/status/error_history.json")).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0]["resolution"].is_object());
    }

    #[test]
    fn test_truncate_for_log_utf8_safe() {
        let s = "中文🙂".repeat(100);
        let t = truncate_for_log(&s, 10);
        assert!(t.contains("truncated"));
    }
}
