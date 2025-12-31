//! TOON 格式生成器 - 将提取的符号转换为 Token-Oriented Object Notation
//!
//! TOON 格式优势：
//! - 减少 30-60% 的 token 消耗
//! - 表格化结构更利于 LLM 理解
//! - 人类可读性强

use super::{FileSymbols, Symbol, SymbolKind};
use anyhow::Result;
use chrono::Utc;

/// 生成 Repository Map 的 TOON 格式内容
pub fn generate_toon(all_symbols: &[FileSymbols]) -> Result<String> {
    let mut output = String::new();

    // 头部元数据（使用 YAML 风格）
    output.push_str("# Repository Structure Map (TOON Format)\n");
    output.push_str(&format!(
        "generated: {}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    // 统计信息
    let total_files = all_symbols.len();
    let total_symbols: usize = all_symbols.iter().map(|f| f.symbols.len()).sum();
    output.push_str(&format!("total_files: {}\n", total_files));
    output.push_str(&format!("total_symbols: {}\n\n", total_symbols));

    // 文件数组 - 使用 TOON 表格格式
    if all_symbols.is_empty() {
        output.push_str("files[0]:\n");
        return Ok(output);
    }

    output.push_str(&format!("files[{}]:\n", total_files));

    // 为每个文件生成 TOON 条目
    for (idx, file_symbols) in all_symbols.iter().enumerate() {
        output.push_str(&format!("\n  # File {}\n", idx + 1));
        output.push_str(&format!(
            "  path: {}\n",
            escape_toon_string(&file_symbols.file_path.to_string_lossy())
        ));
        output.push_str(&format!("  language: {}\n", file_symbols.language));
        output.push_str(&format!("  hash: {}\n", file_symbols.hash));

        // 符号数组 - 这里使用 TOON 的核心优势：表格化
        if file_symbols.symbols.is_empty() {
            output.push_str("  symbols[0]:\n");
        } else {
            generate_symbols_table(&mut output, &file_symbols.symbols, "  ")?;
        }
    }

    Ok(output)
}

/// 生成符号表格（TOON 数组格式）
fn generate_symbols_table(output: &mut String, symbols: &[Symbol], indent: &str) -> Result<()> {
    let count = symbols.len();

    // TOON 数组声明：symbols[N]{kind,name,signature,line_start,line_end}:
    output.push_str(&format!(
        "{}symbols[{}]{{kind,name,signature,line_start,line_end}}:\n",
        indent, count
    ));

    // 数据行
    for symbol in symbols {
        let kind_str = format!("{:?}", symbol.kind);
        let name = escape_toon_string(&symbol.name);
        let signature = escape_toon_string(&symbol.signature);

        output.push_str(&format!(
            "{}  {},{},{},{},{}\n",
            indent, kind_str, name, signature, symbol.line_start, symbol.line_end
        ));
    }

    Ok(())
}

/// TOON 字符串转义
///
/// 根据 TOON 规范，以下情况需要加引号：
/// - 包含活动分隔符（逗号、制表符、管道符）
/// - 包含冒号 ':'
/// - 前导/尾随空格
/// - 控制字符
/// - 空字符串
fn escape_toon_string(s: &str) -> String {
    // 检查是否需要引号
    let needs_quotes = s.is_empty()
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.contains(',')
        || s.contains(':')
        || s.contains('\t')
        || s.contains('|')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('"');

    if !needs_quotes {
        return s.to_string();
    }

    // 转义双引号并添加引号
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

/// 按符号类型分组生成（可选的更详细格式）
#[allow(dead_code)]
pub fn generate_toon_grouped(all_symbols: &[FileSymbols]) -> Result<String> {
    let mut output = String::new();

    // 头部元数据
    output.push_str("# Repository Structure Map (TOON Format - Grouped)\n");
    output.push_str(&format!(
        "generated: {}\n\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    output.push_str(&format!("files[{}]:\n", all_symbols.len()));

    // 为每个文件生成分组的符号
    for (idx, file_symbols) in all_symbols.iter().enumerate() {
        output.push_str(&format!("\n  # File {}\n", idx + 1));
        output.push_str(&format!(
            "  path: {}\n",
            escape_toon_string(&file_symbols.file_path.to_string_lossy())
        ));
        output.push_str(&format!("  language: {}\n", file_symbols.language));

        // 按类型分组
        let functions: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();
        let structs: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .collect();
        let enums: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Enum)
            .collect();
        let traits: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Trait)
            .collect();
        let impls: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Impl)
            .collect();

        // 输出每组
        if !structs.is_empty() {
            output.push_str("\n  # Structs\n");
            let structs_owned: Vec<Symbol> = structs.iter().map(|&s| s.clone()).collect();
            generate_symbols_table(&mut output, &structs_owned, "  ")?;
        }

        if !enums.is_empty() {
            output.push_str("\n  # Enums\n");
            let enums_owned: Vec<Symbol> = enums.iter().map(|&s| s.clone()).collect();
            generate_symbols_table(&mut output, &enums_owned, "  ")?;
        }

        if !traits.is_empty() {
            output.push_str("\n  # Traits\n");
            let traits_owned: Vec<Symbol> = traits.iter().map(|&s| s.clone()).collect();
            generate_symbols_table(&mut output, &traits_owned, "  ")?;
        }

        if !functions.is_empty() {
            output.push_str("\n  # Functions\n");
            let functions_owned: Vec<Symbol> = functions.iter().map(|&s| s.clone()).collect();
            generate_symbols_table(&mut output, &functions_owned, "  ")?;
        }

        if !impls.is_empty() {
            output.push_str("\n  # Implementations\n");
            let impls_owned: Vec<Symbol> = impls.iter().map(|&s| s.clone()).collect();
            generate_symbols_table(&mut output, &impls_owned, "  ")?;
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_escape_toon_string() {
        assert_eq!(escape_toon_string("simple"), "simple");
        assert_eq!(escape_toon_string("with,comma"), "\"with,comma\"");
        assert_eq!(escape_toon_string("with:colon"), "\"with:colon\"");
        assert_eq!(escape_toon_string(" leading"), "\" leading\"");
        assert_eq!(escape_toon_string("trailing "), "\"trailing \"");
        assert_eq!(escape_toon_string(""), "\"\"");
        assert_eq!(escape_toon_string("with\"quote"), "\"with\\\"quote\"");
    }

    #[test]
    fn test_generate_toon_basic() {
        let symbols = vec![
            Symbol {
                kind: SymbolKind::Function,
                name: "main".to_string(),
                signature: "fn main()".to_string(),
                line_start: 1,
                line_end: 10,
            },
            Symbol {
                kind: SymbolKind::Struct,
                name: "User".to_string(),
                signature: "struct User { name: String }".to_string(),
                line_start: 12,
                line_end: 15,
            },
        ];

        let file_symbols = FileSymbols {
            file_path: PathBuf::from("src/main.rs"),
            language: "rust".to_string(),
            symbols,
            hash: "abc123".to_string(),
        };

        let result = generate_toon(&[file_symbols]).unwrap();

        // 验证包含关键元素
        assert!(result.contains("files[1]:"));
        assert!(result.contains("path: src/main.rs"));
        assert!(result.contains("language: rust"));
        assert!(result.contains("symbols[2]{kind,name,signature,line_start,line_end}:"));
        assert!(result.contains("Function,main,"));
        assert!(result.contains("Struct,User,"));
    }
}
