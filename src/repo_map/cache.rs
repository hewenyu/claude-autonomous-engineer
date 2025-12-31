//! 文件哈希缓存 - 避免重复解析未修改的文件

use super::FileSymbols;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 缓存条目
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    hash: String,
    symbols: Vec<CachedSymbol>,
    language: String,
}

/// 缓存的符号（简化版，用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSymbol {
    kind: String,
    name: String,
    signature: String,
    line_start: usize,
    line_end: usize,
}

/// 文件哈希缓存
#[derive(Debug)]
pub struct FileHashCache {
    cache: HashMap<PathBuf, CacheEntry>,
    dirty: bool, // 是否有修改需要保存
}

impl FileHashCache {
    /// 加载缓存文件
    pub fn load(project_root: &Path) -> Result<Self> {
        let cache_file = project_root.join(".claude/repo_map/cache.json");

        if cache_file.exists() {
            let content = std::fs::read_to_string(&cache_file)?;
            let cache: HashMap<PathBuf, CacheEntry> = serde_json::from_str(&content)?;
            Ok(Self {
                cache,
                dirty: false,
            })
        } else {
            Ok(Self {
                cache: HashMap::new(),
                dirty: false,
            })
        }
    }

    /// 保存缓存文件
    pub fn save(&mut self, project_root: &Path) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let cache_dir = project_root.join(".claude/repo_map");
        std::fs::create_dir_all(&cache_dir)?;

        let cache_file = cache_dir.join("cache.json");
        let content = serde_json::to_string_pretty(&self.cache)?;
        std::fs::write(cache_file, content)?;

        // 成功写入后清理 dirty 标记，避免每次都重复写盘
        self.dirty = false;

        Ok(())
    }

    /// 获取缓存的符号
    pub fn get(&self, file_path: &Path, hash: &str) -> Option<FileSymbols> {
        self.cache.get(file_path).and_then(|entry| {
            if entry.hash == hash {
                Some(FileSymbols {
                    file_path: file_path.to_path_buf(),
                    language: entry.language.clone(),
                    symbols: entry
                        .symbols
                        .iter()
                        .map(|s| super::Symbol {
                            kind: parse_symbol_kind(&s.kind),
                            name: s.name.clone(),
                            signature: s.signature.clone(),
                            line_start: s.line_start,
                            line_end: s.line_end,
                        })
                        .collect(),
                    hash: entry.hash.clone(),
                })
            } else {
                None
            }
        })
    }

    /// 插入缓存条目
    pub fn insert(&mut self, file_path: &Path, hash: String, file_symbols: FileSymbols) {
        let entry = CacheEntry {
            hash,
            language: file_symbols.language,
            symbols: file_symbols
                .symbols
                .iter()
                .map(|s| CachedSymbol {
                    kind: format!("{:?}", s.kind),
                    name: s.name.clone(),
                    signature: s.signature.clone(),
                    line_start: s.line_start,
                    line_end: s.line_end,
                })
                .collect(),
        };

        self.cache.insert(file_path.to_path_buf(), entry);
        self.dirty = true;
    }

    /// 清除缓存
    pub fn clear(&mut self) {
        self.cache.clear();
        self.dirty = true;
    }
}

/// 计算文件的 BLAKE3 哈希
pub fn compute_hash(content: &[u8]) -> String {
    let hash = blake3::hash(content);
    hash.to_hex().to_string()
}

/// 解析符号类型字符串
fn parse_symbol_kind(kind: &str) -> super::SymbolKind {
    match kind {
        "Function" => super::SymbolKind::Function,
        "Struct" => super::SymbolKind::Struct,
        "Enum" => super::SymbolKind::Enum,
        "Trait" => super::SymbolKind::Trait,
        "Impl" => super::SymbolKind::Impl,
        "Const" => super::SymbolKind::Const,
        "Module" => super::SymbolKind::Module,
        "Type" => super::SymbolKind::Type,
        _ => super::SymbolKind::Function, // 默认
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let content = b"fn main() {}";
        let hash1 = compute_hash(content);
        let hash2 = compute_hash(content);
        assert_eq!(hash1, hash2);

        let different = b"fn other() {}";
        let hash3 = compute_hash(different);
        assert_ne!(hash1, hash3);
    }
}
