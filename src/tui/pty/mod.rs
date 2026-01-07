//! PTY 进程管理模块
//!
//! 使用 portable-pty 管理伪终端进程

mod manager;

pub use manager::PtyManager;
