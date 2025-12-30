//! Loop Driver Hook
//!
//! 智能循环驱动器 - 控制自主循环的继续/停止（Stop）

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::state::{parse_roadmap, Memory};
use crate::utils::{read_json, try_read_file};

/// 最大重试次数
const MAX_RETRIES: u32 = 5;
const MAX_CONSECUTIVE_ERRORS: usize = 10;

/// 运行 loop_driver hook
///
/// 检查 ROADMAP 完成状态，决定是否继续循环
pub fn run_loop_driver_hook(project_root: &Path) -> Result<Value> {
    let roadmap = check_roadmap(project_root)?;
    let stuck = check_stuck(project_root)?;

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
        return Ok(json!({
            "decision": "allow",
            "reason": format!(r#"🎉 ALL TASKS COMPLETED!

Summary:
- Total tasks: {}
- Completed: {}

The autonomous loop has finished successfully.
You may now stop.
"#, roadmap.total, roadmap.completed)
        }));
    }

    // 情况3: 系统卡住
    if stuck.stuck {
        return Ok(json!({
            "decision": "block",
            "reason": format!(r#"⚠️ STUCK STATE DETECTED

Reason: {}
Suggestion: {}

Options:
1. Try a COMPLETELY DIFFERENT approach
2. Skip current task: Mark as [!] in ROADMAP
3. Request human intervention

DO NOT simply retry the same approach.
"#, stuck.reason, stuck.suggestion)
        }));
    }

    // 情况4: 正常继续
    let progress_pct = if roadmap.total > 0 {
        (roadmap.completed as f64 / roadmap.total as f64) * 100.0
    } else {
        0.0
    };

    Ok(json!({
        "decision": "block",
        "reason": format!(r#"🔄 AUTONOMOUS MODE ACTIVE

Progress: {}/{} ({:.1}%)
├── Completed: {}
├── In Progress: {}
└── Pending: {}

Next Task: {}

Continue the loop. DO NOT STOP.
"#,
            roadmap.completed, roadmap.total, progress_pct,
            roadmap.completed, roadmap.in_progress, roadmap.pending,
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
                total: 0,
                next_task: String::new(),
            })
        }
    };

    let data = parse_roadmap(&content)?;

    let next_task = if let Some(task) = data.find_current_task() {
        task.line.clone()
    } else {
        "Check ROADMAP".to_string()
    };

    Ok(RoadmapStatus {
        exists: true,
        complete: data.is_complete(),
        pending: data.pending.len(),
        in_progress: data.in_progress.len(),
        completed: data.completed.len(),
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

    if retry_count >= MAX_RETRIES {
        return Ok(StuckStatus {
            stuck: true,
            reason: format!("Task {} exceeded {} retries", task_id, MAX_RETRIES),
            suggestion: "Try different approach or skip task".to_string(),
        });
    }

    // 检查错误历史
    let error_file = project_root.join(".claude/status/error_history.json");
    let errors: Vec<Value> = read_json(&error_file).unwrap_or_default();

    if !errors.is_empty() {
        let task_errors: Vec<_> = errors
            .iter()
            .filter(|e| {
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
                reason: format!("Task {} has {} unresolved errors", task_id, task_errors.len()),
                suggestion: "Review error patterns, try alternative".to_string(),
            });
        }

        // 检查连续错误
        let recent_unresolved: Vec<_> = errors
            .iter()
            .rev()
            .take(MAX_CONSECUTIVE_ERRORS)
            .filter(|e| e.get("resolution").is_none() || e["resolution"].is_null())
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
        assert!(result["reason"].as_str().unwrap().contains("ROADMAP NOT FOUND"));
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
        assert_eq!(result["decision"], "allow");
        assert!(result["reason"]
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
        write_json(
            &temp.path().join(".claude/status/memory.json"),
            &memory,
        )
        .unwrap();

        let result = run_loop_driver_hook(temp.path()).unwrap();
        assert_eq!(result["decision"], "block");
        assert!(result["reason"]
            .as_str()
            .unwrap()
            .contains("AUTONOMOUS MODE ACTIVE"));
    }
}
