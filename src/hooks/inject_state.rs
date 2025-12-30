// Inject State Hook
// UserPromptSubmit - 注入完整上下文

use crate::context::{ContextBuilder, ContextMode};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// inject_state hook
///
/// 在每次用户提交 prompt 时自动注入完整上下文
pub fn run(project_root: &Path) -> Result<Value> {
    // 构建完整上下文
    let context = ContextBuilder::new(project_root.to_path_buf())
        .mode(ContextMode::Autonomous)
        .with_memory()?
        .with_roadmap(false)?  // 不包含已完成任务
        .with_contract()?
        .with_errors(None)?    // 所有错误
        .with_structure(3, 50)?  // 最多 3 层深度, 50 个文件
        .build()?;

    // 返回 Claude Code 期望的格式
    Ok(json!({
        "hookSpecificOutput": {
            "additionalContext": context
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_inject_state() {
        let current_dir = env::current_dir().unwrap();
        let result = run(&current_dir);

        // 如果 .claude 目录存在,应该能成功执行
        if current_dir.join(".claude").exists() {
            assert!(result.is_ok());
            let output = result.unwrap();
            assert!(output.get("hookSpecificOutput").is_some());
        }
    }
}
