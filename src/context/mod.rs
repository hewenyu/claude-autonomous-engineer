//! 上下文管理模块
//!
//! 智能组装项目状态（将在阶段 3 实现）

pub mod manager;
pub mod truncate;

// 重导出
pub use manager::*;
pub use truncate::*;
