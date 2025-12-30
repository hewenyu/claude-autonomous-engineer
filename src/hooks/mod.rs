//! Hook 模块
//!
//! 实现 4 个 hook：inject_state, progress_sync, codex_review_gate, loop_driver
//! （将在阶段 4 实现）

pub mod runner;

// 重导出
pub use runner::*;
