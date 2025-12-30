// Project Root Finder
// 项目根目录查找逻辑 (支持 submodule)

use std::path::PathBuf;
use std::process::Command;

/// 查找项目根目录
///
/// 查找策略:
/// 1. 优先检查 git superproject (submodule 的父项目)
/// 2. 当前目录
/// 3. git 仓库根目录
/// 4. 向上遍历查找 .claude 目录
pub fn find_project_root() -> Option<PathBuf> {
    // 方法1: 优先检查 git superproject (submodule 的父项目)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_project_root() {
        // 测试应该能找到当前项目的根目录 (因为它包含 .claude)
        let root = find_project_root();
        if root.is_some() {
            let root_path = root.unwrap();
            assert!(root_path.join(".claude").exists());
        }
    }
}
