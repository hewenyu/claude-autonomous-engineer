// Codex Review Gate Hook
// PreToolUse - 提交前代码审查

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// codex_review_gate hook
///
/// 拦截 git commit/push 命令,执行代码审查
///
/// TODO: 实现完整的审查逻辑
/// - 获取 staged files
/// - 构建审查上下文 (API contract + task spec + changed files)
/// - 调用 Codex API
/// - 解析审查结果
pub fn run(_project_root: &Path) -> Result<Value> {
    // 当前简化实现: 始终允许
    // 完整实现需要:
    // 1. 解析 stdin 获取 Bash 命令
    // 2. 检测是否是 git commit/push
    // 3. 如果是,执行审查流程
    // 4. 返回 allow/block 决策

    Ok(json!({
        "decision": "allow"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_codex_review() {
        let current_dir = env::current_dir().unwrap();
        let result = run(&current_dir);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.get("decision").and_then(|v| v.as_str()), Some("allow"));
    }
}
