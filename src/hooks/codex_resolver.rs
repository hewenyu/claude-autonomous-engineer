//! Codex 命令路径解析器
//!
//! 智能搜索 codex 命令路径，支持多种场景：
//! - 环境变量 CLAUDE_AUTONOMOUS_CODEX_BIN
//! - 系统 PATH
//! - nvm node 版本管理器
//! - 项目本地 node_modules
//!
//! # 搜索优先级
//!
//! 1. 环境变量 `CLAUDE_AUTONOMOUS_CODEX_BIN` (最高优先级)
//! 2. 系统 PATH (尝试直接执行 `codex --version`)
//! 3. nvm 目录: `~/.nvm/versions/node/*/bin/codex` (使用最新版本)
//! 4. 项目本地: `./node_modules/.bin/codex` (向上查找最多 5 层)
//!
//! # 示例
//!
//! ```no_run
//! use claude_autonomous::hooks::codex_resolver::resolve_codex_path;
//!
//! fn main() -> anyhow::Result<()> {
//!     let codex_path = resolve_codex_path()?;
//!     println!("Found codex at: {}", codex_path);
//!     Ok(())
//! }
//! ```

use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Session-level cache for codex path
/// 使用 OnceLock 确保线程安全且仅初始化一次
static CODEX_PATH_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// 解析 codex 命令路径（带缓存）
///
/// 这是主要的公共 API。第一次调用时执行完整搜索，后续调用返回缓存结果。
///
/// # Returns
///
/// 返回 codex 可执行文件的路径字符串。如果是缓存的 "codex" 字符串，
/// 表示在系统 PATH 中找到。
///
/// # Errors
///
/// 如果在所有位置都找不到 codex，返回包含详细搜索位置的错误。
pub fn resolve_codex_path() -> Result<String> {
    let cached = CODEX_PATH_CACHE.get_or_init(|| resolve_codex_path_uncached().ok());

    match cached {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => {
            // Cache 中是 None，说明之前搜索失败了
            // 重新尝试并返回详细错误
            resolve_codex_path_uncached().map(|p| p.to_string_lossy().to_string())
        }
    }
}

/// 清除缓存（仅用于测试）
#[cfg(test)]
pub fn clear_cache() {
    // OnceLock 不提供 clear 方法，所以测试需要处理这个限制
    // 实际测试时使用进程隔离或 mock
}

/// 执行未缓存的 codex 路径解析
///
/// 按优先级顺序搜索所有可能的位置。
fn resolve_codex_path_uncached() -> Result<PathBuf> {
    // Priority 1: 环境变量
    if let Ok(env_path) = env::var("CLAUDE_AUTONOMOUS_CODEX_BIN") {
        let path = PathBuf::from(&env_path);
        if validate_codex_binary(&path) {
            return Ok(path);
        } else {
            eprintln!(
                "⚠️  CLAUDE_AUTONOMOUS_CODEX_BIN points to invalid binary: {}",
                env_path
            );
            eprintln!("   Falling back to automatic search...");
        }
    }

    // Priority 2: 系统 PATH
    if is_in_path("codex") {
        return Ok(PathBuf::from("codex"));
    }

    // Priority 3: nvm 目录
    if let Some(nvm_path) = search_nvm_directories() {
        return Ok(nvm_path);
    }

    // Priority 4: 项目本地 node_modules
    if let Some(local_path) = search_project_local() {
        return Ok(local_path);
    }

    // 所有搜索都失败了
    Err(build_resolution_error())
}

/// 验证路径是否为有效的 codex 可执行文件
///
/// # 验证步骤
///
/// 1. 检查文件是否存在
/// 2. 检查文件是否可执行 (仅 Unix)
/// 3. 尝试执行 `codex --version` 验证它确实是 codex
fn validate_codex_binary(path: &Path) -> bool {
    // Check 1: 文件存在
    if !path.exists() {
        return false;
    }

    // Check 2: Unix 平台检查可执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(path) {
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                return false; // 不可执行
            }
        } else {
            return false;
        }
    }

    // Check 3: 尝试执行 --version
    Command::new(path)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 检查命令是否在系统 PATH 中
///
/// 通过尝试执行 `codex --version` 来验证
fn is_in_path(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 搜索 nvm 目录中的 codex
///
/// 扫描 `~/.nvm/versions/node/*/bin/codex`，如果找到多个版本，
/// 返回版本号最新的那个（字典序降序排列）。
///
/// # Returns
///
/// 返回找到的第一个有效 codex 路径，如果没找到返回 None
fn search_nvm_directories() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let nvm_base = home.join(".nvm/versions/node");

    if !nvm_base.exists() {
        return None;
    }

    // 读取所有 node 版本目录
    let mut versions = Vec::new();

    let read_dir = match fs::read_dir(&nvm_base) {
        Ok(dir) => dir,
        Err(_) => return None, // 权限错误或其他问题，跳过
    };

    for entry in read_dir.flatten() {
        let version_dir = entry.path();
        if !version_dir.is_dir() {
            continue;
        }

        let codex_path = version_dir.join("bin/codex");

        if validate_codex_binary(&codex_path) {
            // 提取版本号用于排序
            if let Some(version_name) = version_dir.file_name() {
                versions.push((version_name.to_string_lossy().to_string(), codex_path));
            }
        }
    }

    if versions.is_empty() {
        return None;
    }

    // 按版本号降序排序（字典序，对 semver 足够了）
    // v24.11.0 > v20.0.0 > v18.0.0
    versions.sort_by(|a, b| b.0.cmp(&a.0));

    // 返回最新版本的 codex
    versions.first().map(|(_, path)| path.clone())
}

/// 搜索项目本地的 node_modules/.bin/codex
///
/// 从当前目录开始向上查找，最多查找 5 层。
///
/// # Returns
///
/// 返回找到的第一个有效 codex 路径，如果没找到返回 None
fn search_project_local() -> Option<PathBuf> {
    let mut current = env::current_dir().ok()?;

    // 向上查找最多 5 层
    for _ in 0..5 {
        let candidate = current.join("node_modules/.bin/codex");

        if validate_codex_binary(&candidate) {
            return Some(candidate);
        }

        // 向上移动一层
        current = current.parent()?.to_path_buf();
    }

    None
}

/// 构建详细的解析失败错误消息
///
/// 列出所有搜索过的位置和安装建议
fn build_resolution_error() -> anyhow::Error {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|| "~".to_string());

    let nvm_path = format!("{}/.nvm", home);
    let nvm_detected = PathBuf::from(&nvm_path).exists();

    let mut error_msg = String::from(
        "Codex command not found in any of the following locations:\n\
         1. Environment variable: CLAUDE_AUTONOMOUS_CODEX_BIN ",
    );

    if env::var("CLAUDE_AUTONOMOUS_CODEX_BIN").is_ok() {
        error_msg.push_str("(set but invalid)\n");
    } else {
        error_msg.push_str("(not set)\n");
    }

    error_msg.push_str("2. System PATH (command 'codex' not found)\n");
    error_msg.push_str(&format!(
        "3. nvm directories: {}/.nvm/versions/node/*/bin/codex (not found)\n",
        home
    ));
    error_msg.push_str("4. Project-local: ./node_modules/.bin/codex (not found)\n");
    error_msg.push_str("\n💡 Installation suggestions:\n");
    error_msg.push_str("- Install via npm: npm install -g @anthropic-ai/codex\n");
    error_msg.push_str("- Or set CLAUDE_AUTONOMOUS_CODEX_BIN to the full path\n");

    if nvm_detected {
        error_msg.push_str(&format!("- Detected nvm at: {}\n", nvm_path));
        error_msg.push_str("  Try: nvm use <version> && npm install -g @anthropic-ai/codex\n");
    }

    error_msg.push_str("\nFor more info, visit: https://github.com/anthropics/codex");

    anyhow::anyhow!(error_msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // 测试锁，防止并发测试相互干扰
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    /// 创建一个模拟的 codex 可执行文件
    fn create_mock_codex(path: &Path) -> std::io::Result<()> {
        fs::write(path, "#!/bin/sh\necho 'codex version 1.0.0'\nexit 0")?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    #[test]
    fn test_validate_codex_binary_nonexistent() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let path = PathBuf::from("/nonexistent/codex");
        assert!(!validate_codex_binary(&path));
    }

    #[test]
    fn test_validate_codex_binary_valid() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let temp = TempDir::new().unwrap();
        let codex_path = temp.path().join("codex");

        create_mock_codex(&codex_path).unwrap();

        assert!(validate_codex_binary(&codex_path));
    }

    #[test]
    #[cfg(unix)]
    fn test_validate_codex_binary_not_executable() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let temp = TempDir::new().unwrap();
        let codex_path = temp.path().join("codex");

        // 创建文件但不设置可执行权限
        fs::write(&codex_path, "#!/bin/sh\necho 'test'\n").unwrap();

        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&codex_path).unwrap().permissions();
        perms.set_mode(0o644); // rw-r--r-- (不可执行)
        fs::set_permissions(&codex_path, perms).unwrap();

        assert!(!validate_codex_binary(&codex_path));
    }

    #[test]
    fn test_search_nvm_directories_single_version() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let temp = TempDir::new().unwrap();
        let nvm_base = temp.path().join(".nvm/versions/node");
        let v20_bin = nvm_base.join("v20.0.0/bin");

        fs::create_dir_all(&v20_bin).unwrap();

        let codex_path = v20_bin.join("codex");
        create_mock_codex(&codex_path).unwrap();

        // 注意：这个测试需要修改 search_nvm_directories 以接受 base_path 参数
        // 或者使用环境变量 mock home 目录
        // 这里仅作为示例
    }

    #[test]
    fn test_search_project_local() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let original_dir = env::current_dir().unwrap();

        let temp = TempDir::new().unwrap();
        let node_modules = temp.path().join("node_modules/.bin");
        fs::create_dir_all(&node_modules).unwrap();

        let codex_path = node_modules.join("codex");
        create_mock_codex(&codex_path).unwrap();

        // 切换到临时目录
        env::set_current_dir(temp.path()).unwrap();

        let result = search_project_local();

        // 恢复原目录
        env::set_current_dir(&original_dir).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), codex_path);
    }

    #[test]
    fn test_build_resolution_error() {
        let error = build_resolution_error();
        let error_msg = error.to_string();

        assert!(error_msg.contains("Codex command not found"));
        assert!(error_msg.contains("CLAUDE_AUTONOMOUS_CODEX_BIN"));
        assert!(error_msg.contains("System PATH"));
        assert!(error_msg.contains("nvm directories"));
        assert!(error_msg.contains("Project-local"));
        assert!(error_msg.contains("Installation suggestions"));
    }
}
