//! 上下文管理器核心
//!
//! ContextManager - 统一上下文管理（替代历史上的 Python 版本实现）

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::context::truncate::truncate_middle;
use crate::state::{parse_roadmap, Memory};
use crate::state_machine::{GitStateMachine, WorkflowEngine};
use crate::utils::{get_git_log, read_json, try_read_file};

// ═══════════════════════════════════════════════════════════════════
// 常量定义
// ═══════════════════════════════════════════════════════════════════

/// 上下文预算（字符数）
pub const BUDGET_FULL: usize = 80000;
pub const BUDGET_REVIEW: usize = 40000;
pub const BUDGET_TASK: usize = 30000;

/// 状态文件路径
const STATUS_DIR: &str = ".claude/status";
const PHASES_DIR: &str = ".claude/phases";

// ═══════════════════════════════════════════════════════════════════
// 上下文模式
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub enum ContextMode {
    Autonomous,
    Review,
    Task,
}

// ═══════════════════════════════════════════════════════════════════
// 缓存数据（预留用于未来优化）
// ═══════════════════════════════════════════════════════════════════

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CachedData {
    content: String,
    timestamp: std::time::SystemTime,
}

// ═══════════════════════════════════════════════════════════════════
// ContextManager 核心
// ═══════════════════════════════════════════════════════════════════

/// 上下文管理器
///
/// 负责从各种状态文件中读取信息并组装成不同模式的上下文
pub struct ContextManager {
    pub project_root: PathBuf,
    #[allow(dead_code)]
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
}

impl ContextManager {
    /// 创建新的上下文管理器
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 0: 系统指令
    // ═══════════════════════════════════════════════════════════════════

    /// 获取系统头部
    pub fn get_system_header(&self, mode: ContextMode) -> String {
        match mode {
            ContextMode::Autonomous => r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                    🤖 AUTONOMOUS MODE - CONTEXT INJECTION                     ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  ⚠️ WARNING: Your conversation history may be compressed/truncated            ║
║  ⚠️ TRUST ONLY the state files below, NOT your "memory"                       ║
║  ⚠️ CONTINUE the loop - do NOT stop until ROADMAP is complete                 ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#
            .to_string(),
            ContextMode::Review => r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                    🔍 CODE REVIEW MODE - CONTEXT INJECTION                    ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Review the code changes against the API contract and project standards       ║
║  Check for: contract compliance, test coverage, error handling, consistency   ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#
            .to_string(),
            ContextMode::Task => r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                    📋 TASK EXECUTION MODE - CONTEXT INJECTION                 ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Focus on the current task specification below                                ║
║  Follow TDD: write failing test first, then implement, then verify            ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#
            .to_string(),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 1: 当前状态 (memory.json)
    // ═══════════════════════════════════════════════════════════════════

    /// 获取当前状态上下文
    pub fn get_memory_context(&self) -> Result<String> {
        let memory_file = self.project_root.join(STATUS_DIR).join("memory.json");
        let memory: Memory = read_json(&memory_file).unwrap_or_default();

        let mut ctx = String::from("\n## 🧠 CURRENT STATE\n");

        // 当前任务
        if let Some(task_id) = &memory.current_task.id {
            ctx.push_str(&format!(
                r#"
### Current Task
- **ID**: {}
- **Name**: {}
- **Status**: {}
- **Retry Count**: {}/{}
"#,
                task_id,
                memory.current_task.name.as_deref().unwrap_or("Unknown"),
                memory.current_task.status,
                memory.current_task.retry_count,
                memory.current_task.max_retries
            ));
        }

        // 工作上下文
        if let Some(current_file) = &memory.working_context.current_file {
            ctx.push_str(&format!(
                r#"
### Working Context
- **Current File**: `{}`
- **Current Function**: `{}`
"#,
                current_file,
                memory
                    .working_context
                    .current_function
                    .as_deref()
                    .unwrap_or("N/A")
            ));

            if !memory.working_context.pending_tests.is_empty() {
                let tests: Vec<_> = memory
                    .working_context
                    .pending_tests
                    .iter()
                    .take(5)
                    .cloned()
                    .collect();
                ctx.push_str(&format!("- **Pending Tests**: {}\n", tests.join(", ")));
            }
        }

        // 下一步行动
        if !memory.next_action.action.is_empty() {
            ctx.push_str(&format!(
                r#"
### Next Action
- **Action**: {}
- **Target**: {}
- **Reason**: {}
"#,
                memory.next_action.action,
                memory.next_action.target.as_deref().unwrap_or("N/A"),
                memory.next_action.reason.as_deref().unwrap_or("N/A")
            ));
        }

        // 进度
        if memory.progress.tasks_total > 0 {
            let pct = (memory.progress.tasks_completed as f64 / memory.progress.tasks_total as f64)
                * 100.0;
            ctx.push_str(&format!(
                r#"
### Progress
- **Tasks**: {}/{} ({:.1}%)
- **Current Phase**: {}
"#,
                memory.progress.tasks_completed,
                memory.progress.tasks_total,
                pct,
                memory.progress.current_phase.as_deref().unwrap_or("N/A")
            ));
        }

        Ok(ctx)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 2: 任务列表 (ROADMAP.md)
    // ═══════════════════════════════════════════════════════════════════

    /// 获取 ROADMAP 上下文
    pub fn get_roadmap_context(&self, include_completed: bool) -> Result<String> {
        let roadmap_file = self.project_root.join(STATUS_DIR).join("ROADMAP.md");
        let content =
            match try_read_file(&roadmap_file) {
                Some(c) => c,
                None => return Ok(
                    "\n## ❌ ROADMAP NOT FOUND\nInitialize `.claude/status/ROADMAP.md` first!\n"
                        .to_string(),
                ),
            };

        let data = parse_roadmap(&content)?;

        let mut ctx = String::from("\n## 📋 ROADMAP\n");
        let done = data.completed.len() + data.skipped.len();
        ctx.push_str(&format!(
            "\n**Progress**: {}/{} tasks done (completed + skipped)\n",
            done,
            data.total
        ));

        // 进行中
        if !data.in_progress.is_empty() {
            ctx.push_str("\n### 🔄 IN PROGRESS\n");
            for task in &data.in_progress {
                ctx.push_str(&format!("{}\n", task.line));
            }
        }

        // 待处理
        ctx.push_str("\n### ⏳ PENDING\n");
        let pending_count = data.pending.len().min(20);
        for task in data.pending.iter().take(pending_count) {
            ctx.push_str(&format!("{}\n", task.line));
        }
        if data.pending.len() > 20 {
            ctx.push_str(&format!("... and {} more\n", data.pending.len() - 20));
        }

        // 已完成（可选）
        if include_completed && !data.completed.is_empty() {
            ctx.push_str("\n### ✅ COMPLETED (Recent)\n");
            let completed_count = data.completed.len().min(5);
            for task in data.completed.iter().rev().take(completed_count) {
                ctx.push_str(&format!("{}\n", task.line));
            }
        }

        Ok(ctx)
    }

    /// 获取当前任务规格
    pub fn get_current_task_spec(&self) -> Result<String> {
        let memory_file = self.project_root.join(STATUS_DIR).join("memory.json");
        let memory: Memory = read_json(&memory_file).unwrap_or_default();

        let task_id = match &memory.current_task.id {
            Some(id) => id,
            None => return Ok(String::new()),
        };

        // 在 phases 目录中查找任务文件
        let phases_dir = self.project_root.join(PHASES_DIR);
        if !phases_dir.exists() {
            return Ok(String::new());
        }

        for entry in std::fs::read_dir(&phases_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            for file_entry in std::fs::read_dir(entry.path())? {
                let file_entry = file_entry?;
                let file_name = file_entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                if file_name_str.contains(task_id) && file_name_str.ends_with(".md") {
                    if let Some(content) = try_read_file(&file_entry.path()) {
                        return Ok(format!(
                            "\n## 📝 CURRENT TASK SPEC: {}\n```markdown\n{}\n```\n",
                            task_id, content
                        ));
                    }
                }
            }
        }

        Ok(String::new())
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 3: 错误历史
    // ═══════════════════════════════════════════════════════════════════

    /// 获取错误上下文
    pub fn get_error_context(&self, task_filter: Option<&str>) -> Result<String> {
        let error_file = self
            .project_root
            .join(STATUS_DIR)
            .join("error_history.json");

        let errors: Vec<serde_json::Value> = match read_json(&error_file) {
            Ok(e) => e,
            Err(_) => return Ok(String::new()),
        };

        if errors.is_empty() {
            return Ok(String::new());
        }

        // 过滤相关错误
        let relevant: Vec<_> = if let Some(task) = task_filter {
            errors
                .iter()
                .filter(|e| {
                    e.get("task")
                        .and_then(|t| t.as_str())
                        .map(|t| t == task)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            errors.iter().rev().take(15).collect()
        };

        if relevant.is_empty() {
            return Ok(String::new());
        }

        let mut ctx = String::from("\n## ⚠️ ERROR HISTORY (MUST AVOID REPEATING)\n");

        let unresolved: Vec<_> = relevant
            .iter()
            .filter(|e| e.get("resolution").is_none() || e["resolution"].is_null())
            .collect();

        if !unresolved.is_empty() {
            ctx.push_str("\n### ❌ Unresolved Errors\n");
            for err in unresolved.iter().rev().take(5) {
                let task = err["task"].as_str().unwrap_or("unknown");
                let error = err["error"].as_str().unwrap_or("unknown");
                let attempted = err["attempted_fix"].as_str().unwrap_or("N/A");

                ctx.push_str(&format!(
                    r#"
**Task**: {}
**Error**: {}
**Attempted**: {}
---
"#,
                    task,
                    &error.chars().take(200).collect::<String>(),
                    &attempted.chars().take(100).collect::<String>()
                ));
            }
        }

        Ok(ctx)
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 4: API 契约
    // ═══════════════════════════════════════════════════════════════════

    /// 获取 API 契约上下文
    pub fn get_contract_context(&self) -> Result<String> {
        let contract_file = self.project_root.join(STATUS_DIR).join("api_contract.yaml");

        let content = match try_read_file(&contract_file) {
            Some(c) => c,
            None => return Ok(String::new()),
        };

        Ok(format!(
            "\n## 📜 API CONTRACT\n```yaml\n{}\n```\n",
            truncate_middle(&content, 8000)
        ))
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 4.5: Repository Map (代码骨架)
    // ═══════════════════════════════════════════════════════════════════

    /// 获取 Repository Map 上下文
    pub fn get_repo_map_context(&self) -> Result<String> {
        // 优先使用 TOON（更省 token），其次 Markdown
        let candidates = [
            (".claude/repo_map/structure.toon", "TOON"),
            (".claude/repo_map/structure.md", "Markdown"),
        ];

        let mut selected: Option<(std::path::PathBuf, &'static str)> = None;

        for (rel_path, label) in candidates {
            let path = self.project_root.join(rel_path);
            if path.exists() {
                selected = Some((path, label));
                break;
            }
        }

        let Some((repo_map_file, label)) = selected else {
            // 未生成则只给极短提示，避免每次注入都浪费 token
            return Ok("\n## 🗺️ REPOSITORY MAP\n\n*Not generated. Run `claude-autonomous map` (recommended: default TOON).* \n".to_string());
        };

        let content = match try_read_file(&repo_map_file) {
            Some(c) => c,
            None => return Ok(String::new()),
        };

        // Repository Map 通常较大，限制在 15K tokens 左右
        Ok(format!(
            "\n## 🗺️ REPOSITORY MAP (Code Skeleton - {})\n```text\n{}\n```\n",
            label,
            truncate_middle(&content, 15000)
        ))
    }

    // ═══════════════════════════════════════════════════════════════════
    // Layer 5-8: 其他上下文
    // ═══════════════════════════════════════════════════════════════════

    /// 获取 Git 历史上下文
    pub fn get_git_context(&self, limit: usize) -> Result<String> {
        match get_git_log(limit, Some(&self.project_root)) {
            Ok(log) => Ok(format!(
                "\n## 📜 RECENT GIT HISTORY\n```\n{}\n```\n",
                &log.chars().take(2000).collect::<String>()
            )),
            Err(_) => Ok(String::new()),
        }
    }

    /// 获取决策日志上下文
    pub fn get_decisions_context(&self, limit: usize) -> Result<String> {
        let log_file = self.project_root.join(STATUS_DIR).join("decisions.log");

        let content = match try_read_file(&log_file) {
            Some(c) => c,
            None => return Ok(String::new()),
        };

        let lines: Vec<_> = content.lines().rev().take(limit).collect();
        if lines.is_empty() {
            return Ok(String::new());
        }

        let recent: String = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
        Ok(format!("\n## 📝 RECENT DECISIONS\n```\n{}\n```\n", recent))
    }

    /// 获取状态机上下文（新增）
    pub fn get_state_machine_context(&self) -> Result<String> {
        // 默认关闭：只有当用户显式启用（创建了 state.json）后才注入状态机上下文
        let state_file = self.project_root.join(STATUS_DIR).join("state.json");
        if !state_file.exists() {
            return Ok(String::new());
        }

        // 尝试加载状态机
        let state_machine = match GitStateMachine::new(&self.project_root) {
            Ok(sm) => sm,
            Err(_) => {
                // 如果不是 git 仓库或没有初始化，返回空
                return Ok(String::new());
            }
        };

        // 获取当前状态
        let current_state = state_machine.current_state()?;

        let mut ctx = String::from("\n## 🔄 STATE MACHINE\n\n");

        // 当前状态
        ctx.push_str(&format!(
            "**Current State**: {} {}\n",
            current_state.state_id.icon(),
            current_state.state_id.as_str().to_uppercase()
        ));

        if let Some(task_id) = &current_state.task_id {
            ctx.push_str(&format!("**Task ID**: {}\n", task_id));
        }

        if let Some(phase) = &current_state.phase {
            ctx.push_str(&format!("**Phase**: {}\n", phase));
        }

        // 状态描述
        ctx.push_str(&format!(
            "**Description**: {}\n\n",
            WorkflowEngine::state_description(current_state.state_id)
        ));

        // 可能的后继状态
        let next_states = WorkflowEngine::next_states(current_state.state_id);
        if !next_states.is_empty() {
            ctx.push_str("**Possible Next States**:\n");
            for next in &next_states {
                let recommended = if WorkflowEngine::recommend_next_state(current_state.state_id)
                    == Some(*next)
                {
                    " (Recommended)"
                } else {
                    ""
                };

                ctx.push_str(&format!(
                    "  → {} {}{}\n",
                    next.icon(),
                    next.as_str(),
                    recommended
                ));
            }
            ctx.push('\n');
        }

        // 最近的状态转换历史（最多 5 个）
        let snapshots = state_machine.list_states()?;
        if snapshots.len() > 1 {
            ctx.push_str("**Recent Transitions**:\n");
            for snapshot in snapshots.iter().take(5) {
                if let Some((state_id, task_id)) = snapshot.parse_tag_info() {
                    let task_str = task_id.as_deref().unwrap_or("-");
                    ctx.push_str(&format!(
                        "  {} {} [{}] - {}\n",
                        state_id.icon(),
                        state_id.as_str(),
                        task_str,
                        snapshot.formatted_time()
                    ));
                }
            }
        }

        Ok(ctx)
    }

    // ═══════════════════════════════════════════════════════════════════
    // 组装方法
    // ═══════════════════════════════════════════════════════════════════

    /// 获取完整上下文（用于 UserPromptSubmit）
    pub fn get_full_context(&self) -> Result<String> {
        let parts = [
            self.get_system_header(ContextMode::Autonomous),
            self.get_memory_context()?,
            self.get_state_machine_context()?, // 新增：State Machine
            self.get_roadmap_context(false)?,
            self.get_current_task_spec()?,
            self.get_repo_map_context()?, // Repository Map
            self.get_error_context(None)?,
            self.get_contract_context()?,
            self.get_git_context(10)?,
            self.get_decisions_context(20)?,
        ];

        let mut ctx = parts.join("");

        // 添加行动指令
        ctx.push_str(
            r#"
═══════════════════════════════════════════════════════════════════════════════
📌 MANDATORY ACTIONS:
1. Read the CURRENT STATE above carefully
2. Check ERROR HISTORY to avoid repeating mistakes
3. Follow the NEXT ACTION from memory.json
4. Execute following TDD (test first, then implement)
5. Update memory.json IMMEDIATELY after any progress
6. Continue loop - DO NOT STOP until all tasks are [x] marked
═══════════════════════════════════════════════════════════════════════════════
"#,
        );

        Ok(truncate_middle(&ctx, BUDGET_FULL))
    }

    /// 获取代码审查上下文
    pub fn get_review_context(&self, _changed_files: &[String]) -> Result<String> {
        let parts = [
            self.get_system_header(ContextMode::Review),
            self.get_memory_context()?,
            self.get_current_task_spec()?,
            self.get_contract_context()?,
            self.get_error_context(None)?,
        ];

        let mut ctx = parts.join("");

        // 添加审查检查清单
        ctx.push_str(
            r#"
═══════════════════════════════════════════════════════════════════════════════
📌 REVIEW CHECKLIST:
1. Does the code match the API CONTRACT exactly? (signatures, types, returns)
2. Are there comprehensive tests? (happy path + edge cases + error cases)
3. Is error handling complete?
4. Does it follow project conventions?
5. Any security concerns?
═══════════════════════════════════════════════════════════════════════════════
"#,
        );

        Ok(truncate_middle(&ctx, BUDGET_REVIEW))
    }

    /// 获取任务上下文
    pub fn get_task_context(&self, task_id: &str) -> Result<String> {
        let parts = [
            self.get_system_header(ContextMode::Task),
            self.get_memory_context()?,
            self.get_current_task_spec()?,
            self.get_contract_context()?,
            self.get_error_context(Some(task_id))?,
        ];

        let ctx = parts.join("");
        Ok(truncate_middle(&ctx, BUDGET_TASK))
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_context_manager_new() {
        let temp = TempDir::new().unwrap();
        let manager = ContextManager::new(temp.path().to_path_buf());
        assert_eq!(manager.project_root, temp.path());
    }

    #[test]
    fn test_get_system_header() {
        let temp = TempDir::new().unwrap();
        let manager = ContextManager::new(temp.path().to_path_buf());

        let header = manager.get_system_header(ContextMode::Autonomous);
        assert!(header.contains("AUTONOMOUS MODE"));

        let review_header = manager.get_system_header(ContextMode::Review);
        assert!(review_header.contains("CODE REVIEW MODE"));
    }

    #[test]
    fn test_get_memory_context() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        let manager = ContextManager::new(temp.path().to_path_buf());
        let ctx = manager.get_memory_context().unwrap();
        assert!(ctx.contains("CURRENT STATE"));
    }
}
