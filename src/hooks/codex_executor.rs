//! Codex 命令执行器
//!
//! 执行 codex review 命令并捕获输出

use crate::hooks::codex_resolver::resolve_codex_path;
use crate::hooks::review_context::ReviewContext;
use crate::hooks::review_parser::{parse_review_output, ReviewResult};
use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// 执行 codex review 命令
pub fn execute_codex_review(context: &ReviewContext) -> Result<ReviewResult> {
    // 当前实现使用 `wait_with_output()`，避免自制超时逻辑导致 stdout/stderr 丢失。
    execute_codex_review_simple(context)
}

/// 简化版本：直接使用 wait_with_output（实际使用此版本）
pub fn execute_codex_review_simple(context: &ReviewContext) -> Result<ReviewResult> {
    eprintln!("🤖 Invoking codex exec...");

    let codex_bin = resolve_codex_path().context("Failed to resolve codex command path")?;

    let mut child = Command::new(&codex_bin)
        .arg("exec")
        .current_dir(&context.project_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn codex process. Is 'codex' installed and in PATH?")?;

    // 写入自定义指令到 stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(context.instruction.as_bytes())
            .context("Failed to write to codex stdin")?;
    }

    // 等待执行完成
    let output = child
        .wait_with_output()
        .context("Failed to wait for codex")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        anyhow::bail!(
            "Codex review failed with exit code {:?}:\n{}",
            output.status.code(),
            stderr
        );
    }

    // 组合 stdout 和 stderr - codex 可能将输出写到任一流
    let combined_output = if stdout.is_empty() && !stderr.is_empty() {
        eprintln!("⚠️  Warning: codex wrote output to stderr instead of stdout");
        stderr.to_string()
    } else if !stdout.is_empty() && !stderr.is_empty() {
        // 两者都有内容，优先使用 stdout，但记录 stderr
        eprintln!("⚠️  codex also wrote to stderr: {}", stderr);
        stdout.to_string()
    } else {
        stdout.to_string()
    };

    // 解析输出
    parse_review_output(&combined_output, context.mode.clone())
}
