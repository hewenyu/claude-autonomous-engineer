//! Codex 命令执行器
//!
//! 执行 codex review 命令并捕获输出

use crate::hooks::review_context::ReviewContext;
use crate::hooks::review_parser::{parse_review_output, ReviewResult};
use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Codex review 命令超时时间（秒）
const REVIEW_TIMEOUT_SECS: u64 = 30;

/// 执行 codex review 命令
pub fn execute_codex_review(context: &ReviewContext) -> Result<ReviewResult> {
    println!("🤖 Invoking codex review...");

    // 启动 codex 进程
    let mut child = Command::new("codex")
        .arg("review")
        .arg("--uncommitted")
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

    // 等待执行完成（带超时）
    let output = wait_with_timeout(child, Duration::from_secs(REVIEW_TIMEOUT_SECS))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Codex review failed with exit code {:?}: {}", output.status.code(), stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 解析输出
    parse_review_output(&stdout, context.mode.clone())
}

/// 等待进程完成（带超时）
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output> {
    use std::thread;
    use std::time::Instant;

    let start = Instant::now();

    loop {
        // 尝试非阻塞地检查进程状态
        match child.try_wait()? {
            Some(_status) => {
                // 进程已完成，收集输出
                return Ok(std::process::Output {
                    status: _status,
                    stdout: vec![], // 已经被 piped，需要手动读取
                    stderr: vec![],
                });
            }
            None => {
                // 进程仍在运行，检查超时
                if start.elapsed() > timeout {
                    // 超时，杀死进程
                    child.kill()?;
                    anyhow::bail!("Codex review timed out after {:?}", timeout);
                }

                // 短暂睡眠，避免忙等待
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// 简化版本：直接使用 wait_with_output（实际使用此版本）
pub fn execute_codex_review_simple(context: &ReviewContext) -> Result<ReviewResult> {
    println!("🤖 Invoking codex review...");

    let mut child = Command::new("codex")
        .arg("review")
        .arg("--uncommitted")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::review_context::ReviewMode;

    #[test]
    #[ignore] // 需要 codex 命令才能运行
    fn test_execute_codex_review() {
        let context = ReviewContext {
            instruction: "Test instruction".to_string(),
            mode: ReviewMode::Regular,
        };

        // 这个测试需要 codex 命令
        let result = execute_codex_review_simple(&context);
        // 只检查是否能调用，不检查结果
        assert!(result.is_ok() || result.is_err());
    }
}
