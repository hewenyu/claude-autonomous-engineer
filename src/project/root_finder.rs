//! 项目根目录查找
//!
//! 支持多种场景：
//! - Git superproject（submodule 的父项目）
//! - 当前目录
//! - Git 仓库根目录
//! - 向上遍历父目录

use crate::utils::git::{get_git_root, get_git_superproject_root};
use std::env;
use std::path::PathBuf;

/// 查找项目根目录
///
/// 搜索包含 .claude 目录的项目根目录
///
/// 搜索顺序：
/// 1. Git superproject（优先检查 submodule 的父项目）
/// 2. 当前目录
/// 3. Git 仓库根目录
/// 4. 向上遍历父目录（最多 10 层）
///
/// # Returns
///
/// 返回包含 .claude 目录的路径，如果找不到返回 None
pub fn find_project_root() -> Option<PathBuf> {
    // 方法1: 优先检查 git superproject（submodule 的父项目）
    if let Ok(Some(super_root)) = get_git_superproject_root(None) {
        let path = PathBuf::from(&super_root);
        if path.join(".claude").is_dir() {
            return Some(path);
        }
    }

    // 方法2: 当前目录
    if let Ok(cwd) = env::current_dir() {
        if cwd.join(".claude").is_dir() {
            return Some(cwd);
        }
    }

    // 方法3: git 仓库根目录
    if let Ok(git_root) = get_git_root(None) {
        let path = PathBuf::from(&git_root);
        if path.join(".claude").is_dir() {
            return Some(path);
        }
    }

    // 方法4: 向上遍历
    if let Ok(mut current) = env::current_dir() {
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

/// 查找项目根目录，失败时返回当前目录
pub fn find_project_root_or_current() -> PathBuf {
    find_project_root().unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root_in_current_dir() {
        // 保存当前目录
        let original_dir = env::current_dir().unwrap();

        let temp = TempDir::new().unwrap();
        let claude_dir = temp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();

        // 切换到临时目录
        env::set_current_dir(temp.path()).unwrap();

        let root = find_project_root();

        // 恢复原目录
        env::set_current_dir(&original_dir).unwrap();

        assert!(root.is_some());
        assert_eq!(root.unwrap(), temp.path());
    }

    #[test]
    fn test_find_project_root_or_current() {
        let root = find_project_root_or_current();
        assert!(root.exists());
    }
}
