//! Markdown 生成器 - 将提取的符号转换为易读的 Markdown 格式

use super::FileSymbols;
use anyhow::Result;
use chrono::Utc;

/// 生成 Repository Map 的 Markdown 内容
pub fn generate_markdown(all_symbols: &[FileSymbols]) -> Result<String> {
    let mut output = String::new();

    // 头部
    output.push_str("# Repository Structure Map\n\n");
    output.push_str(&format!(
        "Generated: {}\n\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S")
    ));

    // 统计信息
    let total_files = all_symbols.len();
    let total_symbols: usize = all_symbols.iter().map(|f| f.symbols.len()).sum();
    output.push_str(&format!(
        "Files: {} | Symbols: {}\n\n",
        total_files, total_symbols
    ));

    output.push_str("---\n\n");

    // 按文件组织
    for file_symbols in all_symbols {
        let relative_path = file_symbols.file_path.to_string_lossy().replace("\\", "/");

        output.push_str(&format!("## {}\n\n", relative_path));

        if file_symbols.symbols.is_empty() {
            output.push_str("*No symbols found*\n\n");
            continue;
        }

        // 按类型分组
        let functions: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == super::SymbolKind::Function)
            .collect();

        let structs: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == super::SymbolKind::Struct)
            .collect();

        let enums: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == super::SymbolKind::Enum)
            .collect();

        let traits: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == super::SymbolKind::Trait)
            .collect();

        let impls: Vec<_> = file_symbols
            .symbols
            .iter()
            .filter(|s| s.kind == super::SymbolKind::Impl)
            .collect();

        // 输出结构体
        if !structs.is_empty() {
            output.push_str("### Structs\n\n");
            output.push_str("```rust\n");
            for symbol in structs {
                output.push_str(&format!("// Line {}\n", symbol.line_start));
                output.push_str(&format!("{}\n\n", symbol.signature));
            }
            output.push_str("```\n\n");
        }

        // 输出枚举
        if !enums.is_empty() {
            output.push_str("### Enums\n\n");
            output.push_str("```rust\n");
            for symbol in enums {
                output.push_str(&format!("// Line {}\n", symbol.line_start));
                output.push_str(&format!("{}\n\n", symbol.signature));
            }
            output.push_str("```\n\n");
        }

        // 输出 Trait
        if !traits.is_empty() {
            output.push_str("### Traits\n\n");
            output.push_str("```rust\n");
            for symbol in traits {
                output.push_str(&format!("// Line {}\n", symbol.line_start));
                output.push_str(&format!("{}\n\n", symbol.signature));
            }
            output.push_str("```\n\n");
        }

        // 输出函数
        if !functions.is_empty() {
            output.push_str("### Functions\n\n");
            output.push_str("```rust\n");
            for symbol in functions {
                output.push_str(&format!("// Line {}\n", symbol.line_start));
                output.push_str(&format!("{}\n", symbol.signature));
            }
            output.push_str("```\n\n");
        }

        // 输出 Impl 块
        if !impls.is_empty() {
            output.push_str("### Implementations\n\n");
            output.push_str("```rust\n");
            for symbol in impls {
                output.push_str(&format!("// Line {}\n", symbol.line_start));
                output.push_str(&format!("{}\n\n", symbol.signature));
            }
            output.push_str("```\n\n");
        }

        output.push_str("---\n\n");
    }

    Ok(output)
}
