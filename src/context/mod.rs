// Context Management Module
// 上下文管理器 - 移植自 .claude/lib/context_manager.py

pub mod types;
pub mod memory;
pub mod roadmap;
pub mod builder;
pub mod contract;
pub mod errors;
pub mod structure;

// 重新导出主要接口
pub use builder::{ContextBuilder, ContextMode};
pub use types::{Memory, TaskInfo, Progress, NextAction, WorkingContext};  // 修复: 从 types 导出
pub use roadmap::Roadmap;
