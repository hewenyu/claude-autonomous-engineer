//! Inject State Hook
//!
//! 注入上下文到每次交互（UserPromptSubmit）

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

use crate::context::ContextManager;

/// 运行 inject_state hook
///
/// 从 memory.json, ROADMAP.md 等文件读取状态，组装成完整上下文
pub fn run_inject_state_hook(project_root: &Path) -> Result<Value> {
    let ctx = ContextManager::new(project_root.to_path_buf());
    let full_context = ctx.get_full_context()?;

    Ok(json!({
        "hookSpecificOutput": {
            "for PreToolUse": {
                "hookEventName": "PreToolUse"
            },
            "for UserPromptSubmit": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": full_context
            },
            "for PostToolUse": {
                "hookEventName": "PostToolUse"
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_inject_state_hook() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".claude/status")).unwrap();

        let result = run_inject_state_hook(temp.path()).unwrap();
        assert!(result["hookSpecificOutput"]["for UserPromptSubmit"]["additionalContext"].is_string());

        let context = result["hookSpecificOutput"]["for UserPromptSubmit"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(context.contains("AUTONOMOUS MODE"));
    }
}
