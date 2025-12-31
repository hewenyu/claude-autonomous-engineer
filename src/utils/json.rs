//! JSON 工具

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::utils::write_file;

/// 读取 JSON 文件
pub fn read_json<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read JSON file: {}", path.display()))?;

    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse JSON file: {}", path.display()))
}

/// 写入 JSON 文件（格式化）
pub fn write_json<T>(path: &Path, data: &T) -> Result<()>
where
    T: Serialize,
{
    let json = serde_json::to_string_pretty(data).context("Failed to serialize to JSON")?;

    // Use atomic write to avoid corrupting state files (e.g. memory.json) during long-running loops.
    write_file(path, &json).with_context(|| format!("Failed to write JSON file: {}", path.display()))
}

/// 读取 JSON 文件，失败时返回默认值
pub fn read_json_or_default<T>(path: &Path) -> T
where
    T: for<'de> Deserialize<'de> + Default,
{
    read_json(path).unwrap_or_default()
}

/// 尝试读取 JSON 文件，失败时返回 None
pub fn try_read_json<T>(path: &Path) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    read_json(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_read_write_json() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.json");

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // 写入
        write_json(&file_path, &data).unwrap();

        // 读取
        let loaded: TestData = read_json(&file_path).unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_read_json_or_default() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("nonexistent.json");

        #[derive(Default, Deserialize)]
        struct DefaultData {
            value: i32,
        }

        let data: DefaultData = read_json_or_default(&file_path);
        assert_eq!(data.value, 0);
    }
}
