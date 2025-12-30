//! Claude Autonomous Engineering Library
//!
//! 纯 Rust 实现的自动化工程系统，提供：
//! - Hook 系统（状态注入、进度同步、代码审查、循环驱动）
//! - 上下文管理（智能组装项目状态）
//! - 状态管理（解析和同步 Markdown/JSON/YAML）
//! - 资源嵌入（Agent 定义和模板）

// 模块声明
pub mod utils;
pub mod project;
pub mod state;
pub mod context;
pub mod hooks;
pub mod templates;
pub mod cli;

// 重导出常用类型
pub use anyhow::{Result, Context as AnyhowContext};
pub use utils::*;
pub use project::find_project_root;
pub use state::Memory;

// 版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
