//! Hook 统一执行器

use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// 运行指定的 hook
pub fn run_hook(hook_name: &str, _project_root: &Path) -> Result<Value> {
    // 占位符实现
    Ok(match hook_name {
        "inject_state" => json!({
            "hookSpecificOutput": {
                "additionalContext": "Hook runner will be implemented in Phase 4"
            }
        }),
        "codex_review_gate" | "pre_write_check" => json!({
            "decision": "allow"
        }),
        "progress_sync" | "post_write_update" => json!({
            "status": "ok"
        }),
        "loop_driver" => json!({
            "decision": "allow",
            "reason": "Hook runner will be implemented in Phase 4"
        }),
        _ => json!({}),
    })
}
