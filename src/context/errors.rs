// Error History Handler
// error_history.json 处理

use super::types::ErrorRecord;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const ERROR_HISTORY_FILE: &str = ".claude/status/error_history.json";

pub struct ErrorHistory {
    pub errors: Vec<ErrorRecord>,
}

impl ErrorHistory {
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(ERROR_HISTORY_FILE);

        if !path.exists() {
            return Ok(Self { errors: vec![] });
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let errors: Vec<ErrorRecord> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Self { errors })
    }

    pub fn try_load(project_root: &Path) -> Option<Self> {
        Self::load(project_root).ok()
    }

    pub fn format_context(&self, task_filter: Option<&str>, max_count: usize) -> String {
        if self.errors.is_empty() {
            return String::new();
        }

        // 过滤相关错误
        let relevant: Vec<&ErrorRecord> = if let Some(task) = task_filter {
            self.errors.iter().filter(|e| e.task == task).collect()
        } else {
            self.errors.iter().rev().take(max_count).collect()
        };

        if relevant.is_empty() {
            return String::new();
        }

        let mut ctx = String::from("\n## ⚠️ ERROR HISTORY (MUST AVOID REPEATING)\n");

        // 未解决的错误
        let unresolved: Vec<&&ErrorRecord> =
            relevant.iter().filter(|e| e.resolution.is_none()).collect();
        if !unresolved.is_empty() {
            ctx.push_str("\n### ❌ Unresolved Errors\n");
            for err in unresolved.iter().take(5) {
                let error_preview = truncate(&err.error, 200);
                let attempted = err
                    .attempted_fix
                    .as_ref()
                    .map(|s| truncate(s, 100))
                    .unwrap_or_else(|| "N/A".to_string());

                ctx.push_str(&format!(
                    r#"
**Task**: {}
**Error**: {}
**Attempted**: {}
---
"#,
                    err.task, error_preview, attempted
                ));
            }
        }

        // 已解决的错误 (学习)
        let resolved: Vec<&&ErrorRecord> = relevant.iter().filter(|e| e.resolution.is_some()).collect();
        if !resolved.is_empty() {
            ctx.push_str("\n### ✅ Resolved (Learn from these)\n");
            for err in resolved.iter().take(3) {
                let error_preview = truncate(&err.error, 100);
                let solution = err
                    .resolution
                    .as_ref()
                    .map(|s| truncate(s, 150))
                    .unwrap_or_else(|| "N/A".to_string());

                ctx.push_str(&format!(
                    r#"
**Task**: {}
**Error**: {}
**Solution**: {}
---
"#,
                    err.task, error_preview, solution
                ));
            }
        }

        ctx
    }
}

fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}
