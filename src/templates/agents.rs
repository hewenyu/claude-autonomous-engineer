// Agent Templates
// 嵌入的 Agent markdown 文件

use anyhow::Result;
use std::fs;
use std::path::Path;

// 嵌入 agent 模板文件
pub const PROJECT_ARCHITECT: &str =
    include_str!("../../templates/agents/project-architect-supervisor.md");
pub const CODE_EXECUTOR: &str = include_str!("../../templates/agents/code-executor.md");
pub const CODEX_REVIEWER: &str = include_str!("../../templates/agents/codex-reviewer.md");
pub const PRD_GENERATOR: &str = include_str!("../../templates/agents/prd-generator.md");
pub const VISUAL_DESIGNER: &str = include_str!("../../templates/agents/visual-designer.md");

/// 写入所有 agent 模板到指定目录
pub fn write_all_agents(agents_dir: &Path) -> Result<()> {
    // 确保目录存在
    fs::create_dir_all(agents_dir)?;

    // 写入每个 agent 文件
    fs::write(
        agents_dir.join("project-architect-supervisor.md"),
        PROJECT_ARCHITECT,
    )?;
    fs::write(agents_dir.join("code-executor.md"), CODE_EXECUTOR)?;
    fs::write(agents_dir.join("codex-reviewer.md"), CODEX_REVIEWER)?;
    fs::write(agents_dir.join("prd-generator.md"), PRD_GENERATOR)?;
    fs::write(agents_dir.join("visual-designer.md"), VISUAL_DESIGNER)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_templates_embedded() {
        // 验证模板内容不为空
        assert!(!PROJECT_ARCHITECT.is_empty());
        assert!(!CODE_EXECUTOR.is_empty());
        assert!(!CODEX_REVIEWER.is_empty());
        assert!(!PRD_GENERATOR.is_empty());
        assert!(!VISUAL_DESIGNER.is_empty());
    }
}
