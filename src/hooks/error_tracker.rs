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
    // Only act on clear failures; otherwise no-op.
    let Some(failure) = extract_failure(input) else {
        return Ok(json!({
            "status": "ok",
            "action": "none"
        }));
    };

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

#[derive(Debug)]
struct FailureInfo {
    kind: &'static str,
    message: String,
    attempted_fix: Option<String>,
    increment_retry: bool,
}

fn extract_failure(input: &Value) -> Option<FailureInfo> {
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
        .or_else(|| {
            input
                .pointer("/tool_result/exit_code")
                .and_then(|v| v.as_i64())
        })
        .or_else(|| input.pointer("/tool_output/code").and_then(|v| v.as_i64()))
        .or_else(|| input.pointer("/tool_result/code").and_then(|v| v.as_i64()))
        .or_else(|| input.get("exit_code").and_then(|v| v.as_i64()));

    let success = input
        .pointer("/tool_output/success")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            input
                .pointer("/tool_result/success")
                .and_then(|v| v.as_bool())
        })
        .or_else(|| exit_code.map(|c| c == 0));

    if success == Some(true) {
        return None;
    }

    // If we can't confidently say it's a failure, do nothing (avoid false positives).
    if success.is_none() && exit_code.is_none() {
        return None;
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

    Some(FailureInfo {
        kind,
        message: truncate_for_log(raw_message, 2000),
        attempted_fix: if command.is_empty() {
            None
        } else {
            Some(format!("command: {}", truncate_for_log(&command, 500)))
        },
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

fn truncate_for_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}…(truncated)", &s[..max.min(s.len())])
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
        assert!(extract_failure(&input).is_none());
    }

    #[test]
    fn test_extract_failure_records_error() {
        let input = json!({
            "tool_input": { "command": "false" },
            "tool_output": { "exit_code": 1, "stderr": "boom" }
        });
        let failure = extract_failure(&input).unwrap();
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
}
