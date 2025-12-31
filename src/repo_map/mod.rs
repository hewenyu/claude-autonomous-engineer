//! Repository Mapping - 代码库结构提取
//!
//! 使用 Tree-sitter 解析源代码，提取函数签名、结构体定义等骨架信息，
//! 减少 90% 的 token 消耗，同时保持代码结构的完整性。

pub mod cache;
pub mod extractor;
pub mod generator;
pub mod languages;
pub mod parser;

use anyhow::Result;
use std::path::{Path, PathBuf};

/// 代码符号（函数、结构体、impl 块等）
#[derive(Debug, Clone)]
pub struct Symbol {
    pub kind: SymbolKind,
    pub name: String,
    pub signature: String,
    pub line_start: usize,
    pub line_end: usize,
}

/// 符号类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    Module,
    Type,
}

/// 文件符号信息
#[derive(Debug, Clone)]
pub struct FileSymbols {
    pub file_path: PathBuf,
    pub language: String,
    pub symbols: Vec<Symbol>,
    pub hash: String, // BLAKE3 hash
}

/// Repository Map 生成器
pub struct RepoMapper {
    project_root: PathBuf,
    cache: cache::FileHashCache,
}

impl RepoMapper {
    /// 创建新的 RepoMapper
    pub fn new(project_root: impl AsRef<Path>) -> Result<Self> {
        let project_root = project_root.as_ref().to_path_buf();
        let cache = cache::FileHashCache::load(&project_root)?;

        Ok(Self {
            project_root,
            cache,
        })
    }

    /// 生成完整的 Repository Map
    pub fn generate_map(&mut self) -> Result<String> {
        let files = self.find_source_files()?;
        let mut all_symbols = Vec::new();

        for file in files {
            if let Some(symbols) = self.extract_file_symbols(&file)? {
                all_symbols.push(symbols);
            }
        }

        // 保存缓存
        self.cache.save(&self.project_root)?;

        // 生成 Markdown
        generator::generate_markdown(&all_symbols)
    }

    /// 查找所有源代码文件
    fn find_source_files(&self) -> Result<Vec<PathBuf>> {
        use ignore::WalkBuilder;
        use rayon::prelude::*;

        let walker = WalkBuilder::new(&self.project_root)
            .hidden(false)
            .git_ignore(true)
            .build();

        let files: Vec<PathBuf> = walker
            .filter_map(|entry| entry.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect::<Vec<_>>()
            .into_par_iter()
            .filter(|path| self.is_supported_language(path))
            .collect();

        Ok(files)
    }

    /// 检查文件是否是支持的语言
    fn is_supported_language(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            matches!(ext.to_str(), Some("rs") | Some("py") | Some("ts"))
        } else {
            false
        }
    }

    /// 提取文件符号
    fn extract_file_symbols(&mut self, file_path: &Path) -> Result<Option<FileSymbols>> {
        // 计算文件哈希
        let content = std::fs::read(file_path)?;
        let hash = cache::compute_hash(&content);

        // 检查缓存
        if let Some(cached) = self.cache.get(file_path, &hash) {
            return Ok(Some(cached));
        }

        // 解析文件
        let language = self.detect_language(file_path)?;
        let extractor = extractor::get_extractor(&language)?;

        let source = String::from_utf8_lossy(&content).to_string();
        let symbols = extractor.extract_symbols(&source)?;

        let file_symbols = FileSymbols {
            file_path: file_path.to_path_buf(),
            language,
            symbols,
            hash: hash.clone(),
        };

        // 更新缓存
        self.cache.insert(file_path, hash, file_symbols.clone());

        Ok(Some(file_symbols))
    }

    /// 检测文件语言
    fn detect_language(&self, path: &Path) -> Result<String> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| anyhow::anyhow!("No file extension"))?;

        Ok(match ext {
            "rs" => "rust",
            "py" => "python",
            "ts" => "typescript",
            _ => anyhow::bail!("Unsupported language: {}", ext),
        }
        .to_string())
    }
}
