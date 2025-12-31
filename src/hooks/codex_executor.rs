//! Codex 命令执行器
//!
//! 执行 codex review 命令并捕获输出

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
    eprintln!("🤖 Invoking codex review...");

    let codex_bin =
        std::env::var("CLAUDE_AUTONOMOUS_CODEX_BIN").unwrap_or_else(|_| "codex".to_string());

    let mut child = Command::new(codex_bin)
        .arg("review")
        .arg("--uncommitted")
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

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        // 如果是因为 codex 命令不存在
        if stderr.contains("not found") || stderr.contains("No such file") {
            anyhow::bail!(
                "Codex command not found. Please install codex CLI tool.\n\
                 Visit: https://github.com/your-codex-repo (replace with actual URL)"
            );
        }

        anyhow::bail!(
            "Codex review failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 解析输出
    parse_review_output(&stdout, context.mode.clone())
}
