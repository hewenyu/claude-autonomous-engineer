use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use std::env;

use claude_autonomous::{find_project_root, project::init_project, templates::AgentAssets};

/// Claude Autonomous Engineering CLI
///
/// 纯 Rust 实现的自主工程工具 - 零 Python 依赖
#[derive(Parser)]
#[command(name = "claude-autonomous")]
#[command(author, version = env!("APP_VERSION"), about)]
#[command(
    long_about = "A pure Rust implementation of Claude Autonomous Engineering toolkit.\n\
                        All agents and hooks are embedded in the binary - no external dependencies required."
)]
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

    /// 生成 Repository Map（代码库结构骨架）
    Map {
        /// 输出文件路径（默认：.claude/repo_map/structure.toon 或 structure.md）
        #[arg(short, long)]
        output: Option<String>,

        /// 强制重新生成（忽略缓存）
        #[arg(short, long)]
        force: bool,

        /// 输出格式：markdown, toon, toon-grouped（默认：toon）
        #[arg(long, default_value = "toon")]
        format: String,
    },

    /// 状态机管理（Git 驱动的状态快照）
    #[command(subcommand)]
    State(StateCommands),
}

/// 状态机子命令
#[derive(Subcommand)]
enum StateCommands {
    /// 列出所有状态快照
    List,

    /// 显示当前状态
    Current,

    /// 回滚到指定 tag
    Rollback {
        /// Tag 名称（例如：state-20251231-120000-planning-TASK-001）
        tag: String,
    },

    /// 显示状态转换图
    Graph {
        /// 仅显示指定任务的转换（可选）
        #[arg(short, long)]
        task_id: Option<String>,
    },

    /// 手动创建状态转换
    Transition {
        /// 目标状态（idle, planning, coding, testing, reviewing, completed, blocked）
        state: String,

        /// 任务 ID（可选）
        #[arg(short, long)]
        task_id: Option<String>,
    },

    /// 显示工作流帮助
    Help,
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
                    ((data.completed.len() + data.skipped.len()) as f64 / data.total as f64) * 100.0
                } else {
                    0.0
                };

                println!();
                println!("📋 Progress:");
                println!("   {} Completed: {}", "✓".green(), data.completed.len());
                println!(
                    "   {} In Progress: {}",
                    "▶".yellow(),
                    data.in_progress.len()
                );
                println!("   {} Pending: {}", "○".white(), data.pending.len());
                println!("   {} Blocked: {}", "!".red(), data.blocked.len());
                println!("   {} Skipped: {}", "−".blue(), data.skipped.len());
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
    println!(
        "💡 Tip: Use {} to see available agents",
        "claude-autonomous agents".cyan()
    );

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

    println!(
        "{}",
        "🔍 Claude Autonomous Engineering Doctor".cyan().bold()
    );
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
            println!("   {}", "No .claude directory found in current path".red());
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
                println!("   {} {} ({} items)", "✓".green(), desc.yellow(), count);
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
        let hooks = vec![
            "inject_state",
            "progress_sync",
            "codex_review_gate",
            "loop_driver",
        ];
        for hook in hooks {
            println!("   {} {}", "✓".green(), hook.cyan());
        }
    }

    println!();
    println!("{}", "✅ Diagnostic complete".green().bold());

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Repository Map
// ═══════════════════════════════════════════════════════════════════

fn generate_repo_map(output: Option<String>, force: bool, format_str: String) -> Result<()> {
    use claude_autonomous::repo_map::{OutputFormat, RepoMapper};
    use std::time::Instant;

    let project_root = match find_project_root() {
        Some(root) => root,
        None => {
            println!("{}", "❌ No .claude directory found".red());
            println!("Run {} to initialize", "claude-autonomous init".cyan());
            return Ok(());
        }
    };

    // 解析格式参数
    let format = match format_str.to_lowercase().as_str() {
        "markdown" | "md" => OutputFormat::Markdown,
        "toon" => OutputFormat::Toon,
        "toon-grouped" | "grouped" => OutputFormat::ToonGrouped,
        _ => {
            println!("{}", format!("❌ Unknown format: {}", format_str).red());
            println!("Available formats: markdown, toon, toon-grouped");
            return Ok(());
        }
    };

    let format_name = match format {
        OutputFormat::Markdown => "Markdown",
        OutputFormat::Toon => "TOON",
        OutputFormat::ToonGrouped => "TOON (Grouped)",
    };

    println!(
        "{}",
        format!("🗺️  Generating Repository Map ({})...", format_name)
            .cyan()
            .bold()
    );
    println!();

    let start = Instant::now();

    // 如果强制重新生成，清除缓存
    if force {
        let cache_file = project_root.join(".claude/repo_map/cache.json");
        if cache_file.exists() {
            std::fs::remove_file(&cache_file)?;
            println!("{}", "   🗑️  Cleared cache".yellow());
        }
    }

    // 生成 map
    let mut mapper = RepoMapper::new(&project_root)?;
    let content = mapper.generate_map_with_format(format)?;

    // 确定输出路径和扩展名
    let default_extension = match format {
        OutputFormat::Markdown => "md",
        OutputFormat::Toon | OutputFormat::ToonGrouped => "toon",
    };

    let output_path = if let Some(path) = output {
        project_root.join(path)
    } else {
        project_root.join(format!(".claude/repo_map/structure.{}", default_extension))
    };

    // 确保目录存在
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 写入文件
    std::fs::write(&output_path, &content)?;

    let elapsed = start.elapsed();

    // Token 统计（简单估算）
    let token_count = content.split_whitespace().count();
    let token_saved_msg = match format {
        OutputFormat::Toon | OutputFormat::ToonGrouped => {
            format!(
                " (预计节省 30-60% tokens，约 {} tokens)",
                token_count.to_string().cyan()
            )
        }
        OutputFormat::Markdown => String::new(),
    };

    println!();
    println!("{}", "✅ Repository Map generated!".green().bold());
    println!("   📁 Output: {}", output_path.display().to_string().cyan());
    println!("   📊 Format: {}{}", format_name.cyan(), token_saved_msg);
    println!("   ⏱️  Time: {:.2}s", elapsed.as_secs_f64());
    println!();

    if matches!(format, OutputFormat::Toon | OutputFormat::ToonGrouped) {
        println!("💡 Tip: TOON 格式可减少 30-60% token 消耗，更适合 LLM 处理");
    } else {
        println!("💡 Tip: Repository Map 已保存，可用于减少 token 消耗");
    }

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
        Commands::Map {
            output,
            force,
            format,
        } => generate_repo_map(output, force, format),
        Commands::State(cmd) => {
            use claude_autonomous::cli;

            match cmd {
                StateCommands::List => cli::list_states(),
                StateCommands::Current => cli::show_current_state(),
                StateCommands::Rollback { tag } => cli::rollback_to_tag(&tag),
                StateCommands::Graph { task_id } => cli::show_state_graph(task_id.as_deref()),
                StateCommands::Transition { state, task_id } => {
                    cli::transition_to(&state, task_id.as_deref())
                }
                StateCommands::Help => cli::show_workflow_help(),
            }
        }
    }
}
