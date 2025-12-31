//! Rust 语言提取器
//!
//! 使用 tree-sitter-rust 解析 Rust 代码，提取函数签名、结构体、枚举等骨架信息

use crate::repo_map::extractor::LanguageExtractor;
use crate::repo_map::parser::{find_child_by_kind, node_text, parse_source};
use crate::repo_map::{Symbol, SymbolKind};
use anyhow::Result;
use tree_sitter::Node;

/// Rust 提取器
pub struct RustExtractor {
    language: tree_sitter::Language,
}

impl RustExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            language: tree_sitter_rust::language(),
        })
    }

    fn find_visibility_modifier(&self, node: &Node, source: &str) -> Option<String> {
        if let Some(vis) = find_child_by_kind(node, "visibility_modifier") {
            return Some(node_text(&vis, source).to_string());
        }

        // 兼容：某些节点上 visibility 可能被解析为兄弟节点（极少数情况）
        self.find_sibling_before(node, "visibility_modifier", source)
    }

    fn has_modifier_token(&self, node: &Node, modifier: &str, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if node_text(&child, source).trim() == modifier {
                return true;
            }
        }
        false
    }

    /// 提取函数签名
    fn extract_function(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "identifier")?;
        let name = node_text(&name_node, source).to_string();

        // 提取完整签名（包括 pub、参数、返回类型）
        let signature = self.build_function_signature(node, source);

        Some(Symbol {
            kind: SymbolKind::Function,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 构建函数签名字符串
    fn build_function_signature(&self, node: &Node, source: &str) -> String {
        let mut parts = Vec::new();

        // 检查可见性
        if let Some(vis) = self.find_visibility_modifier(node, source) {
            parts.push(vis);
        }

        // async/const/unsafe
        for modifier in ["async", "const", "unsafe"] {
            if self.has_modifier_token(node, modifier, source) {
                parts.push(modifier.to_string());
            }
        }

        // fn 关键字 + 函数名
        if let Some(name_node) = find_child_by_kind(node, "identifier") {
            let name = node_text(&name_node, source);
            parts.push(format!("fn {}", name));
        }

        // 泛型参数
        if let Some(generics_node) = find_child_by_kind(node, "type_parameters") {
            parts.push(node_text(&generics_node, source).to_string());
        }

        // 参数列表
        if let Some(params_node) = find_child_by_kind(node, "parameters") {
            parts.push(node_text(&params_node, source).to_string());
        }

        // 返回类型
        if let Some(return_node) = find_child_by_kind(node, "return_type") {
            let return_text = node_text(&return_node, source);
            parts.push(return_text.to_string());
        }

        // where 子句
        if let Some(where_node) = find_child_by_kind(node, "where_clause") {
            parts.push(node_text(&where_node, source).to_string());
        }

        format!("{};", parts.join(" "))
    }

    /// 提取结构体
    fn extract_struct(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // 可见性
        if let Some(vis) = self.find_visibility_modifier(node, source) {
            signature.push_str(&vis);
            signature.push(' ');
        }

        // struct 关键字 + 名称
        signature.push_str("struct ");
        signature.push_str(&name);

        // 泛型参数
        if let Some(generics) = find_child_by_kind(node, "type_parameters") {
            signature.push_str(node_text(&generics, source));
        }

        // where 子句
        if let Some(where_clause) = find_child_by_kind(node, "where_clause") {
            signature.push(' ');
            signature.push_str(node_text(&where_clause, source));
        }

        // 字段 (简化版)
        if let Some(_body) = find_child_by_kind(node, "field_declaration_list") {
            signature.push_str(" { ... }");
        } else if node.child_count() > 2 {
            signature.push_str(" ( ... )");
        } else {
            signature.push(';');
        }

        Some(Symbol {
            kind: SymbolKind::Struct,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取枚举
    fn extract_enum(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // 可见性
        if let Some(vis) = self.find_visibility_modifier(node, source) {
            signature.push_str(&vis);
            signature.push(' ');
        }

        signature.push_str("enum ");
        signature.push_str(&name);

        // 泛型参数
        if let Some(generics) = find_child_by_kind(node, "type_parameters") {
            signature.push_str(node_text(&generics, source));
        }

        signature.push_str(" { ... }");

        Some(Symbol {
            kind: SymbolKind::Enum,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取 Trait
    fn extract_trait(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // 可见性
        if let Some(vis) = self.find_visibility_modifier(node, source) {
            signature.push_str(&vis);
            signature.push(' ');
        }

        // unsafe
        if self.has_modifier_token(node, "unsafe", source) {
            signature.push_str("unsafe ");
        }

        signature.push_str("trait ");
        signature.push_str(&name);

        // 泛型参数
        if let Some(generics) = find_child_by_kind(node, "type_parameters") {
            signature.push_str(node_text(&generics, source));
        }

        signature.push_str(" { ... }");

        Some(Symbol {
            kind: SymbolKind::Trait,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取 impl 块
    fn extract_impl(&self, node: &Node, source: &str) -> Option<Symbol> {
        let type_node = find_child_by_kind(node, "type_identifier")
            .or_else(|| find_child_by_kind(node, "generic_type"))?;
        let type_name = node_text(&type_node, source).to_string();

        let mut signature = String::new();

        // unsafe
        if self.has_modifier_token(node, "unsafe", source) {
            signature.push_str("unsafe ");
        }

        signature.push_str("impl");

        // 泛型参数
        if let Some(generics) = find_child_by_kind(node, "type_parameters") {
            signature.push(' ');
            signature.push_str(node_text(&generics, source));
        }

        signature.push(' ');

        // Trait (如果是 trait impl)
        if let Some(trait_node) = find_child_by_kind(node, "trait") {
            signature.push_str(node_text(&trait_node, source));
            signature.push_str(" for ");
        }

        signature.push_str(&type_name);

        signature.push_str(" { ... }");

        Some(Symbol {
            kind: SymbolKind::Impl,
            name: type_name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 查找节点之前的兄弟节点（用于可见性修饰符等）
    fn find_sibling_before(&self, node: &Node, kind: &str, source: &str) -> Option<String> {
        let parent = node.parent()?;
        let mut cursor = parent.walk();

        for child in parent.children(&mut cursor) {
            if child.id() == node.id() {
                break;
            }
            if child.kind() == kind {
                return Some(node_text(&child, source).to_string());
            }
        }
        None
    }

    /// 检查节点之前是否有特定修饰符
    // NOTE: 旧实现曾尝试从“父节点兄弟”里寻找修饰符；在 tree-sitter-rust 语法中，
    // 大多数修饰符是 function_item/impl_item 的子节点，因此已由 `has_modifier_token` 覆盖。
}

impl LanguageExtractor for RustExtractor {
    fn extract_symbols(&self, source: &str) -> Result<Vec<Symbol>> {
        let tree = parse_source(source, self.language.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Rust source"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();

        self.walk_tree(&root, source, &mut symbols);

        Ok(symbols)
    }

    fn language_name(&self) -> &str {
        "rust"
    }
}

impl RustExtractor {
    /// 递归遍历 AST
    fn walk_tree(&self, node: &Node, source: &str, symbols: &mut Vec<Symbol>) {
        match node.kind() {
            "function_item" => {
                if let Some(symbol) = self.extract_function(node, source) {
                    symbols.push(symbol);
                }
            }
            "struct_item" => {
                if let Some(symbol) = self.extract_struct(node, source) {
                    symbols.push(symbol);
                }
            }
            "enum_item" => {
                if let Some(symbol) = self.extract_enum(node, source) {
                    symbols.push(symbol);
                }
            }
            "trait_item" => {
                if let Some(symbol) = self.extract_trait(node, source) {
                    symbols.push(symbol);
                }
            }
            "impl_item" => {
                if let Some(symbol) = self.extract_impl(node, source) {
                    symbols.push(symbol);
                }
            }
            _ => {}
        }

        // 递归处理子节点
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_tree(&child, source, symbols);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_function() {
        let source = r#"
pub fn hello(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#;

        let extractor = RustExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].name, "hello");
        assert!(symbols[0].signature.contains("pub"));
        assert!(symbols[0].signature.contains("fn hello"));
    }

    #[test]
    fn test_extract_struct() {
        let source = r#"
pub struct User {
    name: String,
    age: u32,
}
"#;

        let extractor = RustExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].name, "User");
    }
}
