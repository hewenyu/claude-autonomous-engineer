//! 审查上下文组装器
//!
//! 为 codex review 准备上下文信息

use crate::hooks::state_tracker::{TaskSnapshot, TransitionType};
use crate::state::CurrentTask;
use crate::utils::try_read_file;
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use std::path::PathBuf;

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

        // 加载 ROADMAP 摘要
        let roadmap_summary = summarize_roadmap(project_root)?;

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

## Overall Progress
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
            roadmap_summary,
        );

        Ok(ReviewContext {
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

        // 构建常规审查指令
        let instruction = format!(
            r#"# Code Review - Task In Progress

## Current Task
{}

## Staged Changes
{}

## API Contract
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
            task_spec, diff, api_contract,
        );

        Ok(ReviewContext {
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

/// 总结 ROADMAP
fn summarize_roadmap(project_root: &Path) -> Result<String> {
    let roadmap_file = project_root.join(".claude/status/ROADMAP.md");

    match try_read_file(&roadmap_file) {
        Some(content) => {
            // 提取前 20 行作为摘要
            let lines: Vec<&str> = content.lines().take(20).collect();
            Ok(lines.join("\n"))
        }
        None => Ok("*No ROADMAP found*".to_string()),
    }
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
}
