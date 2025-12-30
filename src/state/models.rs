//! 状态数据模型
//!
//! 定义 Memory, RoadmapData, TaskItem 等数据结构

use serde::{Deserialize, Serialize};

/// Memory 数据结构（占位符）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Memory {
    pub project: String,
    pub version: String,
}

/// Roadmap 数据结构（占位符）
#[derive(Debug, Clone, Default)]
pub struct RoadmapData {
    pub pending: Vec<TaskItem>,
    pub in_progress: Vec<TaskItem>,
    pub completed: Vec<TaskItem>,
}

/// 任务项
#[derive(Debug, Clone)]
pub struct TaskItem {
    pub line: String,
    pub id: Option<String>,
}
