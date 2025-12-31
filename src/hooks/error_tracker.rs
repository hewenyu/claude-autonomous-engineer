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

const CONSECUTIVE_TEST_FAILURES_KEY: &str = "consecutive_test_failures";
const REPEAT_TEST_FAILURES_KEY: &str = "repeat_test_failure_count";
const LAST_TEST_FAILURE_SIG_KEY: &str = "last_test_failure_signature";

/// Run error_tracker hook
pub fn run_error_tracker_hook(project_root: &Path, input: &Value) -> Result<Value> {
    match extract_outcome(input) {
        ExecutionOutcome::Unknown => Ok(noop_posttooluse_output()),
        ExecutionOutcome::Success { command } => {
            // Best-effort: if a command succeeds, mark matching unresolved errors as resolved.
            let memory_file = project_root.join(".claude/status/memory.json");
            let mut memory: Memory = read_json(&memory_file).unwrap_or_default();
            let Some(task_id) = memory.current_task.id.clone() else {
                return Ok(noop_posttooluse_output());
            };

            let _resolved = resolve_matching_errors(project_root, &task_id, &command)?;

            // Best-effort bookkeeping for long-running sessions
            update_memory_on_command_success(&mut memory, &command);
            let _ = write_json(&memory_file, &memory);

            Ok(noop_posttooluse_output())
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
            }

            update_memory_on_command_failure(&mut memory, &failure);
            let _ = write_json(&memory_file, &memory);

            Ok(noop_posttooluse_output())
        }
    }
}

fn noop_posttooluse_output() -> Value {
    json!({
        "hookSpecificOutput": {
            "for PostToolUse": {
                "hookEventName": "PostToolUse"
            }
        }
    })
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

    if !is_test_command(&cmd) {
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

fn is_test_command(command_lower: &str) -> bool {
    command_lower.contains("pytest")
        || command_lower.contains("cargo test")
        || command_lower.contains("go test")
        || command_lower.contains("npm test")
        || command_lower.contains("pnpm test")
        || command_lower.contains("yarn test")
}

fn extract_text_field(root: &Value, path: &[&str]) -> Option<String> {
    let mut cur = root;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn update_memory_on_command_success(memory: &mut Memory, command: &str) {
    let now = Utc::now().to_rfc3339();
    memory.last_updated = Some(now.clone());

    memory
        .session
        .extra
        .insert("last_command".to_string(), json!(command));
    memory
        .session
        .extra
        .insert("last_command_at".to_string(), json!(now));

    // Track test activity explicitly so other components (e.g. state machine) can infer Testing.
    if is_test_command(&command.to_lowercase()) {
        memory
            .session
            .extra
            .insert("last_test_command".to_string(), json!(command));
        memory
            .session
            .extra
            .insert("last_test_at".to_string(), json!(Utc::now().to_rfc3339()));
        memory
            .session
            .extra
            .insert("last_test_outcome".to_string(), json!("success"));

        // Reset consecutive test failure counters on a successful test run.
        memory
            .session
            .extra
            .insert(CONSECUTIVE_TEST_FAILURES_KEY.to_string(), json!(0));
        memory
            .session
            .extra
            .insert(REPEAT_TEST_FAILURES_KEY.to_string(), json!(0));
        memory.session.extra.remove(LAST_TEST_FAILURE_SIG_KEY);
    }
}

fn update_memory_on_command_failure(memory: &mut Memory, failure: &FailureInfo) {
    let now = Utc::now().to_rfc3339();
    memory.last_updated = Some(now.clone());

    memory.error_state.last_error = Some(failure.message.clone());
    memory.error_state.last_error_at = Some(now);
    memory.error_state.error_count = memory.error_state.error_count.saturating_add(1);

    if let Some(cmd) = &failure.command {
        memory
            .session
            .extra
            .insert("last_command".to_string(), json!(cmd));
        memory.session.extra.insert(
            "last_command_at".to_string(),
            json!(Utc::now().to_rfc3339()),
        );

        if is_test_command(&cmd.to_lowercase()) {
            memory
                .session
                .extra
                .insert("last_test_command".to_string(), json!(cmd));
            memory
                .session
                .extra
                .insert("last_test_at".to_string(), json!(Utc::now().to_rfc3339()));
            memory
                .session
                .extra
                .insert("last_test_outcome".to_string(), json!(failure.kind));

            // Track consecutive/duplicate test failures to avoid infinite loops.
            if failure.kind == "test_failure" {
                let signature = format!(
                    "{}|{}",
                    cmd,
                    failure
                        .message
                        .lines()
                        .next()
                        .unwrap_or("test failure")
                        .trim()
                );
                let signature = truncate_for_log(&signature, 500);

                let consecutive = memory
                    .session
                    .extra
                    .get(CONSECUTIVE_TEST_FAILURES_KEY)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
                    .saturating_add(1);
                memory.session.extra.insert(
                    CONSECUTIVE_TEST_FAILURES_KEY.to_string(),
                    json!(consecutive),
                );

                let last_sig = memory
                    .session
                    .extra
                    .get(LAST_TEST_FAILURE_SIG_KEY)
                    .and_then(|v| v.as_str());
                let repeat = if last_sig == Some(signature.as_str()) {
                    memory
                        .session
                        .extra
                        .get(REPEAT_TEST_FAILURES_KEY)
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0)
                        .saturating_add(1)
                } else {
                    1
                };

                memory
                    .session
                    .extra
                    .insert(REPEAT_TEST_FAILURES_KEY.to_string(), json!(repeat));
                memory
                    .session
                    .extra
                    .insert(LAST_TEST_FAILURE_SIG_KEY.to_string(), json!(signature));
            }
        }
    }
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
        assert_eq!(
            result["hookSpecificOutput"]["for PostToolUse"]["hookEventName"],
            "PostToolUse"
        );

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
        assert_eq!(
            result["hookSpecificOutput"]["for PostToolUse"]["hookEventName"],
            "PostToolUse"
        );

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

    #[test]
    fn test_update_memory_records_test_success() {
        let mut mem = Memory::default();
        mem.session
            .extra
            .insert(CONSECUTIVE_TEST_FAILURES_KEY.to_string(), json!(3));
        mem.session
            .extra
            .insert(REPEAT_TEST_FAILURES_KEY.to_string(), json!(2));
        mem.session
            .extra
            .insert(LAST_TEST_FAILURE_SIG_KEY.to_string(), json!("sig"));
        update_memory_on_command_success(&mut mem, "cargo test -q");
        assert_eq!(
            mem.session
                .extra
                .get("last_test_outcome")
                .and_then(|v| v.as_str()),
            Some("success")
        );
        assert_eq!(
            mem.session
                .extra
                .get(CONSECUTIVE_TEST_FAILURES_KEY)
                .and_then(|v| v.as_u64()),
            Some(0)
        );
        assert_eq!(
            mem.session
                .extra
                .get(REPEAT_TEST_FAILURES_KEY)
                .and_then(|v| v.as_u64()),
            Some(0)
        );
        assert!(!mem.session.extra.contains_key(LAST_TEST_FAILURE_SIG_KEY));
    }

    #[test]
    fn test_run_error_tracker_tracks_consecutive_test_failures_and_resets() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        std::fs::write(
            temp.path().join(".claude/status/memory.json"),
            r#"{ "current_task": { "id": "TASK-001", "status": "IN_PROGRESS", "retry_count": 0, "max_retries": 5 } }"#,
        )
        .unwrap();

        let fail_input = json!({
            "tool_input": { "command": "cargo test -q" },
            "tool_output": { "exit_code": 101, "stderr": "thread 't' panicked at 'assertion failed'" }
        });
        run_error_tracker_hook(temp.path(), &fail_input).unwrap();
        run_error_tracker_hook(temp.path(), &fail_input).unwrap();

        let mem: Memory = read_json(&temp.path().join(".claude/status/memory.json")).unwrap();
        assert_eq!(
            mem.session
                .extra
                .get(CONSECUTIVE_TEST_FAILURES_KEY)
                .and_then(|v| v.as_u64()),
            Some(2)
        );
        assert_eq!(
            mem.session
                .extra
                .get(REPEAT_TEST_FAILURES_KEY)
                .and_then(|v| v.as_u64()),
            Some(2)
        );

        let success_input = json!({
            "tool_input": { "command": "cargo test -q" },
            "tool_output": { "exit_code": 0, "stdout": "ok" }
        });
        run_error_tracker_hook(temp.path(), &success_input).unwrap();

        let mem: Memory = read_json(&temp.path().join(".claude/status/memory.json")).unwrap();
        assert_eq!(
            mem.session
                .extra
                .get(CONSECUTIVE_TEST_FAILURES_KEY)
                .and_then(|v| v.as_u64()),
            Some(0)
        );
    }
}
