//! 嵌入资源管理
//!
//! 使用 rust-embed 将 agent 定义和模板文件编译进二进制

pub mod agents;
pub mod files;

pub use agents::AgentAssets;
pub use files::TemplateAssets;
