//! Claude Autonomous Engineering TUI
//!
//! 基于 Ratatui 的多智能体会话管理系统
//!
//! 用法:
//! - `claude-autonomous` - 启动 TUI 界面
//! - `claude-autonomous hook <name>` - 运行 hook（供 Claude Code 调用）
//! - `claude-autonomous --shell` - 启动 shell 测试模式

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{KeyCode, KeyModifiers};
use std::env;
use std::time::Duration;

use claude_autonomous::tui::{
    init_terminal, install_panic_hook, restore_terminal, App, AppMode, Event, EventHandler,
};

/// Claude Autonomous Engineering TUI
///
/// 基于 Ratatui 的多智能体会话管理系统
#[derive(Parser)]
#[command(name = "claude-autonomous")]
#[command(author, version, about = "Multi-agent TUI orchestration system")]
struct Cli {
    /// 启动 shell 测试模式（而非 Claude）
    #[arg(long)]
    shell: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 运行 hook（由 Claude Code 调用）
    Hook {
        /// Hook 名称: inject_state, progress_sync, repo_map_sync, codex_review_gate, error_tracker, loop_driver
        name: String,
    },
}

/// 运行 hook（保持向后兼容）
fn run_hook(hook_name: &str) -> Result<()> {
    let project_root = match claude_autonomous::find_project_root() {
        Some(root) => root,
        None => env::current_dir()?,
    };

    use claude_autonomous::hooks::{print_hook_output, run_hook_from_stdin};

    let output = run_hook_from_stdin(hook_name, &project_root)?;
    print_hook_output(&output);

    Ok(())
}

/// 运行 TUI 主循环
fn run_tui(use_shell: bool) -> Result<()> {
    // 安装 panic hook 确保终端恢复
    install_panic_hook();

    // 初始化终端
    let mut terminal = init_terminal()?;

    // 获取终端大小
    let size = terminal.size()?;
    let mut app = App::new(size.width, size.height);

    // 启动进程
    let reader = if use_shell {
        app.spawn_shell()?
    } else {
        app.spawn_claude()?
    };

    // 创建事件处理器
    let tick_rate = Duration::from_millis(50); // 20fps
    let events = EventHandler::new(tick_rate);

    // 启动 PTY 读取线程
    events.start_pty_reader(reader);

    // 主事件循环
    loop {
        // 渲染 UI
        terminal.draw(|frame| {
            claude_autonomous::tui::ui::render(frame, &app);
        })?;

        // 处理事件
        match events.next() {
            Ok(event) => match event {
                Event::Key(key) => {
                    handle_key_event(&mut app, key)?;
                }
                Event::PtyOutput(data) => {
                    app.process_pty_output(&data);
                }
                Event::PtyExit(code) => {
                    app.status_message = format!(
                        "Process exited (code: {:?}) | Press any key to quit",
                        code
                    );
                }
                Event::Resize(w, h) => {
                    app.resize(w, h)?;
                }
                Event::Error(e) => {
                    app.status_message = format!("Error: {}", e);
                }
                Event::Tick | Event::Mouse(_) => {
                    // 只触发重新渲染
                }
            },
            Err(_) => {
                // Channel 关闭，退出
                break;
            }
        }

        if app.should_quit {
            break;
        }
    }

    // 恢复终端
    restore_terminal()?;

    Ok(())
}

/// 处理键盘事件
fn handle_key_event(
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<()> {
    match app.mode {
        AppMode::Normal => {
            match (key.modifiers, key.code) {
                // Ctrl+Q 退出
                (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                    app.should_quit = true;
                }
                // Ctrl+C 退出
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                    // 发送 Ctrl+C 到 PTY
                    app.send_input(&[0x03])?;
                }
                // Ctrl+B 进入命令模式
                (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                    app.mode = AppMode::Command;
                    app.status_message = "Command mode | ESC to exit | Enter to execute".to_string();
                }
                // 其他按键发送到 PTY
                _ => {
                    let bytes = key_to_bytes(key);
                    app.send_input(&bytes)?;
                }
            }
        }
        AppMode::Command => {
            match key.code {
                KeyCode::Esc => {
                    app.mode = AppMode::Normal;
                    app.input_buffer.clear();
                    app.status_message =
                        "Press Ctrl+Q to quit | Ctrl+B for command mode".to_string();
                }
                KeyCode::Enter => {
                    execute_command(app)?;
                    app.mode = AppMode::Normal;
                    app.input_buffer.clear();
                }
                KeyCode::Char(c) => {
                    app.input_buffer.push(c);
                }
                KeyCode::Backspace => {
                    app.input_buffer.pop();
                }
                _ => {}
            }
        }
        AppMode::Quitting => {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    app.should_quit = true;
                }
                _ => {
                    app.mode = AppMode::Normal;
                    app.status_message =
                        "Press Ctrl+Q to quit | Ctrl+B for command mode".to_string();
                }
            }
        }
    }
    Ok(())
}

/// 执行内部命令
fn execute_command(app: &mut App) -> Result<()> {
    let cmd = app.input_buffer.trim().to_lowercase();

    match cmd.as_str() {
        "quit" | "q" => {
            app.should_quit = true;
        }
        "clear" | "cls" => {
            if let Ok(mut state) = app.terminal_state.lock() {
                state.output_buffer.clear();
            }
            app.status_message = "Buffer cleared".to_string();
        }
        "help" | "?" => {
            app.status_message =
                "Commands: quit, clear, help | Ctrl+Q to quit".to_string();
        }
        _ => {
            app.status_message = format!("Unknown command: {}", cmd);
        }
    }

    Ok(())
}

/// 将键盘事件转换为字节
fn key_to_bytes(key: crossterm::event::KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+字母 -> 控制字符
                let ctrl_char = (c as u8) & 0x1f;
                vec![ctrl_char]
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f], // DEL
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => vec![0x1b, b'[', b'A'],
        KeyCode::Down => vec![0x1b, b'[', b'B'],
        KeyCode::Right => vec![0x1b, b'[', b'C'],
        KeyCode::Left => vec![0x1b, b'[', b'D'],
        KeyCode::Home => vec![0x1b, b'[', b'H'],
        KeyCode::End => vec![0x1b, b'[', b'F'],
        KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
        KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
        KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
        KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
        KeyCode::F(n) => {
            // F1-F12 的转义序列
            match n {
                1 => vec![0x1b, b'O', b'P'],
                2 => vec![0x1b, b'O', b'Q'],
                3 => vec![0x1b, b'O', b'R'],
                4 => vec![0x1b, b'O', b'S'],
                5 => vec![0x1b, b'[', b'1', b'5', b'~'],
                6 => vec![0x1b, b'[', b'1', b'7', b'~'],
                7 => vec![0x1b, b'[', b'1', b'8', b'~'],
                8 => vec![0x1b, b'[', b'1', b'9', b'~'],
                9 => vec![0x1b, b'[', b'2', b'0', b'~'],
                10 => vec![0x1b, b'[', b'2', b'1', b'~'],
                11 => vec![0x1b, b'[', b'2', b'3', b'~'],
                12 => vec![0x1b, b'[', b'2', b'4', b'~'],
                _ => vec![],
            }
        }
        _ => vec![],
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Hook { name }) => run_hook(&name),
        None => run_tui(cli.shell),
    }
}
