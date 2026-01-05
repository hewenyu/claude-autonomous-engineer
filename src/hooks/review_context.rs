//! 审查上下文组装器
//!
//! 为 codex review 准备上下文信息

use crate::hooks::state_tracker::{TaskSnapshot, TransitionType};
use crate::state::CurrentTask;
use crate::utils::try_read_file;
use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// 审查模式
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewMode {
    /// 常规审查（持续执行时）
    Regular,
    /// 深度审查（状态转换时）
    Deep,
}

/// 审查上下文
pub struct ReviewContext {
    pub project_root: PathBuf,
    pub instruction: String,
    pub mode: ReviewMode,
}

impl ReviewContext {
    /// 构建深度审查上下文
    pub fn build_deep(
        project_root: &Path,
        current_task: &CurrentTask,
        previous_snapshot: &Option<TaskSnapshot>,
        transition_type: &TransitionType,
    ) -> Result<Self> {
        // 获取 staged changes
        let diff = get_staged_diff(project_root)?;

        // 加载原始需求
        let requirements = read_requirements(project_root)?;

        // 加载任务规格
        let task_spec = read_task_spec(project_root, &current_task.id)?;

        // 加载 API 契约
        let api_contract = read_api_contract(project_root)?;

        // 加载完整 ROADMAP
        let roadmap_full = read_full_roadmap(project_root)?;

        // 获取当前 phase 编号
        let current_phase = get_current_phase_number(project_root)?;

        // 加载当前 phase plan
        let phase_plan = read_phase_plan(project_root, current_phase)?
            .unwrap_or_else(|| "*No PHASE_PLAN found for current phase*".to_string());

        // 加载前一阶段摘要
        let prev_phase_summary = read_previous_phase_summary(project_root, current_phase)?
            .unwrap_or_else(|| "*No previous phase summary (this is phase 1)*".to_string());

        let prev_status = previous_snapshot
            .as_ref()
            .map(|s| s.status.as_str())
            .unwrap_or("UNKNOWN");

        // 构建深度审查指令
        let instruction = format!(
            r#"# Code Review - Task State Transition

⚠️ **CRITICAL REVIEW**: Task state is changing from {} → {}

## State Transition Context
- Previous State: {}
- New State: {}
- Task ID: {}
- Transition Type: {:?}

## Original Requirements
{}

## Current Task Specification
{}

## Staged Changes
{}

## API Contract Validation
{}

## Full Project Roadmap
{}

## Current Phase Plan
{}

## Previous Phase Context
{}

## State Transition Review Checklist
- [ ] 变更是否完整实现了任务要求？
- [ ] 是否符合原始需求的设计意图？
- [ ] API 契约是否被正确遵守？
- [ ] 状态转换是否合理（如COMPLETED需要测试通过）？
- [ ] 是否引入了技术债务？
- [ ] 文档和注释是否完整？

Output format:
VERDICT: PASS | FAIL | WARN
STATE_TRANSITION_VALID: YES | NO
ISSUES:
- [Severity: CRITICAL|ERROR|WARN] Description
"#,
            prev_status,
            current_task.status,
            prev_status,
            current_task.status,
            current_task.id.as_ref().unwrap_or(&"UNKNOWN".to_string()),
            transition_type,
            requirements,
            task_spec,
            diff,
            api_contract,
            roadmap_full,
            phase_plan,
            prev_phase_summary,
        );

        Ok(ReviewContext {
            project_root: project_root.to_path_buf(),
            instruction,
            mode: ReviewMode::Deep,
        })
    }

    /// 构建常规审查上下文
    pub fn build_regular(project_root: &Path, current_task: &CurrentTask) -> Result<Self> {
        // 获取 staged changes
        let diff = get_staged_diff(project_root)?;

        // 加载任务规格
        let task_spec = read_task_spec(project_root, &current_task.id)?;

        // 加载 API 契约
        let api_contract = read_api_contract(project_root)?;

        // 加载完整 ROADMAP
        let roadmap_full = read_full_roadmap(project_root)?;

        // 获取当前 phase 编号
        let current_phase = get_current_phase_number(project_root)?;

        // 加载当前 phase plan
        let phase_plan = read_phase_plan(project_root, current_phase)?
            .unwrap_or_else(|| "*No PHASE_PLAN found for current phase*".to_string());

        // 加载前一阶段摘要
        let prev_phase_summary = read_previous_phase_summary(project_root, current_phase)?
            .unwrap_or_else(|| "*No previous phase summary (this is phase 1)*".to_string());

        // 构建常规审查指令
        let instruction = format!(
            r#"# Code Review - Task In Progress

## Current Task
{}

## Staged Changes
{}

## API Contract
{}

## Full Project Roadmap
{}

## Current Phase Plan
{}

## Previous Phase Context
{}

## Regular Review Checklist
- [ ] 代码符合任务要求
- [ ] 函数签名符合 API 契约
- [ ] 错误处理完整
- [ ] 无明显安全问题
- [ ] 遵循项目规范

Output format:
VERDICT: PASS | FAIL | WARN
ISSUES:
- [Severity: ERROR|WARN] Description
"#,
            task_spec,
            diff,
            api_contract,
            roadmap_full,
            phase_plan,
            prev_phase_summary,
        );

        Ok(ReviewContext {
            project_root: project_root.to_path_buf(),
            instruction,
            mode: ReviewMode::Regular,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════
// 辅助函数
// ═══════════════════════════════════════════════════════════════════

/// 获取 staged files 的 diff
fn get_staged_diff(project_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(project_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to get staged diff: {}", stderr);
    }

    let diff = String::from_utf8_lossy(&output.stdout).to_string();

    if diff.trim().is_empty() {
        Ok("*No staged changes*".to_string())
    } else {
        Ok(diff)
    }
}

/// 读取原始需求
fn read_requirements(project_root: &Path) -> Result<String> {
    let requirements_file = project_root.join(".claude/status/requirements.md");

    match try_read_file(&requirements_file) {
        Some(content) => Ok(content),
        None => Ok("*No requirements.md file found*".to_string()),
    }
}

/// 读取任务规格
fn read_task_spec(project_root: &Path, task_id: &Option<String>) -> Result<String> {
    let task_id = match task_id {
        Some(id) => id,
        None => return Ok("*No current task*".to_string()),
    };

    if let Some(path) = find_task_spec_file(project_root, task_id) {
        if let Some(content) = try_read_file(&path) {
            return Ok(content);
        }
    }

    Ok(format!("*Task spec for {} not found*", task_id))
}

fn find_task_spec_file(project_root: &Path, task_id: &str) -> Option<PathBuf> {
    // 1) Explicit status path (simple override)
    let status_candidate = project_root.join(format!(".claude/status/{}.md", task_id));
    if status_candidate.exists() {
        return Some(status_candidate);
    }

    // 2) Search under phases (expected layout: .claude/phases/phase-N_xxx/TASK-NNN_xxx.md)
    let phases_root = project_root.join(".claude/phases");
    if !phases_root.is_dir() {
        return None;
    }

    fn walk(dir: &Path, task_id: &str, depth_left: usize) -> Option<PathBuf> {
        if depth_left == 0 {
            return None;
        }

        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let ft = entry.file_type().ok()?;

            if ft.is_dir() {
                if let Some(found) = walk(&path, task_id, depth_left - 1) {
                    return Some(found);
                }
                continue;
            }

            if !ft.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if filename.ends_with(".md") && filename.contains(task_id) {
                return Some(path);
            }
        }
        None
    }

    walk(&phases_root, task_id, 4)
}

/// 读取 API 契约
fn read_api_contract(project_root: &Path) -> Result<String> {
    let contract_file = project_root.join(".claude/status/api_contract.yaml");

    match try_read_file(&contract_file) {
        Some(content) => Ok(content),
        None => Ok("*No API contract defined*".to_string()),
    }
}

/// 读取完整 ROADMAP
fn read_full_roadmap(project_root: &Path) -> Result<String> {
    let roadmap_file = project_root.join(".claude/status/ROADMAP.md");

    match try_read_file(&roadmap_file) {
        Some(content) => Ok(content), // 返回完整内容，不再截取前 20 行
        None => Ok("*No ROADMAP found*".to_string()),
    }
}

/// 从 memory.json 中获取当前 phase 编号
fn get_current_phase_number(project_root: &Path) -> Result<u32> {
    use crate::utils::read_json;
    use crate::state::Memory;

    let memory_file = project_root.join(".claude/status/memory.json");
    let memory: Memory = read_json(&memory_file).unwrap_or_default();

    // 从 memory.progress.current_phase 解析编号
    if let Some(phase_str) = memory.progress.current_phase {
        // 可能的格式: "1", "Phase 1", "phase-1" 等
        let num_str: String = phase_str.chars()
            .filter(|c| c.is_ascii_digit())
            .collect();

        if let Ok(num) = num_str.parse::<u32>() {
            return Ok(num);
        }
    }

    // 默认为 phase 1
    Ok(1)
}

/// 读取当前阶段的 PHASE_PLAN.md
fn read_phase_plan(project_root: &Path, phase_number: u32) -> Result<Option<String>> {
    // 搜索 .claude/phases/phase-{N}_*/PHASE_PLAN.md
    let phases_dir = project_root.join(".claude/phases");

    if !phases_dir.is_dir() {
        return Ok(None);
    }

    // 查找匹配 phase-{N}_ 前缀的目录
    let entries = std::fs::read_dir(&phases_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // 匹配 phase-N_ 格式
        if dir_name.starts_with(&format!("phase-{}_", phase_number)) {
            let plan_file = path.join("PHASE_PLAN.md");
            if let Some(content) = try_read_file(&plan_file) {
                return Ok(Some(content));
            }
        }
    }

    Ok(None)
}

/// 提取文档摘要
fn extract_summary(content: &str) -> String {
    // 查找 "## Summary" section
    if let Some(pos) = content.find("## Summary") {
        let after_summary = &content[pos..];
        if let Some(next_section) = after_summary.find("\n## ") {
            return after_summary[..next_section].to_string();
        }
    }

    // 如果没有 Summary section，返回前 30 行
    content.lines().take(30).collect::<Vec<_>>().join("\n")
}

/// 读取前一阶段的摘要
fn read_previous_phase_summary(project_root: &Path, current_phase: u32) -> Result<Option<String>> {
    if current_phase <= 1 {
        return Ok(None); // 第一阶段没有前置阶段
    }

    let prev_phase = current_phase - 1;

    // 尝试读取前一阶段的 PHASE_PLAN.md，提取摘要部分
    if let Some(plan_content) = read_phase_plan(project_root, prev_phase)? {
        // 提取 "## Summary" 或前 30 行作为摘要
        let summary = extract_summary(&plan_content);
        return Ok(Some(format!("Phase {} Summary:\n{}", prev_phase, summary)));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_staged_diff() {
        let temp = tempfile::TempDir::new().unwrap();

        // 初始化一个最小 git 仓库（不需要任何提交也能运行 git diff --cached）
        let status = std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .status()
            .unwrap();
        assert!(status.success());

        let result = get_staged_diff(temp.path()).unwrap();
        // 空仓库/无暂存修改时，应返回占位文案或空 diff
        assert!(!result.is_empty());
    }

    #[test]
    fn test_find_task_spec_file_in_phases() {
        let temp = tempfile::TempDir::new().unwrap();
        let phases_dir = temp.path().join(".claude/phases/phase-1_test");
        std::fs::create_dir_all(&phases_dir).unwrap();

        let task_path = phases_dir.join("TASK-001_test.md");
        std::fs::write(&task_path, "# TASK-001: Example\n").unwrap();

        let found = find_task_spec_file(temp.path(), "TASK-001").unwrap();
        assert_eq!(found, task_path);
    }

    #[test]
    fn test_read_full_roadmap() {
        let temp = tempfile::TempDir::new().unwrap();
        let status_dir = temp.path().join(".claude/status");
        std::fs::create_dir_all(&status_dir).unwrap();

        let roadmap_content = "# Project Roadmap\n\nLine 1\nLine 2\nLine 3\n...many more lines...\nLine 25";
        let roadmap_file = status_dir.join("ROADMAP.md");
        std::fs::write(&roadmap_file, roadmap_content).unwrap();

        let result = read_full_roadmap(temp.path()).unwrap();
        // 应该读取完整内容，包含所有行（不只是前 20 行）
        assert!(result.contains("Line 25"));
        assert_eq!(result, roadmap_content);
    }

    #[test]
    fn test_read_phase_plan() {
        let temp = tempfile::TempDir::new().unwrap();
        let phase_dir = temp.path().join(".claude/phases/phase-2_implementation");
        std::fs::create_dir_all(&phase_dir).unwrap();

        let plan_content = "# Phase 2 Plan\n\n## Summary\nThis is phase 2";
        let plan_file = phase_dir.join("PHASE_PLAN.md");
        std::fs::write(&plan_file, plan_content).unwrap();

        // 查找 phase 2 的 plan
        let result = read_phase_plan(temp.path(), 2).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), plan_content);

        // 查找不存在的 phase 3
        let result_none = read_phase_plan(temp.path(), 3).unwrap();
        assert!(result_none.is_none());
    }

    #[test]
    fn test_get_current_phase_number() {
        use crate::state::Memory;
        use crate::state::models::Progress;

        let temp = tempfile::TempDir::new().unwrap();
        let status_dir = temp.path().join(".claude/status");
        std::fs::create_dir_all(&status_dir).unwrap();

        // 测试格式 "1"
        let memory = Memory {
            progress: Progress {
                current_phase: Some("1".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let memory_file = status_dir.join("memory.json");
        std::fs::write(&memory_file, serde_json::to_string_pretty(&memory).unwrap()).unwrap();

        let result = get_current_phase_number(temp.path()).unwrap();
        assert_eq!(result, 1);

        // 测试格式 "Phase 2"
        let memory2 = Memory {
            progress: Progress {
                current_phase: Some("Phase 2".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(&memory_file, serde_json::to_string_pretty(&memory2).unwrap()).unwrap();

        let result2 = get_current_phase_number(temp.path()).unwrap();
        assert_eq!(result2, 2);

        // 测试格式 "phase-3"
        let memory3 = Memory {
            progress: Progress {
                current_phase: Some("phase-3".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(&memory_file, serde_json::to_string_pretty(&memory3).unwrap()).unwrap();

        let result3 = get_current_phase_number(temp.path()).unwrap();
        assert_eq!(result3, 3);
    }

    #[test]
    fn test_extract_summary() {
        // 测试包含 ## Summary section 的情况
        let content_with_summary = r#"# Plan

## Summary
This is a summary
of the plan

## Details
Some details here"#;

        let summary = extract_summary(content_with_summary);
        assert!(summary.contains("This is a summary"));
        assert!(summary.contains("of the plan"));
        assert!(!summary.contains("## Details"));

        // 测试没有 Summary section 的情况（返回前 30 行）
        let content_no_summary = "Line 1\nLine 2\nLine 3\n";
        let summary2 = extract_summary(content_no_summary);
        assert_eq!(summary2, content_no_summary.trim_end());
    }
}
