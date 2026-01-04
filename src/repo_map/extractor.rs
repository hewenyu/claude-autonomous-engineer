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
        "python" => Ok(Box::new(super::languages::python::PythonExtractor::new()?)),
        "go" => Ok(Box::new(super::languages::go::GoExtractor::new()?)),
        "typescript" => Ok(Box::new(
            super::languages::typescript::TypeScriptExtractor::new()?,
        )),
        "tsx" => Ok(Box::new(
            super::languages::typescript::TypeScriptExtractor::new_tsx()?,
        )),
        "javascript" => Ok(Box::new(
            super::languages::javascript::JavaScriptExtractor::new()?,
        )),
        "jsx" => Ok(Box::new(
            super::languages::javascript::JavaScriptExtractor::new()?,
        )),
        _ => anyhow::bail!("Unsupported language: {}", language),
    }
}
