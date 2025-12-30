// Roadmap Parser
// ROADMAP.md 解析器

use super::types::{Task, TaskStatus};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const ROADMAP_FILE: &str = ".claude/status/ROADMAP.md";

#[derive(Debug, Clone)]
pub struct Roadmap {
    pub raw_content: String,
    pub pending: Vec<Task>,
    pub in_progress: Vec<Task>,
    pub completed: Vec<Task>,
    pub blocked: Vec<Task>,
}

impl Roadmap {
    /// 从文件加载 ROADMAP.md
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(ROADMAP_FILE);

        if !path.exists() {
            anyhow::bail!("ROADMAP.md not found at {}", path.display());
        }

        let raw_content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        Ok(Self::parse(&raw_content))
    }

    /// 解析 ROADMAP 内容
    pub fn parse(content: &str) -> Self {
        let mut pending = Vec::new();
        let mut in_progress = Vec::new();
        let mut completed = Vec::new();
        let mut blocked = Vec::new();

        for line in content.lines() {
            if let Some(task) = Task::parse(line) {
                match task.status {
                    TaskStatus::Pending => pending.push(task),
                    TaskStatus::InProgress => in_progress.push(task),
                    TaskStatus::Completed => completed.push(task),
                    TaskStatus::Blocked => blocked.push(task),
                }
            }
        }

        Roadmap {
            raw_content: content.to_string(),
            pending,
            in_progress,
            completed,
            blocked,
        }
    }

    /// 尝试加载,如果失败则返回 None
    pub fn try_load(project_root: &Path) -> Option<Self> {
        Self::load(project_root).ok()
    }

    /// 获取总任务数
    pub fn total_tasks(&self) -> usize {
        self.pending.len() + self.in_progress.len() + self.completed.len() + self.blocked.len()
    }

    /// 格式化为上下文字符串
    pub fn format_context(&self, include_completed: bool) -> String {
        let mut ctx = String::from("\n## 📋 ROADMAP\n");

        let total = self.total_tasks();
        ctx.push_str(&format!(
            "\n**Progress**: {}/{} tasks completed\n",
            self.completed.len(),
            total
        ));

        // In Progress
        if !self.in_progress.is_empty() {
            ctx.push_str("\n### 🔄 IN PROGRESS\n");
            for task in &self.in_progress {
                ctx.push_str(&format!("{}\n", task.raw_line));
            }
        }

        // Pending
        ctx.push_str("\n### ⏳ PENDING\n");
        let limit = 20;
        for (i, task) in self.pending.iter().enumerate() {
            if i >= limit {
                ctx.push_str(&format!("... and {} more\n", self.pending.len() - limit));
                break;
            }
            ctx.push_str(&format!("{}\n", task.raw_line));
        }

        // Blocked
        if !self.blocked.is_empty() {
            ctx.push_str("\n### ⚠️  BLOCKED\n");
            for task in &self.blocked {
                ctx.push_str(&format!("{}\n", task.raw_line));
            }
        }

        // Completed (if requested)
        if include_completed && !self.completed.is_empty() {
            ctx.push_str("\n### ✅ COMPLETED (Recent)\n");
            let recent = self.completed.len().saturating_sub(5);
            for task in &self.completed[recent..] {
                ctx.push_str(&format!("{}\n", task.raw_line));
            }
        }

        ctx
    }

    /// 是否还有待处理任务
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty() || !self.in_progress.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roadmap_parse() {
        let content = r#"
# Roadmap

## Phase 1
- [ ] Task 1
- [>] Task 2 (in progress)
- [x] Task 3 (done)

## Phase 2
- [ ] Task 4
- [!] Task 5 (blocked)
"#;

        let roadmap = Roadmap::parse(content);

        assert_eq!(roadmap.pending.len(), 2);
        assert_eq!(roadmap.in_progress.len(), 1);
        assert_eq!(roadmap.completed.len(), 1);
        assert_eq!(roadmap.blocked.len(), 1);
        assert_eq!(roadmap.total_tasks(), 5);
    }

    #[test]
    fn test_has_pending() {
        let content = r#"
- [x] Done 1
- [x] Done 2
"#;
        let roadmap = Roadmap::parse(content);
        assert!(!roadmap.has_pending());

        let content2 = r#"
- [x] Done 1
- [ ] Pending 1
"#;
        let roadmap2 = Roadmap::parse(content2);
        assert!(roadmap2.has_pending());
    }
}
