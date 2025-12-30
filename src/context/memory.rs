// Memory Management
// memory.json 读写操作

use super::types::Memory;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const MEMORY_FILE: &str = ".claude/status/memory.json";

impl Memory {
    /// 从文件加载 memory.json
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(MEMORY_FILE);

        if !path.exists() {
            anyhow::bail!("memory.json not found at {}", path.display());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))
    }

    /// 保存到文件
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let path = project_root.join(MEMORY_FILE);

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        Ok(())
    }

    /// 尝试加载,如果失败则返回 None
    pub fn try_load(project_root: &Path) -> Option<Self> {
        Self::load(project_root).ok()
    }

    /// 格式化为上下文字符串
    pub fn format_context(&self) -> String {
        let mut ctx = String::from("\n## 🧠 CURRENT STATE\n");

        // 当前任务
        if let Some(ref task) = self.current_task {
            ctx.push_str(&format!(
                r#"
### Current Task
- **ID**: {}
- **Name**: {}
- **Status**: {}
- **Retry Count**: {}/{}
"#,
                task.id, task.name, task.status, task.retry_count, task.max_retries
            ));
        }

        // 工作上下文
        if let Some(ref current_file) = self.working_context.current_file {
            ctx.push_str("\n### Working Context\n");
            ctx.push_str(&format!("- **Current File**: `{}`\n", current_file));

            if let Some(ref func) = self.working_context.current_function {
                ctx.push_str(&format!("- **Current Function**: `{}`\n", func));
            }

            if !self.working_context.pending_tests.is_empty() {
                let tests: Vec<&str> = self.working_context.pending_tests.iter().take(5).map(|s| s.as_str()).collect();
                ctx.push_str(&format!("- **Pending Tests**: {}\n", tests.join(", ")));
            }

            if !self.working_context.pending_implementations.is_empty() {
                let impls: Vec<&str> = self
                    .working_context
                    .pending_implementations
                    .iter()
                    .take(5)
                    .map(|s| s.as_str())
                    .collect();
                ctx.push_str(&format!("- **Pending Impl**: {}\n", impls.join(", ")));
            }
        }

        // 下一步行动
        ctx.push_str(&format!(
            r#"
### Next Action
- **Action**: {}
- **Target**: {}
- **Reason**: {}
"#,
            self.next_action.action, self.next_action.target, self.next_action.reason
        ));

        // 进度
        if self.progress.tasks_total > 0 {
            let pct = (self.progress.tasks_completed as f64 / self.progress.tasks_total as f64)
                * 100.0;
            ctx.push_str(&format!(
                r#"
### Progress
- **Tasks**: {}/{} ({:.1}%)
- **Current Phase**: {}
"#,
                self.progress.tasks_completed,
                self.progress.tasks_total,
                pct,
                self.progress
                    .current_phase
                    .as_deref()
                    .unwrap_or("N/A")
            ));
        }

        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::types::{NextAction, Progress, WorkingContext};

    #[test]
    fn test_memory_serialization() {
        let memory = Memory {
            project: "test-project".to_string(),
            version: "1.0.0".to_string(),
            mode: "autonomous".to_string(),
            current_phase: None,
            current_task: None,
            progress: Progress::default(),
            next_action: NextAction::default(),
            error_history: vec![],
            decisions_log: vec![],
            active_files: vec![],
            working_context: WorkingContext::default(),
        };

        let json = serde_json::to_string_pretty(&memory).unwrap();
        let deserialized: Memory = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.project, "test-project");
        assert_eq!(deserialized.version, "1.0.0");
    }
}
