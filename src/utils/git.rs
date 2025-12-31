//! Git 操作工具

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// 执行 git 命令并返回输出
pub fn git_command(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd.output().context("Failed to execute git command")?;

    if !output.status.success() {
        anyhow::bail!(
            "Git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 获取 git 仓库根目录
pub fn get_git_root(cwd: Option<&Path>) -> Result<String> {
    git_command(&["rev-parse", "--show-toplevel"], cwd)
}

/// 获取 git superproject 根目录（用于 submodule）
pub fn get_git_superproject_root(cwd: Option<&Path>) -> Result<Option<String>> {
    match git_command(&["rev-parse", "--show-superproject-working-tree"], cwd) {
        Ok(path) if !path.is_empty() => Ok(Some(path)),
        _ => Ok(None),
    }
}

/// 获取暂存文件列表
pub fn get_staged_files(cwd: Option<&Path>) -> Result<Vec<String>> {
    let output = git_command(&["diff", "--cached", "--name-only"], cwd)?;

    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

/// 获取 git 日志
pub fn get_git_log(limit: usize, cwd: Option<&Path>) -> Result<String> {
    git_command(
        &["log", &format!("-{}", limit), "--oneline", "--name-status"],
        cwd,
    )
}

/// 检查当前目录是否在 git 仓库中
pub fn is_git_repo(cwd: Option<&Path>) -> bool {
    git_command(&["rev-parse", "--git-dir"], cwd).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_git_repo_true_and_false() {
        let non_repo = TempDir::new().unwrap();
        assert!(!is_git_repo(Some(non_repo.path())));

        let repo = TempDir::new().unwrap();
        let status = Command::new("git")
            .args(["init"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success());
        assert!(is_git_repo(Some(repo.path())));
    }

    #[test]
    fn test_get_git_root() {
        let repo = TempDir::new().unwrap();
        let status = Command::new("git")
            .args(["init"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success());

        let root = get_git_root(Some(repo.path())).unwrap();
        assert!(!root.is_empty());

        let expected = std::fs::canonicalize(repo.path()).unwrap();
        let actual = std::fs::canonicalize(&root).unwrap();
        assert_eq!(actual, expected);
    }
}
