//! 语言提取器 - 为不同语言提供统一的符号提取接口

use super::Symbol;
use anyhow::Result;

/// 语言提取器 trait
pub trait LanguageExtractor: Send + Sync {
    /// 从源代码中提取符号
    fn extract_symbols(&self, source: &str) -> Result<Vec<Symbol>>;

    /// 语言名称
    fn language_name(&self) -> &str;
}

/// 获取指定语言的提取器
pub fn get_extractor(language: &str) -> Result<Box<dyn LanguageExtractor>> {
    match language {
        "rust" => Ok(Box::new(super::languages::rust::RustExtractor::new()?)),
        _ => anyhow::bail!("Unsupported language: {}", language),
    }
}
