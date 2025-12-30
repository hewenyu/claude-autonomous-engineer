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
    match Roadmap::try_load(project_root) {
        Some(roadmap) => {
            // 如果还有待处理任务,阻止停止
            if roadmap.has_pending() {
                let pending_count = roadmap.pending.len() + roadmap.in_progress.len();

                return Ok(json!({
                    "decision": "block",
                    "reason": format!(
                        "❌ CANNOT STOP - {} tasks remaining\n\n\
                        📋 Pending Tasks:\n\
                        - In Progress: {}\n\
                        - Pending: {}\n\n\
                        ⚠️  You must complete all tasks before stopping.\n\
                        💡 Continue working on the current task or mark tasks as blocked [!] if stuck.",
                        pending_count,
                        roadmap.in_progress.len(),
                        roadmap.pending.len()
                    )
                }));
            }

            // 所有任务完成,允许停止
            Ok(json!({
                "decision": "allow",
                "reason": format!(
                    "✅ All tasks completed!\n\n\
                    📊 Summary:\n\
                    - Total tasks: {}\n\
                    - Completed: {}\n\
                    - Blocked: {}\n\n\
                    🎉 Great work! The autonomous loop can now be stopped.",
                    roadmap.total_tasks(),
                    roadmap.completed.len(),
                    roadmap.blocked.len()
                )
            }))
        }
        None => {
            // ROADMAP 不存在,阻止停止并提供指导
            Ok(json!({
                "decision": "block",
                "reason":
                    "❌ ROADMAP NOT FOUND\n\n\
                    Cannot run autonomous loop without a roadmap.\n\n\
                    Action Required:\n\
                    1. Use project-architect-supervisor to create:\n\
                       - .claude/status/ROADMAP.md\n\
                       - .claude/status/api_contract.yaml\n\
                       - .claude/status/memory.json\n\n\
                    2. Or create manually following the template."
            }))
        }
    }
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
