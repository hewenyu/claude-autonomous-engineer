//! 项目初始化
//!
//! 创建 .claude 目录结构和初始文件

use anyhow::{anyhow, Result};
use colored::*;
use std::fs;
use std::path::Path;

use crate::templates::{AgentAssets, TemplateAssets};

/// 初始化项目
///
/// 创建完整的 .claude 目录结构并写入嵌入的资源
pub fn init_project(project_root: &Path, name: Option<&str>, force: bool) -> Result<()> {
    let claude_dir = project_root.join(".claude");

    // 检查是否已存在
    if claude_dir.exists() && !force {
        return Err(anyhow!(
            ".claude directory already exists. Use --force to overwrite."
        ));
    }

    println!("{}", "🚀 Initializing Claude Autonomous project...".cyan().bold());

    // 1. 创建目录结构
    create_directory_structure(project_root)?;

    // 2. 写入 Agent 定义
    write_agent_files(project_root)?;

    // 3. 写入模板文件
    write_template_files(project_root, name)?;

    // 4. 创建状态文件
    create_state_files(project_root)?;

    println!("{}", "\n✅ Project initialized successfully!".green().bold());
    println!("\nNext steps:");
    println!("  1. Edit {} to add project instructions", ".claude/CLAUDE.md".cyan());
    println!("  2. Review {} for hook configuration", ".claude/settings.json".cyan());
    println!("  3. Create {} to define your project roadmap", ".claude/status/ROADMAP.md".cyan());

    Ok(())
}

/// 创建目录结构
fn create_directory_structure(project_root: &Path) -> Result<()> {
    let dirs = vec![
        ".claude",
        ".claude/agents",
        ".claude/status",
        ".claude/phases",
    ];

    for dir in dirs {
        let path = project_root.join(dir);
        fs::create_dir_all(&path)?;
        println!("  📁 Created {}", dir.cyan());
    }

    Ok(())
}

/// 写入 Agent 文件
fn write_agent_files(project_root: &Path) -> Result<()> {
    let agents_dir = project_root.join(".claude/agents");
    let agent_names = AgentAssets::list_agents();

    println!("\n📦 Installing {} agents...", agent_names.len());

    for agent_name in agent_names {
        let content = AgentAssets::get_agent(&agent_name)?;
        let filename = format!("{}.md", agent_name);
        let file_path = agents_dir.join(&filename);

        fs::write(&file_path, content)?;
        println!("  ✓ {}", filename.cyan());
    }

    Ok(())
}

/// 写入模板文件
fn write_template_files(project_root: &Path, name: Option<&str>) -> Result<()> {
    println!("\n📝 Writing configuration files...");

    // CLAUDE.md (项目根目录)
    let claude_md = TemplateAssets::get_claude_md()?;
    let claude_path = project_root.join("CLAUDE.md");
    fs::write(&claude_path, claude_md)?;
    println!("  ✓ {}", "CLAUDE.md".cyan());

    // settings.json
    let mut settings_json = TemplateAssets::get_settings_json()?;

    // 替换项目名称占位符（如果提供）
    if let Some(project_name) = name {
        settings_json = settings_json.replace("\"My Project\"", &format!("\"{}\"", project_name));
    }

    let settings_path = project_root.join(".claude/settings.json");
    fs::write(&settings_path, settings_json)?;
    println!("  ✓ {}", ".claude/settings.json".cyan());

    // memory.json
    let mut memory_json = TemplateAssets::get_memory_json()?;

    // 替换项目名称占位符（如果提供）
    if let Some(project_name) = name {
        memory_json = memory_json.replace(
            "\"project\": \"unknown\"",
            &format!("\"project\": \"{}\"", project_name),
        );
    }

    let memory_path = project_root.join(".claude/status/memory.json");
    fs::write(&memory_path, memory_json)?;
    println!("  ✓ {}", ".claude/status/memory.json".cyan());

    Ok(())
}

/// 创建状态文件模板
fn create_state_files(project_root: &Path) -> Result<()> {
    println!("\n📋 Creating state file templates...");

    // ROADMAP.md 模板
    let roadmap_template = r#"# Project Roadmap

## Current: Phase 1

## Task List

- [ ] TASK-001: Define project requirements
- [ ] TASK-002: Setup project structure
- [ ] TASK-003: Implement core functionality

## Notes

Update this file to reflect your project's actual roadmap.
Use the following status markers:
- `[ ]` - Pending
- `[>]` - In Progress
- `[x]` - Completed
- `[!]` - Blocked
"#;
    let roadmap_path = project_root.join(".claude/status/ROADMAP.md");
    fs::write(&roadmap_path, roadmap_template)?;
    println!("  ✓ {}", ".claude/status/ROADMAP.md".cyan());

    // api_contract.yaml 模板
    let contract_template = r#"# API Contract

version: "1.0"

modules:
  - name: example_module
    functions:
      - name: example_function
        signature: "fn example_function(arg: String) -> Result<String>"
        description: "Example function description"
        tests:
          - "test_example_function_success"
          - "test_example_function_error"

# Update this file with your actual API contract
"#;
    let contract_path = project_root.join(".claude/status/api_contract.yaml");
    fs::write(&contract_path, contract_template)?;
    println!("  ✓ {}", ".claude/status/api_contract.yaml".cyan());

    // error_history.json 初始化为空数组
    let error_history_path = project_root.join(".claude/status/error_history.json");
    fs::write(&error_history_path, "[]")?;
    println!("  ✓ {}", ".claude/status/error_history.json".cyan());

    // decisions.log 初始化为空
    let decisions_path = project_root.join(".claude/status/decisions.log");
    fs::write(&decisions_path, "")?;
    println!("  ✓ {}", ".claude/status/decisions.log".cyan());

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// 测试
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_project() {
        let temp = TempDir::new().unwrap();
        let result = init_project(temp.path(), Some("test-project"), false);
        assert!(result.is_ok());

        // 验证目录结构
        assert!(temp.path().join(".claude").exists());
        assert!(temp.path().join(".claude/agents").exists());
        assert!(temp.path().join(".claude/status").exists());
        assert!(temp.path().join(".claude/phases").exists());

        // 验证文件
        assert!(temp.path().join("CLAUDE.md").exists());
        assert!(temp.path().join(".claude/settings.json").exists());
        assert!(temp.path().join(".claude/status/memory.json").exists());
        assert!(temp.path().join(".claude/status/ROADMAP.md").exists());
        assert!(temp.path().join(".claude/status/api_contract.yaml").exists());

        // 验证 agent 文件
        let agents_dir = temp.path().join(".claude/agents");
        assert!(agents_dir.join("project-architect-supervisor.md").exists());
        assert!(agents_dir.join("codex-reviewer.md").exists());
    }

    #[test]
    fn test_init_project_already_exists() {
        let temp = TempDir::new().unwrap();

        // 第一次初始化
        init_project(temp.path(), None, false).unwrap();

        // 第二次初始化应该失败
        let result = init_project(temp.path(), None, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_init_project_force() {
        let temp = TempDir::new().unwrap();

        // 第一次初始化
        init_project(temp.path(), None, false).unwrap();

        // 使用 force 再次初始化应该成功
        let result = init_project(temp.path(), Some("forced-project"), true);
        assert!(result.is_ok());
    }
}

