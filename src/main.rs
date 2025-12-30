use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::env;

use claude_autonomous::{
    find_project_root,
    project::init_project,
    templates::AgentAssets,
};

/// Claude Autonomous Engineering CLI
///
/// 纯 Rust 实现的自主工程工具 - 零 Python 依赖
#[derive(Parser)]
#[command(name = "claude-autonomous")]
#[command(author, version, about)]
#[command(long_about = "A pure Rust implementation of Claude Autonomous Engineering toolkit.\n\
                        All agents and hooks are embedded in the binary - no external dependencies required.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化项目 - 创建 .claude 目录和嵌入资源
    Init {
        /// 项目名称（可选）
        #[arg(short, long)]
        name: Option<String>,

        /// 强制覆盖已存在的配置
        #[arg(short, long)]
        force: bool,
    },

    /// 运行 hook（由 Claude Code 调用）
    Hook {
        /// Hook 名称: inject_state, codex_review_gate, progress_sync, loop_driver
        name: String,
    },

    /// 显示项目根目录
    Root,

    /// 显示当前状态和进度
    Status,

    /// 列出所有内嵌的 agents
    Agents,

    /// 诊断环境和配置
    Doctor,
}

// ═══════════════════════════════════════════════════════════════════
// Hook 执行（纯 Rust 实现）
// ═══════════════════════════════════════════════════════════════════

fn run_hook(hook_name: &str) -> Result<()> {
    let project_root = match find_project_root() {
        Some(root) => root,
        None => {
            // 如果没有项目根目录，使用当前目录
            env::current_dir()?
        }
    };

    use claude_autonomous::hooks::{print_hook_output, run_hook_from_stdin};

    let output = run_hook_from_stdin(hook_name, &project_root)?;
    print_hook_output(&output);

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// 状态显示
// ═══════════════════════════════════════════════════════════════════

fn show_status() -> Result<()> {
    use claude_autonomous::{state::parse_roadmap, utils::read_json, Memory};

    let project_root = match find_project_root() {
        Some(root) => root,
        None => {
            println!("{}", "❌ No .claude directory found".red());
            println!("Run {} to initialize", "claude-autonomous init".cyan());
            return Ok(());
        }
    };

    println!(
        "{}",
        "╔══════════════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║          Claude Autonomous Engineering Status                     ║".cyan()
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
    let memory: Memory = read_json(&memory_file).unwrap_or_default();

    println!();
    println!("🧠 Current State:");
    println!("   Project: {}", memory.project.yellow());

    if let Some(task_id) = &memory.current_task.id {
        println!("   Task: {}", task_id.cyan());
        println!("   Status: {}", memory.current_task.status.yellow());
        println!(
            "   Retries: {}/{}",
            memory.current_task.retry_count, memory.current_task.max_retries
        );
    }

    // 读取 ROADMAP.md
    let roadmap_file = project_root.join(".claude/status/ROADMAP.md");
    if roadmap_file.exists() {
        use std::fs;
        let content = fs::read_to_string(&roadmap_file)?;
        match parse_roadmap(&content) {
            Ok(data) => {
                let pct = if data.total > 0 {
                    (data.completed.len() as f64 / data.total as f64) * 100.0
                } else {
                    0.0
                };

                println!();
                println!("📋 Progress:");
                println!("   {} Completed: {}", "✓".green(), data.completed.len());
                println!("   {} In Progress: {}", "▶".yellow(), data.in_progress.len());
                println!("   {} Pending: {}", "○".white(), data.pending.len());
                println!("   {} Blocked: {}", "!".red(), data.blocked.len());
                println!("   Total: {} ({:.1}%)", data.total, pct);

                if let Some(phase) = &data.current_phase {
                    println!();
                    println!("📍 Current Phase: {}", phase.cyan());
                }
            }
            Err(e) => {
                println!();
                println!("{}", format!("⚠️  Failed to parse ROADMAP: {}", e).yellow());
            }
        }
    } else {
        println!();
        println!(
            "{}",
            "⚠️  ROADMAP.md not found - Run project planning first".yellow()
        );
    }

    println!();
    println!("💡 Tip: Use {} to see available agents", "claude-autonomous agents".cyan());

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// 列出 Agents
// ═══════════════════════════════════════════════════════════════════

fn list_agents() -> Result<()> {
    println!("{}", "📦 Embedded Agents:".cyan().bold());
    println!();

    let agents = AgentAssets::list_agents();

    for agent in agents {
        println!("  {} {}", "•".green(), agent.yellow());
    }

    println!();
    println!(
        "{} {} embedded agents available",
        "✓".green(),
        AgentAssets::list_agents().len()
    );
    println!();
    println!(
        "💡 All agents are pre-installed in {}",
        ".claude/agents/".cyan()
    );

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// 诊断环境
// ═══════════════════════════════════════════════════════════════════

fn doctor() -> Result<()> {
    use std::fs;

    println!("{}", "🔍 Claude Autonomous Engineering Doctor".cyan().bold());
    println!();

    // 检查项目根目录
    print!("📁 Project root detection... ");
    match find_project_root() {
        Some(root) => {
            println!("{}", "✓".green());
            println!("   {}", root.display().to_string().yellow());
        }
        None => {
            println!("{}", "✗".red());
            println!(
                "   {}",
                "No .claude directory found in current path".red()
            );
            println!("   Run {} to initialize", "claude-autonomous init".cyan());
        }
    }

    if let Some(root) = find_project_root() {
        println!();
        println!("📂 Directory structure:");

        let dirs = vec![
            (".claude/agents", "Agent definitions"),
            (".claude/status", "State files"),
            (".claude/phases", "Phase plans"),
        ];

        for (dir, desc) in dirs {
            let path = root.join(dir);
            if path.exists() {
                let count = fs::read_dir(&path)?.count();
                println!(
                    "   {} {} ({} items)",
                    "✓".green(),
                    desc.yellow(),
                    count
                );
            } else {
                println!("   {} {} {}", "✗".red(), desc.yellow(), "(missing)".red());
            }
        }

        println!();
        println!("📝 Configuration files:");

        let files = vec![
            ("CLAUDE.md", "Project instructions"),
            (".claude/settings.json", "Hook configuration"),
            (".claude/status/memory.json", "State memory"),
            (".claude/status/ROADMAP.md", "Task roadmap"),
            (".claude/status/api_contract.yaml", "API contract"),
        ];

        for (file, desc) in files {
            let path = root.join(file);
            if path.exists() {
                println!("   {} {}", "✓".green(), desc.yellow());
            } else {
                println!("   {} {} {}", "✗".red(), desc.yellow(), "(missing)".red());
            }
        }

        println!();
        println!("🎯 Hooks:");
        let hooks = vec!["inject_state", "progress_sync", "codex_review_gate", "loop_driver"];
        for hook in hooks {
            println!("   {} {}", "✓".green(), hook.cyan());
        }
    }

    println!();
    println!("{}", "✅ Diagnostic complete".green().bold());

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, force } => {
            let cwd = env::current_dir()?;
            init_project(&cwd, name.as_deref(), force)
        }
        Commands::Hook { name } => run_hook(&name),
        Commands::Root => {
            match find_project_root() {
                Some(root) => println!("{}", root.display()),
                None => {
                    eprintln!("{}", "No .claude directory found".red());
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Commands::Status => show_status(),
        Commands::Agents => list_agents(),
        Commands::Doctor => doctor(),
    }
}
