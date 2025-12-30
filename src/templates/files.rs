//! 模板文件嵌入
//!
//! 嵌入项目初始化所需的模板文件

use anyhow::{anyhow, Result};
use rust_embed::RustEmbed;

/// 模板文件资源（编译时嵌入）
#[derive(RustEmbed)]
#[folder = "embedded/templates/"]
pub struct TemplateAssets;

impl TemplateAssets {
    /// 获取 CLAUDE.md 模板
    pub fn get_claude_md() -> Result<String> {
        Self::get_file("CLAUDE.md")
    }

    /// 获取 settings.json 模板
    pub fn get_settings_json() -> Result<String> {
        Self::get_file("settings.json")
    }

    /// 获取 memory.json 模板
    pub fn get_memory_json() -> Result<String> {
        Self::get_file("memory.json")
    }

    /// 获取指定模板文件
    fn get_file(filename: &str) -> Result<String> {
        let file = Self::get(filename)
            .ok_or_else(|| anyhow!("Template '{}' not found", filename))?;

        let content = std::str::from_utf8(file.data.as_ref())
            .map_err(|e| anyhow!("Failed to decode template '{}': {}", filename, e))?;

        Ok(content.to_string())
    }

    /// 列出所有可用的模板文件
    pub fn list_templates() -> Vec<String> {
        Self::iter()
            .map(|path| path.as_ref().to_string())
            .collect()
    }

    /// 检查模板文件是否存在
    pub fn template_exists(filename: &str) -> bool {
        Self::get(filename).is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_templates() {
        let templates = TemplateAssets::list_templates();
        assert!(!templates.is_empty());
        assert!(templates.contains(&"CLAUDE.md".to_string()));
        assert!(templates.contains(&"settings.json".to_string()));
        assert!(templates.contains(&"memory.json".to_string()));
    }

    #[test]
    fn test_get_claude_md() {
        let content = TemplateAssets::get_claude_md().unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_get_settings_json() {
        let content = TemplateAssets::get_settings_json().unwrap();
        assert!(!content.is_empty());
        // 验证是 JSON 格式
        let _json: serde_json::Value = serde_json::from_str(&content).unwrap();
    }

    #[test]
    fn test_get_memory_json() {
        let content = TemplateAssets::get_memory_json().unwrap();
        assert!(!content.is_empty());
        // 验证是 JSON 格式
        let _json: serde_json::Value = serde_json::from_str(&content).unwrap();
    }

    #[test]
    fn test_template_exists() {
        assert!(TemplateAssets::template_exists("CLAUDE.md"));
        assert!(TemplateAssets::template_exists("settings.json"));
        assert!(TemplateAssets::template_exists("memory.json"));
        assert!(!TemplateAssets::template_exists("non-existent.txt"));
    }
}
