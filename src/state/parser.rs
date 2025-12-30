//! 状态文件解析器
//!
//! 解析 Markdown, YAML, JSON（将在阶段 2 实现）

use anyhow::Result;
use super::RoadmapData;

/// 解析 ROADMAP.md（占位符）
pub fn parse_roadmap(_content: &str) -> Result<RoadmapData> {
    Ok(RoadmapData::default())
}
