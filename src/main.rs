use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use serde_json::json;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Claude Autonomous Engineering CLI
/// 
/// 统一的命令行工具，支持自动查找项目根目录（包括 submodule）
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
// 项目根目录查找
// ═══════════════════════════════════════════════════════════════════

fn find_project_root() -> Option<PathBuf> {
    // 方法1: 优先检查 git superproject（submodule 的父项目）
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-superproject-working-tree"])
        .output()
    {
        if output.status.success() {
            let super_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !super_root.is_empty() {
                let path = PathBuf::from(&super_root);
                if path.join(".claude").is_dir() {
                    return Some(path);
                }
            }
        }
    }

    // 方法2: 当前目录
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join(".claude").is_dir() {
            return Some(cwd);
        }
    }

    // 方法3: git 仓库根目录
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let git_root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(&git_root);
            if path.join(".claude").is_dir() {
                return Some(path);
            }
        }
    }

    // 方法4: 向上遍历
    if let Ok(mut current) = std::env::current_dir() {
        for _ in 0..10 {
            if current.join(".claude").is_dir() {
                return Some(current);
            }
            if let Some(parent) = current.parent() {
                current = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    None
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

    let hook_path = project_root
        .join(".claude")
        .join("hooks")
        .join(format!("{}.py", hook_name));

    if !hook_path.exists() {
        print_graceful_response(hook_name);
        return Ok(());
    }

    // 读取 stdin
    let mut stdin_data = String::new();
    io::stdin().read_to_string(&mut stdin_data)?;

    // 执行 hook
    let output = Command::new("python3")
        .arg(&hook_path)
        .current_dir(&project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PROJECT_ROOT", &project_root)
        .spawn()
        .context("Failed to spawn python3")?
        .wait_with_output()
        .context("Failed to wait for hook")?;

    // 如果需要传入 stdin
    if !stdin_data.is_empty() {
        let mut child = Command::new("python3")
            .arg(&hook_path)
            .current_dir(&project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PROJECT_ROOT", &project_root)
            .spawn()
            .context("Failed to spawn python3")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_data.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    } else {
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

fn print_graceful_response(hook_name: &str) {
    let response = match hook_name {
        "inject_state" => json!({
            "hookSpecificOutput": {
                "additionalContext": ""
            }
        }),
        "codex_review_gate" | "pre_write_check" => json!({
            "decision": "allow"
        }),
        "progress_sync" | "post_write_update" => json!({
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
        eprintln!("{}", "⚠️  .claude directory already exists. Use --force to overwrite.".yellow());
        return Ok(());
    }

    let project_name = name.unwrap_or_else(|| {
        cwd.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "my-project".to_string())
    });

    println!("{}", format!("🚀 Initializing Claude Autonomous Engineering for: {}", project_name).cyan());

    // 创建目录结构
    let dirs = [
        ".claude/hooks",
        ".claude/lib",
        ".claude/status",
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

    // 创建 hook 模板
    create_hook_templates(&claude_dir)?;
    println!("  {} Created hook templates", "✓".green());

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
        "_comment": "Claude Autonomous Engineering - Hook configuration",
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
        "decisions_log": []
    });
    serde_json::to_string_pretty(&memory).unwrap()
}

fn create_claude_md(project_name: &str) -> String {
    format!(r#"# {} - Claude Autonomous Engineering

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
"#, project_name)
}

fn create_hook_templates(claude_dir: &Path) -> Result<()> {
    // inject_state.py
    let inject_state = r#"#!/usr/bin/env python3
"""Inject State Hook - 注入上下文到每次交互"""
import sys
import json
import os

def main():
    stdin_data = sys.stdin.read()
    project_root = os.environ.get("PROJECT_ROOT", os.getcwd())
    
    context_parts = []
    context_parts.append("""
╔══════════════════════════════════════════════════════════════════╗
║                 🤖 AUTONOMOUS MODE ACTIVE                         ║
╚══════════════════════════════════════════════════════════════════╝
""")
    
    # 读取 memory.json
    memory_file = os.path.join(project_root, ".claude", "status", "memory.json")
    if os.path.exists(memory_file):
        with open(memory_file, 'r', encoding='utf-8') as f:
            memory = json.load(f)
        context_parts.append(f"\n## 🧠 CURRENT STATE\n```json\n{json.dumps(memory, indent=2, ensure_ascii=False)}\n```\n")
    
    # 读取 ROADMAP.md
    roadmap_file = os.path.join(project_root, ".claude", "status", "ROADMAP.md")
    if os.path.exists(roadmap_file):
        with open(roadmap_file, 'r', encoding='utf-8') as f:
            content = f.read()
        pending = [l for l in content.split('\n') if '- [ ]' in l or '- [>]' in l]
        if pending:
            context_parts.append("\n## 📋 PENDING TASKS\n" + '\n'.join(pending[:15]) + "\n")
    
    print(json.dumps({
        "hookSpecificOutput": {
            "additionalContext": ''.join(context_parts)
        }
    }))

if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(json.dumps({"hookSpecificOutput": {"additionalContext": f"[Error: {e}]"}}))
"#;
    fs::write(claude_dir.join("hooks/inject_state.py"), inject_state)?;

    // codex_review_gate.py
    let codex_review = r#"#!/usr/bin/env python3
"""Codex Review Gate - 提交前代码审查"""
import sys
import json

def main():
    input_data = json.loads(sys.stdin.read())
    command = input_data.get("tool_input", {}).get("command", "")
    
    # 只拦截 git commit/push
    if "git commit" not in command and "git push" not in command:
        print(json.dumps({"decision": "allow"}))
        return
    
    # TODO: 在这里添加 Codex 审查逻辑
    print(json.dumps({"decision": "allow"}))

if __name__ == "__main__":
    try:
        main()
    except:
        print(json.dumps({"decision": "allow"}))
"#;
    fs::write(claude_dir.join("hooks/codex_review_gate.py"), codex_review)?;

    // progress_sync.py
    let progress_sync = r#"#!/usr/bin/env python3
"""Progress Sync - 自动同步 Markdown 进度到 memory.json"""
import sys
import json

def main():
    # PostToolUse hook - 检测文件修改并同步
    print(json.dumps({"status": "ok"}))

if __name__ == "__main__":
    try:
        main()
    except:
        print(json.dumps({"status": "ok"}))
"#;
    fs::write(claude_dir.join("hooks/progress_sync.py"), progress_sync)?;

    // loop_driver.py
    let loop_driver = r#"#!/usr/bin/env python3
"""Loop Driver - 控制自主循环"""
import sys
import json
import os

def main():
    project_root = os.environ.get("PROJECT_ROOT", os.getcwd())
    
    # 检查 ROADMAP 是否完成
    roadmap_file = os.path.join(project_root, ".claude", "status", "ROADMAP.md")
    if os.path.exists(roadmap_file):
        with open(roadmap_file, 'r', encoding='utf-8') as f:
            content = f.read()
        pending = [l for l in content.split('\n') if '- [ ]' in l or '- [>]' in l]
        if pending:
            print(json.dumps({
                "decision": "block",
                "reason": f"[Loop] {len(pending)} tasks remaining. Continue working!"
            }))
            return
    
    print(json.dumps({"decision": "allow"}))

if __name__ == "__main__":
    try:
        main()
    except:
        print(json.dumps({"decision": "allow"}))
"#;
    fs::write(claude_dir.join("hooks/loop_driver.py"), loop_driver)?;

    Ok(())
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

    println!("{}", "╔══════════════════════════════════════════════════════════════════╗".cyan());
    println!("{}", "║              Claude Autonomous Engineering Status                 ║".cyan());
    println!("{}", "╚══════════════════════════════════════════════════════════════════╝".cyan());
    println!();
    println!("📁 Project Root: {}", project_root.display().to_string().green());

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
        println!("{}", "⚠️  ROADMAP.md not found - Run planning first".yellow());
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
