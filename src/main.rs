use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use serde_json::json;
use std::fs;
use std::io::{self, Read};

// 使用新的模块化结构
use claude_autonomous::{find_project_root, HookRunner};

/// Claude Autonomous Engineering CLI
///
/// 零 Python 依赖的自主工程系统
#[derive(Parser)]
#[command(name = "claude-autonomous")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化 .claude 目录
    Init {
        /// 项目名称
        #[arg(short, long)]
        name: Option<String>,

        /// 强制覆盖已存在的配置
        #[arg(short, long)]
        force: bool,
    },

    /// 运行 hook
    Hook {
        /// Hook 名称: inject_state, codex_review_gate, progress_sync, loop_driver
        name: String,
    },

    /// 显示项目根目录
    Root,

    /// 显示当前状态
    Status,

    /// 生成简化的 settings.json
    GenSettings,
}

// ═══════════════════════════════════════════════════════════════════
// Hook 执行
// ═══════════════════════════════════════════════════════════════════

fn run_hook(hook_name: &str) -> Result<()> {
    let project_root = match find_project_root() {
        Some(root) => root,
        None => {
            // 返回适当的默认响应
            print_graceful_response(hook_name);
            return Ok(());
        }
    };

    // 读取 stdin
    let mut stdin_data = String::new();
    io::stdin().read_to_string(&mut stdin_data).ok();

    // 执行 hook (使用 Rust 实现)
    let result = HookRunner::run(
        hook_name,
        &project_root,
        if stdin_data.is_empty() {
            None
        } else {
            Some(&stdin_data)
        },
    );

    match result {
        Ok(output) => {
            println!("{}", serde_json::to_string(&output)?);
            Ok(())
        }
        Err(e) => {
            eprintln!("Hook error: {}", e);
            print_graceful_response(hook_name);
            Ok(())
        }
    }
}

fn print_graceful_response(hook_name: &str) {
    let response = match hook_name {
        "inject_state" => json!({
            "hookSpecificOutput": {
                "additionalContext": ""
            }
        }),
        "codex_review_gate" => json!({
            "decision": "allow"
        }),
        "progress_sync" => json!({
            "status": "ok"
        }),
        "loop_driver" => json!({
            "decision": "allow",
            "reason": "[CLI] .claude directory not found"
        }),
        _ => json!({}),
    };
    println!("{}", serde_json::to_string(&response).unwrap());
}

// ═══════════════════════════════════════════════════════════════════
// 初始化
// ═══════════════════════════════════════════════════════════════════

fn init_project(name: Option<String>, force: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let claude_dir = cwd.join(".claude");

    if claude_dir.exists() && !force {
        eprintln!(
            "{}",
            "⚠️  .claude directory already exists. Use --force to overwrite.".yellow()
        );
        return Ok(());
    }

    let project_name = name.unwrap_or_else(|| {
        cwd.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "my-project".to_string())
    });

    println!(
        "{}",
        format!(
            "🚀 Initializing Claude Autonomous Engineering for: {}",
            project_name
        )
        .cyan()
    );

    // 创建目录结构
    let dirs = [
        ".claude/hooks",
        ".claude/lib",
        ".claude/status",
        ".claude/phases",
        ".claude/agents",
    ];

    for dir in dirs {
        fs::create_dir_all(cwd.join(dir))?;
        println!("  {} Created {}", "✓".green(), dir);
    }

    // 创建 settings.json
    let settings = create_settings_json();
    fs::write(claude_dir.join("settings.json"), settings)?;
    println!("  {} Created .claude/settings.json", "✓".green());

    // 创建 memory.json
    let memory = create_memory_json(&project_name);
    fs::write(claude_dir.join("status/memory.json"), memory)?;
    println!("  {} Created .claude/status/memory.json", "✓".green());

    // 创建 CLAUDE.md
    let claude_md = create_claude_md(&project_name);
    fs::write(claude_dir.join("CLAUDE.md"), claude_md)?;
    println!("  {} Created .claude/CLAUDE.md", "✓".green());

    // 写入 agent 模板 (从嵌入的内容)
    let agents_dir = claude_dir.join("agents");
    claude_autonomous::templates::write_all_agents(&agents_dir)?;
    println!("  {} Created agent templates", "✓".green());

    println!();
    println!("{}", "✅ Initialization complete!".green().bold());
    println!();
    println!("Next steps:");
    println!("  1. Review and customize .claude/CLAUDE.md");
    println!("  2. Start Claude Code in this directory");
    println!("  3. Say: \"Plan the project: [your description]\"");

    Ok(())
}

fn create_settings_json() -> String {
    let settings = json!({
        "_comment": "Claude Autonomous Engineering - Hook configuration (Rust binary)",
        "hooks": {
            "UserPromptSubmit": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "claude-autonomous hook inject_state",
                    "timeout": 5000
                }]
            }],
            "PreToolUse": [{
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": "claude-autonomous hook codex_review_gate",
                    "timeout": 180000
                }]
            }],
            "PostToolUse": [{
                "matcher": "Write|Edit|Create",
                "hooks": [{
                    "type": "command",
                    "command": "claude-autonomous hook progress_sync",
                    "timeout": 5000
                }]
            }],
            "Stop": [{
                "matcher": "*",
                "hooks": [{
                    "type": "command",
                    "command": "claude-autonomous hook loop_driver",
                    "timeout": 5000
                }]
            }]
        }
    });
    serde_json::to_string_pretty(&settings).unwrap()
}

fn create_memory_json(project_name: &str) -> String {
    let memory = json!({
        "project": project_name,
        "version": "1.0.0",
        "mode": "autonomous",
        "current_phase": null,
        "current_task": null,
        "progress": {
            "tasks_total": 0,
            "tasks_completed": 0,
            "current_phase": null,
            "completed": [],
            "in_progress": [],
            "blocked": [],
            "pending": []
        },
        "next_action": {
            "action": "INITIALIZE",
            "target": "Run project-architect-supervisor",
            "reason": "System initialized, awaiting project plan"
        },
        "error_history": [],
        "decisions_log": [],
        "active_files": [],
        "working_context": {
            "current_file": null,
            "current_function": null,
            "pending_tests": [],
            "pending_implementations": []
        }
    });
    serde_json::to_string_pretty(&memory).unwrap()
}

fn create_claude_md(project_name: &str) -> String {
    format!(
        r#"# {} - Claude Autonomous Engineering

## 🎯 Project Overview
[Describe your project here]

## 📋 Prime Directives
1. **State Recovery First**: On context restore, ALWAYS read memory.json before any action
2. **No Human Dependency**: Never ask questions that block progress
3. **Auto-Sync**: Progress automatically syncs from ROADMAP.md to memory.json
4. **Quality Gate**: All commits go through Codex review

## 🔄 The Loop
```
READ state → IDENTIFY task → EXECUTE (TDD) → UPDATE state → CONTINUE
```

## 🚫 Anti-Patterns (FORBIDDEN)
- ❌ Asking "should I continue?"
- ❌ Stopping without completing ROADMAP
- ❌ Skipping tests
- ❌ Ignoring error_history

## 📁 Key Files
- `.claude/status/memory.json` - Current state (TRUST THIS)
- `.claude/status/ROADMAP.md` - Task list with status markers
- `.claude/status/api_contract.yaml` - API signatures

## 🛠️ Powered By
**claude-autonomous** (Rust) - Zero Python dependencies, single binary deployment
"#,
        project_name
    )
}

// ═══════════════════════════════════════════════════════════════════
// 状态显示
// ═══════════════════════════════════════════════════════════════════

fn show_status() -> Result<()> {
    let project_root = match find_project_root() {
        Some(root) => root,
        None => {
            println!("{}", "❌ No .claude directory found".red());
            println!("Run 'claude-autonomous init' to initialize");
            return Ok(());
        }
    };

    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║              Claude Autonomous Engineering Status                 ║".cyan()
    );
    println!(
        "{}",
        "╚══════════════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
    println!(
        "📁 Project Root: {}",
        project_root.display().to_string().green()
    );

    // 读取 memory.json
    let memory_file = project_root.join(".claude/status/memory.json");
    if memory_file.exists() {
        let content = fs::read_to_string(&memory_file)?;
        let memory: serde_json::Value = serde_json::from_str(&content)?;

        println!();
        println!("🧠 Current State:");
        if let Some(project) = memory.get("project") {
            println!("   Project: {}", project.as_str().unwrap_or("N/A").yellow());
        }
        if let Some(phase) = memory.get("current_phase") {
            if !phase.is_null() {
                println!("   Phase: {}", phase.as_str().unwrap_or("N/A"));
            }
        }
        if let Some(task) = memory.get("current_task") {
            if !task.is_null() {
                println!("   Task: {}", task.as_str().unwrap_or("N/A"));
            }
        }
    }

    // 读取 ROADMAP.md
    let roadmap_file = project_root.join(".claude/status/ROADMAP.md");
    if roadmap_file.exists() {
        let content = fs::read_to_string(&roadmap_file)?;
        let completed: Vec<_> = content.lines().filter(|l| l.contains("- [x]")).collect();
        let pending: Vec<_> = content.lines().filter(|l| l.contains("- [ ]")).collect();
        let in_progress: Vec<_> = content.lines().filter(|l| l.contains("- [>]")).collect();

        println!();
        println!("📋 Progress:");
        println!("   {} Completed: {}", "✓".green(), completed.len());
        println!("   {} In Progress: {}", "▶".yellow(), in_progress.len());
        println!("   {} Pending: {}", "○".white(), pending.len());
    } else {
        println!();
        println!(
            "{}",
            "⚠️  ROADMAP.md not found - Run planning first".yellow()
        );
    }

    Ok(())
}

fn gen_settings() -> Result<()> {
    let settings = create_settings_json();
    println!("{}", settings);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, force } => init_project(name, force),
        Commands::Hook { name } => run_hook(&name),
        Commands::Root => {
            match find_project_root() {
                Some(root) => println!("{}", root.display()),
                None => {
                    eprintln!("No .claude directory found");
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Commands::Status => show_status(),
        Commands::GenSettings => gen_settings(),
    }
}
