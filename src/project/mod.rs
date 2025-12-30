//! 项目管理模块
//!
//! 提供项目根目录查找和初始化功能

pub mod initializer;
pub mod root_finder;

// 重导出
pub use initializer::*;
pub use root_finder::find_project_root;
