//! Claude Protocol Hook
//!
//! SessionStart 时自动注入 CLAUDE.md 静态规范

use anyhow::Result;
use serde_json::{json, Value};

/// 运行 claude_protocol hook
///
/// 在每次 session 开始时注入 CLAUDE.md 模板内容
pub fn run_claude_protocol_hook() -> Result<Value> {
    // 从 embedded templates 读取 CLAUDE.md
    let claude_md = include_str!("../../embedded/templates/CLAUDE.md");

    Ok(json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": format!(
                "📋 AUTONOMOUS ENGINEERING PROTOCOL\n\n{}\n\n{}",
                "═".repeat(80),
                claude_md
            )
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_protocol_hook() {
        let result = run_claude_protocol_hook().unwrap();

        // 验证输出格式
        assert_eq!(result["hookSpecificOutput"]["hookEventName"], "SessionStart");
        assert!(result["hookSpecificOutput"]["additionalContext"].is_string());

        // 验证包含关键内容
        let context = result["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(context.contains("Autonomous Engineering Orchestrator Protocol"));
        assert!(context.contains("Prime Directives"));
        assert!(context.contains("Agent Swarm Protocol"));
        assert!(context.contains("The Loop"));
    }

    #[test]
    fn test_claude_protocol_output_format() {
        let result = run_claude_protocol_hook().unwrap();

        // 确保使用正确的扁平格式，而不是嵌套的 "for SessionStart"
        assert!(result["hookSpecificOutput"].is_object());
        assert!(result["hookSpecificOutput"]["for SessionStart"].is_null());
        assert_eq!(
            result["hookSpecificOutput"]["hookEventName"],
            "SessionStart"
        );
    }
}
