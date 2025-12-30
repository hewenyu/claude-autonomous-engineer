// Context Builder
// 上下文构建器 - 组装不同模式的上下文

use super::contract::ApiContract;
use super::errors::ErrorHistory;
use super::types::Memory;  // 修复: Memory 定义在 types.rs 中
use super::roadmap::Roadmap;
use super::structure::ProjectStructure;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum ContextMode {
    Autonomous,  // 完整上下文 (inject_state)
    Review,      // 代码审查上下文 (codex_review)
    Task,        // 任务执行上下文
}

pub struct ContextBuilder {
    project_root: PathBuf,
    mode: ContextMode,
    parts: Vec<String>,
}

impl ContextBuilder {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            mode: ContextMode::Autonomous,
            parts: Vec::new(),
        }
    }

    pub fn mode(mut self, mode: ContextMode) -> Self {
        self.mode = mode;
        self
    }

    fn add_header(&mut self) -> Result<()> {
        let header = match self.mode {
            ContextMode::Autonomous => {
                r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                    🤖 AUTONOMOUS MODE - CONTEXT INJECTION                     ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  ⚠️ WARNING: Your conversation history may be compressed/truncated            ║
║  ⚠️ TRUST ONLY the state files below, NOT your "memory"                       ║
║  ⚠️ CONTINUE the loop - do NOT stop until ROADMAP is complete                 ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#
            }
            ContextMode::Review => {
                r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                    🔍 CODE REVIEW MODE - CONTEXT INJECTION                    ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Review the code changes against the API contract and project standards       ║
║  Check for: contract compliance, test coverage, error handling, consistency   ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#
            }
            ContextMode::Task => {
                r#"
╔══════════════════════════════════════════════════════════════════════════════╗
║                    📋 TASK EXECUTION MODE - CONTEXT INJECTION                 ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Focus on the current task specification below                                ║
║  Follow TDD: write failing test first, then implement, then verify            ║
╚══════════════════════════════════════════════════════════════════════════════╝
"#
            }
        };

        self.parts.push(header.to_string());
        Ok(())
    }

    pub fn with_memory(mut self) -> Result<Self> {
        if let Some(memory) = Memory::try_load(&self.project_root) {
            self.parts.push(memory.format_context());
        } else {
            self.parts.push(
                r#"
## 🧠 CURRENT STATE
```json
{"status": "NOT_INITIALIZED", "message": "Run initialization first"}
```
"#
                .to_string(),
            );
        }
        Ok(self)
    }

    pub fn with_roadmap(mut self, include_completed: bool) -> Result<Self> {
        if let Some(roadmap) = Roadmap::try_load(&self.project_root) {
            self.parts.push(roadmap.format_context(include_completed));
        } else {
            self.parts
                .push("\n## ❌ ROADMAP NOT FOUND\nInitialize `.claude/status/ROADMAP.md` first!\n".to_string());
        }
        Ok(self)
    }

    pub fn with_contract(mut self) -> Result<Self> {
        if let Some(contract) = ApiContract::try_load(&self.project_root) {
            self.parts.push(contract.format_context(8000));
        }
        Ok(self)
    }

    pub fn with_errors(mut self, task_filter: Option<&str>) -> Result<Self> {
        if let Some(errors) = ErrorHistory::try_load(&self.project_root) {
            let ctx = errors.format_context(task_filter, 15);
            if !ctx.is_empty() {
                self.parts.push(ctx);
            }
        }
        Ok(self)
    }

    pub fn with_structure(mut self, max_depth: usize, max_files: usize) -> Result<Self> {
        let structure = ProjectStructure::scan(&self.project_root, max_depth);
        self.parts.push(structure.format_context(max_files));
        Ok(self)
    }

    fn add_footer(&mut self) -> Result<()> {
        let footer: &str = match self.mode {
            ContextMode::Autonomous => {
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
"#
            }
            ContextMode::Review => {
                r#"
═══════════════════════════════════════════════════════════════════════════════
📌 REVIEW CHECKLIST:
1. Does the code match the API CONTRACT exactly? (signatures, types, returns)
2. Are there comprehensive tests? (happy path + edge cases + error cases)
3. Is error handling complete?
4. Does it follow project conventions?
5. Any security concerns?
═══════════════════════════════════════════════════════════════════════════════
"#
            }
            ContextMode::Task => "",
        };

        if !footer.is_empty() {
            self.parts.push(footer.to_string());
        }
        Ok(())
    }

    pub fn build(mut self) -> Result<String> {
        self.add_header()?;
        self.add_footer()?;
        Ok(self.parts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_context_builder_basic() {
        let current_dir = env::current_dir().unwrap();
        let context = ContextBuilder::new(current_dir)
            .mode(ContextMode::Autonomous)
            .build();

        assert!(context.is_ok());
        let ctx = context.unwrap();
        assert!(ctx.contains("AUTONOMOUS MODE"));
    }
}
