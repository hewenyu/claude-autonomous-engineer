//! 项目管理模块
//!
//! 提供项目根目录查找和初始化功能

pub mod root_finder;
pub mod initializer;

// 重导出
pub use root_finder::find_project_root;
pub use initializer::*;
