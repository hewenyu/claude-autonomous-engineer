// Loop Driver Hook
// Stop - 控制自主循环

use crate::context::Roadmap;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// loop_driver hook
///
/// 在用户尝试停止时检查是否还有待处理任务
pub fn run(project_root: &Path) -> Result<Value> {
    // 尝试加载 ROADMAP
    if let Some(roadmap) = Roadmap::try_load(project_root) {
        // 如果还有待处理任务,阻止停止
        if roadmap.has_pending() {
            let pending_count = roadmap.pending.len() + roadmap.in_progress.len();
            return Ok(json!({
                "decision": "block",
                "reason": format!("[Loop] {} tasks remaining. Continue working!", pending_count)
            }));
        }
    }

    // 允许停止
    Ok(json!({
        "decision": "allow",
        "reason": "All tasks completed or ROADMAP not found"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_loop_driver() {
        let current_dir = env::current_dir().unwrap();
        let result = run(&current_dir);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.get("decision").is_some());
    }
}
