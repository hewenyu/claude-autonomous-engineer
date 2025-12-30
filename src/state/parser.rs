//! 状态文件解析器
//!
//! 解析 Markdown, YAML, JSON

use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;

use super::{PhasePlan, RoadmapData, TaskDetail, TaskItem};

// ═══════════════════════════════════════════════════════════════════
// 正则表达式定义
// ═══════════════════════════════════════════════════════════════════

lazy_static! {
    // 任务状态正则
    static ref TASK_PENDING: Regex = Regex::new(r"^-\s*\[\s*\]").unwrap();
    static ref TASK_IN_PROGRESS: Regex = Regex::new(r"^-\s*\[(>|~)\]").unwrap();
    static ref TASK_COMPLETED: Regex = Regex::new(r"^-\s*\[[xX]\]").unwrap();
    static ref TASK_BLOCKED: Regex = Regex::new(r"^-\s*\[!\]").unwrap();

    // 任务 ID 正则
    static ref TASK_ID: Regex = Regex::new(r"(TASK-\d+|#\d+)").unwrap();

    // 阶段正则 - 捕获整个 "Phase N" 或只是数字
    static ref CURRENT_PHASE: Regex = Regex::new(r"##\s*Current[:\s]+(Phase\s+\d+|[A-Za-z0-9\s]+)").unwrap();
    static ref PHASE_HEADER: Regex = Regex::new(r"#\s*Phase\s*(\d+)[:\s]+(.+)").unwrap();

    // 状态正则
    static ref STATUS_LINE: Regex = Regex::new(r"##\s*Status[:\s]+(\w+)").unwrap();

    // 任务标题正则
    static ref TASK_TITLE: Regex = Regex::new(r"^#\s*(?:TASK-\d+[:\s]+)?(.+)$").unwrap();
}

// ═══════════════════════════════════════════════════════════════════
// ROADMAP.md 解析
// ═══════════════════════════════════════════════════════════════════

/// 解析 ROADMAP.md
///
/// 提取所有任务的状态：pending, in_progress, completed, blocked
pub fn parse_roadmap(content: &str) -> Result<RoadmapData> {
    let mut pending = Vec::new();
    let mut in_progress = Vec::new();
    let mut completed = Vec::new();
    let mut blocked = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // 提取任务 ID（如果有）
        let task_id = TASK_ID
            .captures(line)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        let item = TaskItem {
            line: line.to_string(),
            id: task_id,
        };

        if TASK_PENDING.is_match(trimmed) {
            pending.push(item);
        } else if TASK_IN_PROGRESS.is_match(trimmed) {
            in_progress.push(item);
        } else if TASK_COMPLETED.is_match(trimmed) {
            completed.push(item);
        } else if TASK_BLOCKED.is_match(trimmed) {
            blocked.push(item);
        }
    }

    // 解析当前阶段
    let current_phase = CURRENT_PHASE
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let total = pending.len() + in_progress.len() + completed.len() + blocked.len();

    Ok(RoadmapData {
        pending,
        in_progress,
        completed,
        blocked,
        current_phase,
        total,
    })
}

// ═══════════════════════════════════════════════════════════════════
// TASK-xxx.md 解析
// ═══════════════════════════════════════════════════════════════════

/// 解析任务文件
///
/// 提取任务详情：ID, 名称, 状态, 验收标准等
pub fn parse_task_file(content: &str, task_id: &str) -> Result<TaskDetail> {
    // 提取状态
    let status = STATUS_LINE
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // 提取名称（从第一个 # 标题）
    let name = content
        .lines()
        .find(|line| line.trim().starts_with('#'))
        .and_then(|line| TASK_TITLE.captures(line))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| task_id.to_string());

    // 提取阶段
    let phase = extract_field(content, r"##\s*Phase[:\s]+(.+)");

    // 提取依赖
    let dependencies = extract_list_items(content, r"##\s*Dependenc", r"(TASK-\d+)");

    // 提取验收标准
    let acceptance_criteria = extract_section_list(content, r"##\s*Acceptance");

    Ok(TaskDetail {
        id: task_id.to_string(),
        name,
        status,
        phase,
        dependencies,
        acceptance_criteria,
    })
}

// ═══════════════════════════════════════════════════════════════════
// PHASE_PLAN.md 解析
// ═══════════════════════════════════════════════════════════════════

/// 解析阶段计划文件
pub fn parse_phase_plan(content: &str) -> Result<Option<PhasePlan>> {
    // 提取阶段信息
    let phase_info = PHASE_HEADER.captures(content);
    if phase_info.is_none() {
        return Ok(None);
    }

    let caps = phase_info.unwrap();
    let phase_num = caps.get(1).map(|m| m.as_str().to_string()).unwrap();
    let phase_name = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap();

    // 提取阶段状态
    let status = STATUS_LINE
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    Ok(Some(PhasePlan {
        phase_num,
        phase_name,
        status,
    }))
}

// ═══════════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════════

/// 提取单个字段
fn extract_field(content: &str, pattern: &str) -> Option<String> {
    Regex::new(pattern)
        .ok()?
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

/// 提取列表项（匹配特定模式）
fn extract_list_items(content: &str, section_pattern: &str, item_pattern: &str) -> Vec<String> {
    let section_regex = Regex::new(section_pattern).unwrap();
    let item_regex = Regex::new(item_pattern).unwrap();

    let mut in_section = false;
    let mut items = Vec::new();

    for line in content.lines() {
        if section_regex.is_match(line) {
            in_section = true;
            continue;
        }

        if in_section {
            // 遇到下一个标题，停止
            if line.trim().starts_with("##") {
                break;
            }

            // 提取匹配项
            if let Some(caps) = item_regex.captures(line) {
                if let Some(m) = caps.get(1) {
                    items.push(m.as_str().to_string());
                }
            }
        }
    }

    items
}

/// 提取章节中的列表项
fn extract_section_list(content: &str, section_pattern: &str) -> Vec<String> {
    let section_regex = Regex::new(section_pattern).unwrap();

    let mut in_section = false;
    let mut items = Vec::new();

    for line in content.lines() {
        if section_regex.is_match(line) {
            in_section = true;
            continue;
        }

        if in_section {
            // 遇到下一个标题，停止
            if line.trim().starts_with("##") {
                break;
            }

            // 提取列表项
            let trimmed = line.trim();
            if trimmed.starts_with("- [") {
                items.push(trimmed.to_string());
            }
        }
    }

    items
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_roadmap() {
        let content = r#"
# Roadmap

## Current: Phase 1

## Task List

### Phase 1
- [ ] TASK-001: Pending task
- [>] TASK-002: In progress task
- [x] TASK-003: Completed task
- [!] TASK-004: Blocked task

### Phase 2
- [ ] TASK-005: Another pending
"#;

        let data = parse_roadmap(content).unwrap();
        assert_eq!(data.pending.len(), 2);
        assert_eq!(data.in_progress.len(), 1);
        assert_eq!(data.completed.len(), 1);
        assert_eq!(data.blocked.len(), 1);
        assert_eq!(data.total, 5);
        assert_eq!(data.current_phase, Some("Phase 1".to_string()));
    }

    #[test]
    fn test_parse_task_file() {
        let content = r#"
# TASK-001: Implement Authentication

## Status: In Progress

## Phase: 1

## Dependencies
- TASK-000: Setup project

## Acceptance Criteria
- [ ] Login works
- [x] Register works
- [ ] Logout works
"#;

        let task = parse_task_file(content, "TASK-001").unwrap();
        assert_eq!(task.id, "TASK-001");
        assert_eq!(task.name, "Implement Authentication");
        assert_eq!(task.status, "In");
        assert_eq!(task.phase, Some("1".to_string()));
        assert_eq!(task.dependencies.len(), 1);
        assert_eq!(task.acceptance_criteria.len(), 3);
    }

    #[test]
    fn test_parse_phase_plan() {
        let content = r#"
# Phase 1: Foundation

## Status: In Progress

## Objectives
- Setup project
"#;

        let phase = parse_phase_plan(content).unwrap().unwrap();
        assert_eq!(phase.phase_num, "1");
        assert_eq!(phase.phase_name, "Foundation");
        assert_eq!(phase.status, "In");
    }
}

