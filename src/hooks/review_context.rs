//! 审查上下文组装器
//!
//! 为 codex review 准备上下文信息

use crate::hooks::state_tracker::{TaskSnapshot, TransitionType};
use crate::state::CurrentTask;
use crate::utils::try_read_file;
use anyhow::Result;
use std::path::Path;
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

    // 尝试多个可能的路径
    let possible_paths = vec![
        project_root.join(format!(".claude/phases/{}.md", task_id)),
        project_root.join(format!(".claude/status/{}.md", task_id)),
    ];

    for path in possible_paths {
        if let Some(content) = try_read_file(&path) {
            return Ok(content);
        }
    }

    Ok(format!("*Task spec for {} not found*", task_id))
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
}
