// Hooks Module
// Hook 实现 - 移植自 .claude/hooks/*.py

pub mod inject_state;
pub mod loop_driver;
pub mod progress_sync;
pub mod codex_review;

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub struct HookRunner;

impl HookRunner {
    /// 运行指定的 hook
    pub fn run(hook_name: &str, project_root: &Path, _stdin: Option<&str>) -> Result<Value> {
        match hook_name {
            "inject_state" => inject_state::run(project_root),
            "loop_driver" => loop_driver::run(project_root),
            "progress_sync" => progress_sync::run(project_root),
            "codex_review_gate" => codex_review::run(project_root),
            _ => {
                anyhow::bail!("Unknown hook: {}", hook_name)
            }
        }
    }
}
