// Progress Sync Hook
// PostToolUse - 自动同步 Markdown 进度到 memory.json

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// progress_sync hook
///
/// 检测 ROADMAP.md 或 TASK-xxx.md 的修改,自动同步到 memory.json
///
/// TODO: 实现完整的同步逻辑
/// - 监听文件修改事件
/// - 解析任务状态变化
/// - 更新 memory.json
pub fn run(_project_root: &Path) -> Result<Value> {
    // 当前简化实现: 只返回成功
    // 完整实现需要:
    // 1. 检测修改的文件 (ROADMAP.md / TASK-xxx.md)
    // 2. 解析任务状态
    // 3. 更新 memory.json

    Ok(json!({
        "status": "ok"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_progress_sync() {
        let current_dir = env::current_dir().unwrap();
        let result = run(&current_dir);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.get("status").and_then(|v| v.as_str()), Some("ok"));
    }
}
