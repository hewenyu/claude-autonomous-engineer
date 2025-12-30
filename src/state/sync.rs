//! 状态同步逻辑
//!
//! 从 ROADMAP.md 等文件同步到 memory.json

use anyhow::Result;
use chrono::Utc;
use std::path::Path;

use super::{parse_phase_plan, parse_roadmap, parse_task_file, Memory};
use crate::utils::{append_file, read_json, try_read_file, write_json};

// ═══════════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════════

/// 记录决策日志
fn log_decision(project_root: &Path, message: &str) -> Result<()> {
    let log_file = project_root.join(".claude/status/decisions.log");
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] {}\n", timestamp, message);
    append_file(&log_file, &entry)
}

// ═══════════════════════════════════════════════════════════════════
// 从 ROADMAP.md 同步
// ═══════════════════════════════════════════════════════════════════

/// 从 ROADMAP.md 同步进度到 memory.json
pub fn sync_from_roadmap(project_root: &Path, roadmap_path: &Path) -> Result<bool> {
    // 读取 ROADMAP 内容
    let content = match try_read_file(roadmap_path) {
        Some(c) => c,
        None => return Ok(false),
    };

    // 解析 ROADMAP
    let roadmap_data = parse_roadmap(&content)?;

    // 读取 memory.json
    let memory_path = project_root.join(".claude/status/memory.json");
    let mut memory: Memory = read_json(&memory_path).unwrap_or_default();

    // 更新进度
    memory.progress.tasks_completed = roadmap_data.completed.len();
    memory.progress.tasks_total = roadmap_data.total;
    memory.progress.tasks_pending = roadmap_data.pending.len();
    memory.progress.tasks_in_progress = roadmap_data.in_progress.len();
    memory.progress.current_phase = roadmap_data.current_phase.clone();
    memory.progress.last_synced = Some(Utc::now().to_rfc3339());

    // 确定当前任务
    if let Some(current) = roadmap_data.find_current_task() {
        if let Some(task_id) = &current.id {
            // 检查是否是新任务
            if memory.current_task.id.as_ref() != Some(task_id) {
                // 任务变更，更新 current_task
                let status = if roadmap_data
                    .in_progress
                    .iter()
                    .any(|t| t.id.as_ref() == Some(task_id))
                {
                    "IN_PROGRESS"
                } else {
                    "PENDING"
                };

                memory.current_task.id = Some(task_id.clone());
                memory.current_task.name = Some(
                    current
                        .line
                        .replace("- [ ]", "")
                        .replace("- [>]", "")
                        .trim()
                        .chars()
                        .take(100)
                        .collect(),
                );
                memory.current_task.status = status.to_string();
                memory.current_task.started_at = if status == "IN_PROGRESS" {
                    Some(Utc::now().to_rfc3339())
                } else {
                    None
                };
                memory.current_task.retry_count = 0;

                log_decision(
                    project_root,
                    &format!("SYNC: Current task updated to {}", task_id),
                )?;
            }
        }
    }

    // 如果所有任务完成
    if roadmap_data.is_complete() {
        memory.current_task.id = None;
        memory.current_task.status = "ALL_COMPLETED".to_string();
        memory.next_action.action = "FINALIZE".to_string();
        memory.next_action.target = Some("Generate completion report".to_string());
        memory.next_action.reason = Some("All tasks in ROADMAP completed".to_string());

        log_decision(project_root, "SYNC: All tasks completed!")?;
    }

    // 写回 memory.json
    write_json(&memory_path, &memory)?;

    Ok(true)
}

// ═══════════════════════════════════════════════════════════════════
// 从 TASK-xxx.md 同步
// ═══════════════════════════════════════════════════════════════════

/// 从任务文件同步状态
pub fn sync_from_task_file(project_root: &Path, task_path: &Path) -> Result<bool> {
    // 从文件名提取任务 ID
    let filename = task_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let task_id_pattern = regex::Regex::new(r"(TASK-\d+)").unwrap();
    let task_id = match task_id_pattern.captures(filename) {
        Some(caps) => caps.get(1).map(|m| m.as_str().to_string()),
        None => return Ok(false),
    };

    let task_id = match task_id {
        Some(id) => id,
        None => return Ok(false),
    };

    // 读取任务文件
    let content = match try_read_file(task_path) {
        Some(c) => c,
        None => return Ok(false),
    };

    // 解析任务详情
    let task_data = parse_task_file(&content, &task_id)?;

    // 读取 memory.json
    let memory_path = project_root.join(".claude/status/memory.json");
    let mut memory: Memory = read_json(&memory_path).unwrap_or_default();

    // 检查是否是当前任务
    if memory.current_task.id.as_ref() == Some(&task_id) {
        // 更新当前任务状态
        memory.current_task.name = Some(task_data.name.clone());
        memory.current_task.status = task_data.status.clone();
        memory.current_task.phase = task_data.phase.clone();
        memory.current_task.last_updated = Some(Utc::now().to_rfc3339());

        // 检查验收标准完成情况
        let criteria = &task_data.acceptance_criteria;
        let completed = criteria
            .iter()
            .filter(|c| c.to_lowercase().contains("[x]"))
            .count();
        let total = criteria.len();

        if total > 0 {
            memory.current_task.acceptance_progress = Some(format!("{}/{}", completed, total));

            if completed == total {
                log_decision(
                    project_root,
                    &format!("SYNC: Task {} all acceptance criteria met!", task_id),
                )?;
            }
        }

        // 写回 memory.json
        write_json(&memory_path, &memory)?;
        log_decision(
            project_root,
            &format!("SYNC: Updated task {} from task file", task_id),
        )?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// ═══════════════════════════════════════════════════════════════════
// 从 PHASE_PLAN.md 同步
// ═══════════════════════════════════════════════════════════════════

/// 从阶段计划文件同步状态
pub fn sync_from_phase_plan(project_root: &Path, phase_path: &Path) -> Result<bool> {
    // 读取阶段计划文件
    let content = match try_read_file(phase_path) {
        Some(c) => c,
        None => return Ok(false),
    };

    // 解析阶段信息
    let phase_plan = match parse_phase_plan(&content)? {
        Some(p) => p,
        None => return Ok(false),
    };

    // 读取 memory.json
    let memory_path = project_root.join(".claude/status/memory.json");
    let mut memory: Memory = read_json(&memory_path).unwrap_or_default();

    // 更新进度中的阶段信息
    memory.progress.current_phase = Some(format!("Phase {}", phase_plan.phase_num));
    memory.progress.current_phase_name = Some(phase_plan.phase_name.clone());
    memory.progress.current_phase_status = Some(phase_plan.status.clone());

    // 写回 memory.json
    write_json(&memory_path, &memory)?;

    log_decision(
        project_root,
        &format!(
            "SYNC: Updated current phase to Phase {}: {}",
            phase_plan.phase_num, phase_plan.phase_name
        ),
    )?;

    Ok(true)
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_sync_from_roadmap() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();

        // 创建目录结构
        fs::create_dir_all(project_root.join(".claude/status")).unwrap();

        // 创建 ROADMAP.md
        let roadmap_content = r#"
# Roadmap

## Current: Phase 1

## Task List
- [ ] TASK-001: Pending task
- [>] TASK-002: In progress task
- [x] TASK-003: Completed task
"#;
        let roadmap_path = project_root.join("ROADMAP.md");
        fs::write(&roadmap_path, roadmap_content).unwrap();

        // 创建初始 memory.json
        let memory = Memory::default();
        let memory_path = project_root.join(".claude/status/memory.json");
        write_json(&memory_path, &memory).unwrap();

        // 执行同步
        let result = sync_from_roadmap(project_root, &roadmap_path).unwrap();
        assert!(result);

        // 验证同步结果
        let updated_memory: Memory = read_json(&memory_path).unwrap();
        assert_eq!(updated_memory.progress.tasks_total, 3);
        assert_eq!(updated_memory.progress.tasks_completed, 1);
        assert_eq!(updated_memory.progress.tasks_pending, 1);
        assert_eq!(updated_memory.progress.tasks_in_progress, 1);
        assert_eq!(
            updated_memory.progress.current_phase,
            Some("Phase 1".to_string())
        );
    }
}
