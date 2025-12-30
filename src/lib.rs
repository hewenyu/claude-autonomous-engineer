// Claude Autonomous Engineering CLI - Library Root
//
// 模块化架构，零 Python 依赖的自主工程系统

pub mod context;
pub mod hooks;
pub mod templates;
pub mod utils;

// 重新导出常用类型
pub use context::{ContextBuilder, ContextMode};
pub use hooks::HookRunner;
pub use utils::find_project_root;
