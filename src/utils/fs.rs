//! 文件系统工具

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// 安全读取文件内容
pub fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))
}

/// 尝试读取文件，失败时返回 None
pub fn try_read_file(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// 安全写入文件
pub fn write_file(path: &Path, content: &str) -> Result<()> {
    // 确保父目录存在
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    fs::write(path, content)
        .with_context(|| format!("Failed to write file: {}", path.display()))
}

/// 追加内容到文件
pub fn append_file(path: &Path, content: &str) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;

    // 确保父目录存在
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open file for append: {}", path.display()))?;

    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to append to file: {}", path.display()))
}

/// 检查文件是否存在
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// 检查目录是否存在
pub fn dir_exists(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

/// 创建目录（包括父目录）
pub fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))
}

/// 获取文件大小
pub fn file_size(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to get file metadata: {}", path.display()))?;
    Ok(metadata.len())
}

/// 计算文件 MD5 hash
pub fn file_hash(path: &Path) -> Result<String> {
    let content = read_file(path)?;
    Ok(format!("{:x}", md5::compute(content.as_bytes())))
}

/// 计算字符串 MD5 hash（短版本，8 字符）
pub fn content_hash_short(content: &str) -> String {
    let hash = format!("{:x}", md5::compute(content.as_bytes()));
    hash[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_write_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        let content = "Hello, World!";
        write_file(&file_path, content).unwrap();

        let loaded = read_file(&file_path).unwrap();
        assert_eq!(loaded, content);
    }

    #[test]
    fn test_append_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        write_file(&file_path, "Line 1\n").unwrap();
        append_file(&file_path, "Line 2\n").unwrap();

        let content = read_file(&file_path).unwrap();
        assert_eq!(content, "Line 1\nLine 2\n");
    }

    #[test]
    fn test_file_exists() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.txt");

        assert!(!file_exists(&file_path));
        write_file(&file_path, "test").unwrap();
        assert!(file_exists(&file_path));
    }

    #[test]
    fn test_content_hash() {
        let hash1 = content_hash_short("test");
        let hash2 = content_hash_short("test");
        let hash3 = content_hash_short("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 8);
    }
}
