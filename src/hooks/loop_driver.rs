//! Loop Driver Hook
//!
//! 智能循环驱动器 - 控制自主循环的继续/停止（Stop）

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::path::Path;

use crate::state::{parse_roadmap, Memory};
use crate::state_machine::{GitStateMachine, StateId};
use crate::utils::{read_json, try_read_file, write_json};

/// 最大重试次数
const DEFAULT_MAX_RETRIES: u32 = 5;
const MAX_CONSECUTIVE_ERRORS: usize = 10;
const TEST_ACTIVITY_WINDOW_MINUTES: i64 = 5;
const MAX_CONSECUTIVE_TEST_FAILURES: u64 = 12;
const MAX_REPEAT_TEST_FAILURES: u64 = 6;
const TEST_FAILURE_WINDOW: usize = 12;

/// 运行 loop_driver hook
///
/// 检查 ROADMAP 完成状态，决定是否继续循环
/// 同时执行自动状态转换
pub fn run_loop_driver_hook(project_root: &Path) -> Result<Value> {
    let roadmap = check_roadmap(project_root)?;
    let stuck = check_stuck(project_root)?;

    // 自动状态转换（在检查之前尝试）
    let _ = auto_transition_state(project_root, &roadmap, &stuck);

    // Best-effort: keep session counters and blocked state updated (do not fail the hook).
    let _ = update_memory_for_loop(project_root, &roadmap, &stuck);

    // 情况1: ROADMAP 不存在
    if !roadmap.exists {
        return Ok(json!({
            "decision": "block",
            "reason": r#"❌ ROADMAP NOT FOUND

Cannot run autonomous loop without a roadmap.

Action Required:
1. Use project-architect-supervisor to create:
   - .claude/status/ROADMAP.md
   - .claude/status/api_contract.yaml
   - .claude/status/memory.json

2. Or create manually following the template.
"#
        }));
    }

    // 情况2: 所有任务完成
    if roadmap.complete {
        // Stop hook: allow stopping by OMITTING "decision".
        return Ok(json!({
            "systemMessage": format!(r#"🎉 ALL TASKS COMPLETED!

Summary:
- Total tasks: {}
- Completed: {}
- Skipped: {}

The autonomous loop has finished successfully.
You may now stop.
"#, roadmap.total, roadmap.completed, roadmap.skipped)
        }));
    }

    // 情况3: 只剩阻塞任务（没有 pending/in_progress）→ 必须人工处理，不要继续循环
    if roadmap.blocked > 0 && roadmap.pending == 0 && roadmap.in_progress == 0 {
        return Ok(json!({
            "decision": "block",
            "reason": format!(r#"🚫 BLOCKED TASKS REMAIN

There are blocked tasks in ROADMAP, and no pending/in-progress tasks to continue.

Blocked: {}

Actions:
1. Resolve blockers and change [!] → [>] / [ ] for the task(s)
2. Or explicitly skip: change [!] → [-] (only if acceptable)

The autonomous loop cannot complete while [!] tasks remain.
"#, roadmap.blocked)
        }));
    }

    // 情况4: 系统卡住
    if stuck.stuck {
        return Ok(json!({
            "decision": "block",
            "reason": format!(r#"⚠️ STUCK STATE DETECTED

Reason: {}
Suggestion: {}

Options:
1. Try a COMPLETELY DIFFERENT approach
2. Block current task: Mark as [!] in ROADMAP
3. Skip current task: Mark as [-] in ROADMAP
4. Request human intervention

DO NOT simply retry the same approach.
"#, stuck.reason, stuck.suggestion)
        }));
    }

    // 情况5: 正常继续
    let progress_pct = if roadmap.total > 0 {
        ((roadmap.completed + roadmap.skipped) as f64 / roadmap.total as f64) * 100.0
    } else {
        0.0
    };

    Ok(json!({
        "decision": "block",
        "reason": format!(r#"🔄 AUTONOMOUS MODE ACTIVE

Progress: {}/{} ({:.1}%)
├── Completed: {}
├── Skipped: {}
├── In Progress: {}
├── Pending: {}
└── Blocked: {}

Next Task: {}

Continue the loop. DO NOT STOP.
"#,
            roadmap.completed + roadmap.skipped, roadmap.total, progress_pct,
            roadmap.completed, roadmap.skipped, roadmap.in_progress, roadmap.pending, roadmap.blocked,
            roadmap.next_task.chars().take(80).collect::<String>()
        )
    }))
}

// ═══════════════════════════════════════════════════════════════════
// 检查函数
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug)]
struct RoadmapStatus {
    exists: bool,
    complete: bool,
    pending: usize,
    in_progress: usize,
    completed: usize,
    blocked: usize,
    skipped: usize,
    total: usize,
    next_task: String,
}

#[derive(Debug)]
struct StuckStatus {
    stuck: bool,
    reason: String,
    suggestion: String,
}

/// 检查 ROADMAP 状态
fn check_roadmap(project_root: &Path) -> Result<RoadmapStatus> {
    let roadmap_file = project_root.join(".claude/status/ROADMAP.md");

    let content = match try_read_file(&roadmap_file) {
        Some(c) => c,
        None => {
            return Ok(RoadmapStatus {
                exists: false,
                complete: false,
                pending: 0,
                in_progress: 0,
                completed: 0,
                blocked: 0,
                skipped: 0,
                total: 0,
                next_task: String::new(),
            })
        }
    };

    let data = parse_roadmap(&content)?;

    let next_task = if let Some(task) = data.find_current_task() {
        task.line.clone()
    } else if !data.blocked.is_empty() {
        data.blocked[0].line.clone()
    } else {
        "Check ROADMAP".to_string()
    };

    Ok(RoadmapStatus {
        exists: true,
        complete: data.is_complete(),
        pending: data.pending.len(),
        in_progress: data.in_progress.len(),
        completed: data.completed.len(),
        blocked: data.blocked.len(),
        skipped: data.skipped.len(),
        total: data.total,
        next_task,
    })
}

/// 检查是否卡住
fn check_stuck(project_root: &Path) -> Result<StuckStatus> {
    let memory_file = project_root.join(".claude/status/memory.json");
    let memory: Memory = read_json(&memory_file).unwrap_or_default();

    // 检查重试次数
    let task_id = memory.current_task.id.as_deref().unwrap_or("unknown");
    let retry_count = memory.current_task.retry_count;
    let max_retries = if memory.current_task.max_retries == 0 {
        DEFAULT_MAX_RETRIES
    } else {
        memory.current_task.max_retries
    };

    if retry_count >= max_retries {
        return Ok(StuckStatus {
            stuck: true,
            reason: format!("Task {} exceeded {} retries", task_id, max_retries),
            suggestion: "Try different approach or skip task".to_string(),
        });
    }

    // Detect repeated test failures (avoid infinite loops during TDD).
    let consecutive_test_failures = memory
        .session
        .extra
        .get("consecutive_test_failures")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let repeat_test_failures = memory
        .session
        .extra
        .get("repeat_test_failure_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if repeat_test_failures >= MAX_REPEAT_TEST_FAILURES {
        return Ok(StuckStatus {
            stuck: true,
            reason: format!(
                "Task {} hit {} repeated identical test failures",
                task_id, repeat_test_failures
            ),
            suggestion: "Stop rerunning the same tests; inspect the failing test output and change approach".to_string(),
        });
    }

    if consecutive_test_failures >= MAX_CONSECUTIVE_TEST_FAILURES {
        return Ok(StuckStatus {
            stuck: true,
            reason: format!(
                "Task {} has {} consecutive test failures",
                task_id, consecutive_test_failures
            ),
            suggestion: "Isolate one failing test, fix root cause, or mark task blocked to avoid infinite loop".to_string(),
        });
    }

    // 检查错误历史
    let error_file = project_root.join(".claude/status/error_history.json");
    let errors: Vec<Value> = read_json(&error_file).unwrap_or_default();

    if !errors.is_empty() {
        // Additional protection: if the last N errors are unresolved test failures for this task,
        // assume we're looping without making progress.
        let recent_test_failures_for_task = errors
            .iter()
            .rev()
            .take(TEST_FAILURE_WINDOW)
            .filter(|e| {
                let kind = e
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("command_failure");
                if kind != "test_failure" {
                    return false;
                }
                let unresolved = e.get("resolution").is_none() || e["resolution"].is_null();
                if !unresolved {
                    return false;
                }
                e.get("task")
                    .and_then(|t| t.as_str())
                    .map(|t| t == task_id)
                    .unwrap_or(false)
            })
            .count();

        if recent_test_failures_for_task >= TEST_FAILURE_WINDOW {
            return Ok(StuckStatus {
                stuck: true,
                reason: format!(
                    "Task {} has {} recent unresolved test failures",
                    task_id, recent_test_failures_for_task
                ),
                suggestion:
                    "Tests keep failing repeatedly; change strategy or mark task [!] to stop the loop"
                        .to_string(),
            });
        }

        let task_errors: Vec<_> = errors
            .iter()
            .filter(|e| {
                let kind = e
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("command_failure");
                if kind == "test_failure" {
                    return false;
                }
                e.get("task")
                    .and_then(|t| t.as_str())
                    .map(|t| t == task_id)
                    .unwrap_or(false)
                    && (e.get("resolution").is_none() || e["resolution"].is_null())
            })
            .collect();

        if task_errors.len() >= 3 {
            return Ok(StuckStatus {
                stuck: true,
                reason: format!(
                    "Task {} has {} unresolved errors",
                    task_id,
                    task_errors.len()
                ),
                suggestion: "Review error patterns, try alternative".to_string(),
            });
        }

        // 检查连续错误
        let recent_unresolved: Vec<_> = errors
            .iter()
            .rev()
            .take(MAX_CONSECUTIVE_ERRORS)
            .filter(|e| {
                let kind = e
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("command_failure");
                if kind == "test_failure" {
                    return false;
                }
                e.get("resolution").is_none() || e["resolution"].is_null()
            })
            .collect();

        if recent_unresolved.len() >= MAX_CONSECUTIVE_ERRORS {
            return Ok(StuckStatus {
                stuck: true,
                reason: format!("{} consecutive errors", recent_unresolved.len()),
                suggestion: "System may need intervention".to_string(),
            });
        }
    }

    Ok(StuckStatus {
        stuck: false,
        reason: String::new(),
        suggestion: String::new(),
    })
}

// ═══════════════════════════════════════════════════════════════════
// 自动状态转换
// ═══════════════════════════════════════════════════════════════════

/// 自动状态转换逻辑
///
/// 根据 ROADMAP 和任务状态自动转换状态机状态
fn auto_transition_state(
    project_root: &Path,
    roadmap: &RoadmapStatus,
    stuck: &StuckStatus,
) -> Result<()> {
    // 默认关闭：只有在用户显式创建过 `.claude/status/state.json` 后才启用自动状态转换，
    // 避免在未授权情况下污染用户仓库历史（state commits/tags）。
    let state_file = project_root.join(".claude/status/state.json");
    if !state_file.exists() {
        return Ok(());
    }

    // 如果不是 git 仓库，跳过
    let state_machine = match GitStateMachine::new(project_root) {
        Ok(sm) => sm,
        Err(_) => return Ok(()), // 非 git 项目，跳过状态转换
    };

    let current_state = state_machine.current_state()?;

    // 加载 memory.json 获取任务信息
    let memory_file = project_root.join(".claude/status/memory.json");
    let memory: Memory = read_json(&memory_file).unwrap_or_default();
    let task_id = memory.current_task.id.clone();

    // 检测场景并执行相应的状态转换

    // 场景 1: ROADMAP 完成 → Completed
    if roadmap.complete && current_state.state_id != StateId::Completed {
        eprintln!("🎉 All tasks completed - transitioning to COMPLETED state");
        let _ = state_machine.transition_to(
            StateId::Completed,
            task_id.as_deref(),
            Some(serde_json::json!({
                "completed_tasks": roadmap.completed,
                "total_tasks": roadmap.total
            })),
        );
        return Ok(());
    }

    // 场景 2: 只剩阻塞任务 → Blocked
    if roadmap.blocked > 0
        && roadmap.pending == 0
        && roadmap.in_progress == 0
        && current_state.state_id != StateId::Blocked
    {
        eprintln!("🚫 Blocked tasks remain - transitioning to BLOCKED state");
        let _ = state_machine.transition_to(
            StateId::Blocked,
            task_id.as_deref(),
            Some(serde_json::json!({
                "blocked_tasks": roadmap.blocked,
                "total_tasks": roadmap.total
            })),
        );
        return Ok(());
    }

    // 场景 3: 系统卡住 → Blocked
    if stuck.stuck && current_state.state_id != StateId::Blocked {
        eprintln!("🚫 System stuck - transitioning to BLOCKED state");
        let _ = state_machine.transition_to(
            StateId::Blocked,
            task_id.as_deref(),
            Some(serde_json::json!({
                "reason": &stuck.reason,
                "suggestion": &stuck.suggestion
            })),
        );
        return Ok(());
    }

    // 场景 4: 有任务进行中 + 当前状态是 Idle → Coding
    if roadmap.in_progress > 0 && task_id.is_some() && current_state.state_id == StateId::Idle {
        eprintln!("💻 Task started - transitioning to CODING state");
        let _ = state_machine.transition_to(StateId::Coding, task_id.as_deref(), None);
        return Ok(());
    }

    // 场景 5: 检测测试执行（优先用 error_history.command，其次用 memory.session.last_test_at）
    let error_file = project_root.join(".claude/status/error_history.json");
    let errors: Vec<Value> = read_json(&error_file).unwrap_or_default();

    let recent_test_activity = errors
        .iter()
        .rev()
        .take(10)
        .filter_map(|e| e.get("command").and_then(|c| c.as_str()))
        .any(is_test_command);

    let recent_test_activity =
        recent_test_activity || has_recent_test_activity_from_memory(&memory);

    if recent_test_activity && current_state.state_id == StateId::Coding {
        eprintln!("🧪 Test execution detected - transitioning to TESTING state");
        let _ = state_machine.transition_to(StateId::Testing, task_id.as_deref(), None);
        return Ok(());
    }

    // 场景 5: 测试失败（有未解决的测试错误）但不卡住 → 保持 Testing
    // （这里不做状态转换，让 codex_review_gate 处理回滚）

    Ok(())
}

fn update_memory_for_loop(
    project_root: &Path,
    roadmap: &RoadmapStatus,
    stuck: &StuckStatus,
) -> Result<()> {
    let memory_file = project_root.join(".claude/status/memory.json");
    let mut memory: Memory = read_json(&memory_file).unwrap_or_default();

    memory.session.loop_count = memory.session.loop_count.saturating_add(1);
    memory.last_updated = Some(Utc::now().to_rfc3339());

    if stuck.stuck {
        memory.error_state.blocked = true;
        memory.error_state.block_reason = Some(stuck.reason.clone());
    } else if roadmap.blocked > 0 && roadmap.pending == 0 && roadmap.in_progress == 0 {
        memory.error_state.blocked = true;
        memory.error_state.block_reason = Some("Blocked tasks remain in ROADMAP".to_string());
    } else {
        memory.error_state.blocked = false;
        memory.error_state.block_reason = None;
    }

    write_json(&memory_file, &memory)?;
    Ok(())
}

fn is_test_command(command: &str) -> bool {
    let cmd = command.to_lowercase();
    cmd.contains("pytest")
        || cmd.contains("cargo test")
        || cmd.contains("go test")
        || cmd.contains("npm test")
        || cmd.contains("pnpm test")
        || cmd.contains("yarn test")
}

fn has_recent_test_activity_from_memory(memory: &Memory) -> bool {
    let Some(last_test_at) = memory
        .session
        .extra
        .get("last_test_at")
        .and_then(|v| v.as_str())
    else {
        return false;
    };

    let Ok(dt) = DateTime::parse_from_rfc3339(last_test_at) else {
        return false;
    };

    let dt = dt.with_timezone(&Utc);
    let age = Utc::now().signed_duration_since(dt);
    age <= Duration::minutes(TEST_ACTIVITY_WINDOW_MINUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::write_json;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_loop_driver_no_roadmap() {
        let temp = TempDir::new().unwrap();
        let result = run_loop_driver_hook(temp.path()).unwrap();
        assert_eq!(result["decision"], "block");
        assert!(result["reason"]
            .as_str()
            .unwrap()
            .contains("ROADMAP NOT FOUND"));
    }

    #[test]
    fn test_loop_driver_complete() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        // 创建完成的 ROADMAP
        let roadmap = r#"
# Roadmap
- [x] TASK-001: Done
- [x] TASK-002: Also done
"#;
        fs::write(temp.path().join(".claude/status/ROADMAP.md"), roadmap).unwrap();

        let result = run_loop_driver_hook(temp.path()).unwrap();
        assert!(result.get("decision").is_none());
        assert!(result["systemMessage"]
            .as_str()
            .unwrap()
            .contains("ALL TASKS COMPLETED"));
    }

    #[test]
    fn test_loop_driver_in_progress() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        // 创建进行中的 ROADMAP
        let roadmap = r#"
# Roadmap
- [x] TASK-001: Done
- [ ] TASK-002: Pending
- [ ] TASK-003: Also pending
"#;
        fs::write(temp.path().join(".claude/status/ROADMAP.md"), roadmap).unwrap();

        // 创建 memory.json
        let memory = Memory::default();
        write_json(&temp.path().join(".claude/status/memory.json"), &memory).unwrap();

        let result = run_loop_driver_hook(temp.path()).unwrap();
        assert_eq!(result["decision"], "block");
        assert!(result["reason"]
            .as_str()
            .unwrap()
            .contains("AUTONOMOUS MODE ACTIVE"));
    }

    #[test]
    fn test_is_test_command() {
        assert!(is_test_command("cargo test -q"));
        assert!(is_test_command("pytest -q"));
        assert!(!is_test_command("cargo build"));
    }

    #[test]
    fn test_loop_driver_stuck_on_consecutive_test_failures() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        let roadmap = r#"
# Roadmap
- [>] TASK-001: In progress
- [ ] TASK-002: Pending
"#;
        fs::write(temp.path().join(".claude/status/ROADMAP.md"), roadmap).unwrap();

        fs::write(
            temp.path().join(".claude/status/memory.json"),
            r#"
{
  "current_task": { "id": "TASK-001", "status": "IN_PROGRESS", "retry_count": 0, "max_retries": 5 },
  "session": { "consecutive_test_failures": 12 }
}
"#,
        )
        .unwrap();

        let result = run_loop_driver_hook(temp.path()).unwrap();
        assert_eq!(result["decision"], "block");
        assert!(result["reason"]
            .as_str()
            .unwrap()
            .contains("STUCK STATE DETECTED"));
    }
}
