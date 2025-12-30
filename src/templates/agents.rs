//! Agent 资源嵌入
//!
//! 嵌入 5 个 agent markdown 文件

use anyhow::{anyhow, Result};
use rust_embed::RustEmbed;

/// Agent 资源（编译时嵌入）
#[derive(RustEmbed)]
#[folder = "embedded/agents/"]
pub struct AgentAssets;

impl AgentAssets {
    /// 获取指定 agent 的定义内容
    ///
    /// # 参数
    /// - `name`: Agent 名称（不带 .md 扩展名）
    ///
    /// # 示例
    /// ```no_run
    /// use claude_autonomous::templates::AgentAssets;
    /// let content = AgentAssets::get_agent("project-architect-supervisor").unwrap();
    /// ```
    pub fn get_agent(name: &str) -> Result<String> {
        let filename = if name.ends_with(".md") {
            name.to_string()
        } else {
            format!("{}.md", name)
        };

        let file = Self::get(&filename).ok_or_else(|| anyhow!("Agent '{}' not found", name))?;

        let content = std::str::from_utf8(file.data.as_ref())
            .map_err(|e| anyhow!("Failed to decode agent '{}': {}", name, e))?;

        Ok(content.to_string())
    }

    /// 列出所有可用的 agent
    ///
    /// # 返回
    /// 返回所有 agent 名称（不带 .md 扩展名）
    pub fn list_agents() -> Vec<String> {
        Self::iter()
            .filter_map(|path| {
                let path_str = path.as_ref();
                if path_str.ends_with(".md") {
                    Some(path_str.trim_end_matches(".md").to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// 检查 agent 是否存在
    pub fn agent_exists(name: &str) -> bool {
        let filename = if name.ends_with(".md") {
            name.to_string()
        } else {
            format!("{}.md", name)
        };
        Self::get(&filename).is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_agents() {
        let agents = AgentAssets::list_agents();
        assert!(!agents.is_empty());
        assert!(agents.contains(&"project-architect-supervisor".to_string()));
        assert!(agents.contains(&"codex-reviewer".to_string()));
        assert!(agents.contains(&"code-executor".to_string()));
    }

    #[test]
    fn test_get_agent() {
        let content = AgentAssets::get_agent("project-architect-supervisor").unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("Project Architect"));
    }

    #[test]
    fn test_get_agent_with_extension() {
        let content = AgentAssets::get_agent("project-architect-supervisor.md").unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_agent_not_found() {
        let result = AgentAssets::get_agent("non-existent-agent");
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_exists() {
        assert!(AgentAssets::agent_exists("project-architect-supervisor"));
        assert!(AgentAssets::agent_exists("project-architect-supervisor.md"));
        assert!(!AgentAssets::agent_exists("non-existent-agent"));
    }
}
