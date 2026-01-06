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

    println!(
        "{}",
        "🚀 Initializing Claude Autonomous project...".cyan().bold()
    );

    // 1. 创建目录结构
    create_directory_structure(project_root)?;

    // 2. 写入 Agent 定义
    write_agent_files(project_root)?;

    // 3. 写入模板文件
    write_template_files(project_root, name)?;

    // 4. 创建状态文件
    create_state_files(project_root)?;

    println!(
        "{}",
        "\n✅ Project initialized successfully!".green().bold()
    );
    println!("\nNext steps:");
    println!(
        "  1. Review {} for hook configuration",
        ".claude/settings.json".cyan()
    );
    println!(
        "  2. Edit {} to define your project roadmap",
        ".claude/status/ROADMAP.md".cyan()
    );
    println!(
        "  3. Start Claude Code - {} hook will auto-inject the protocol",
        "claude_protocol".cyan()
    );

    Ok(())
}

/// 创建目录结构
fn create_directory_structure(project_root: &Path) -> Result<()> {
    let dirs = vec![
        ".claude",
        ".claude/agents",
        ".claude/status",
        ".claude/phases",
        ".claude/stories", // 新增：stories 目录
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

## Overview
This project will be planned phase-by-phase.
Run `project-architect-supervisor` to plan Phase 1 when ready.

## Phases

| Phase | Name | Status | Tasks |
|-------|------|--------|-------|
| 1 | TBD | Pending | - |
| 2 | TBD | Pending | - |
| 3 | TBD | Pending | - |

## Current: Phase 1

*Phase 1 tasks will be added by project-architect-supervisor when you plan the first phase.*

<!--
IMPORTANT: This ROADMAP follows a phase-by-phase planning approach.
- Do NOT plan all phases upfront
- Run project-architect-supervisor to plan ONE phase at a time
- When a phase completes, the system will automatically prompt to plan the next phase

Task Status Markers:
- `[ ]` - Pending
- `[>]` - In Progress
- `[x]` - Completed
- `[!]` - Blocked (requires intervention; blocks overall completion)
- `[-]` - Skipped (explicitly skipped; does not block overall completion)
-->
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

    // requirements.md 模板（可选但推荐）
    let requirements_template = r#"# Requirements

Describe the original user request / PRD here.

- Goals:
- Non-goals:
- Constraints:
- Acceptance criteria:
"#;
    let requirements_path = project_root.join(".claude/status/requirements.md");
    fs::write(&requirements_path, requirements_template)?;
    println!("  ✓ {}", ".claude/status/requirements.md".cyan());

    // error_history.json 初始化为空数组
    let error_history_path = project_root.join(".claude/status/error_history.json");
    fs::write(&error_history_path, "[]")?;
    println!("  ✓ {}", ".claude/status/error_history.json".cyan());

    // decisions.log 初始化为空
    let decisions_path = project_root.join(".claude/status/decisions.log");
    fs::write(&decisions_path, "")?;
    println!("  ✓ {}", ".claude/status/decisions.log".cyan());

    // stories/INDEX.md 模板（新增）
    let index_template = r#"# 📖 User Stories Index

**项目**: 待定
**创建时间**: 待定
**总计Stories**: 0

## 📊 确认状态总览

┌────────────────────────────────────────────────────────────────────┐
│  Story Confirmation Status                                         │
├────────────────────────────────────────────────────────────────────┤
│  [ ] Draft      ● Not yet reviewed                                 │
│  [~] Reviewing  ● Under user review                                │
│  [✓] Confirmed  ● Approved - ready for architecture planning       │
│  [x] Archived   ● No longer needed                                 │
└────────────────────────────────────────────────────────────────────┘

Progress: 0/0 Confirmed (0%)
├── Confirmed: 0
├── Reviewing: 0
├── Draft: 0
└── Archived: 0

---

## 🎯 Stories List

### Phase 1: 待规划

*使用 story-generator agent 来创建业务场景*

示例:
```
你: "我想要一个用户认证系统"
Claude: [调用 story-generator] → 生成 STORY-001, STORY-002, STORY-003...
```

---

## ✅ 如何确认Stories

### 步骤：

1. **生成Stories** - 使用 story-generator agent
2. **阅读每个Story文件** - 点击上面表格中的链接
3. **确认业务理解** - 检查场景、验收标准是否符合预期
4. **修改Story** - 如有问题，直接编辑对应的 STORY-xxx.md 文件
5. **更新状态** - 在上表中将状态从 `[ ]` 改为 `[✓]`

### 状态标记说明：

```markdown
# 确认 STORY-001
| [STORY-001](STORY-001_user_login.md) | [ ] | High | 用户登录功能 | High |
                                        ↓
| [STORY-001](STORY-001_user_login.md) | [✓] | High | 用户登录功能 | High |
```

---

## 🚀 下一步

当所有必要的Stories都标记为 `[✓]` 后：

告诉Claude："Stories已确认，开始技术规划"

系统将自动:
1. 调用 project-architect-supervisor
2. 根据确认的Stories生成:
   - ROADMAP.md (技术任务)
   - api_contract.yaml (接口契约)
   - PHASE_PLAN.md (阶段计划)

---

## 📝 状态说明

- **[ ] Draft**: 刚生成，等待用户首次审查
- **[~] Reviewing**: 用户正在审查中
- **[✓] Confirmed**: 用户已确认，可用于技术规划
- **[x] Archived**: 已归档，不会用于后续开发

**⚠️ 重要**: 只有 `[✓] Confirmed` 状态的Stories才会被 project-architect-supervisor 使用！
"#;
    let index_path = project_root.join(".claude/stories/INDEX.md");
    fs::write(&index_path, index_template)?;
    println!("  ✓ {}", ".claude/stories/INDEX.md".cyan());

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
        assert!(temp.path().join(".claude/settings.json").exists());
        assert!(temp.path().join(".claude/status/memory.json").exists());
        assert!(temp.path().join(".claude/status/ROADMAP.md").exists());
        assert!(temp
            .path()
            .join(".claude/status/api_contract.yaml")
            .exists());
        assert!(temp.path().join(".claude/status/requirements.md").exists());

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
