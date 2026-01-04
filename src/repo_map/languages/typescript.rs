//! TypeScript/JavaScript 语言提取器
//!
//! 使用 tree-sitter-typescript 解析 TypeScript/TSX 代码
//! 提取函数、类、接口、React 组件等骨架信息

use crate::repo_map::extractor::LanguageExtractor;
use crate::repo_map::parser::{find_child_by_kind, node_text, parse_source};
use crate::repo_map::{Symbol, SymbolKind};
use anyhow::Result;
use tree_sitter::Node;

/// TypeScript 提取器
pub struct TypeScriptExtractor {
    language: tree_sitter::Language,
}

impl TypeScriptExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            language: tree_sitter_typescript::language_typescript(),
        })
    }

    /// 创建 TSX 提取器
    pub fn new_tsx() -> Result<Self> {
        Ok(Self {
            language: tree_sitter_typescript::language_tsx(),
        })
    }

    /// 提取函数声明
    fn extract_function(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "identifier")?;
        let name = node_text(&name_node, source).to_string();

        let signature = self.build_function_signature(node, source, &name);

        Some(Symbol {
            kind: SymbolKind::Function,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 构建函数签名
    fn build_function_signature(&self, node: &Node, source: &str, name: &str) -> String {
        let mut parts = Vec::new();

        // 检查 export
        if self.has_export_modifier(node) {
            parts.push("export".to_string());
        }

        // 检查 async
        if self.has_async_modifier(node, source) {
            parts.push("async".to_string());
        }

        // function 关键字
        parts.push("function".to_string());
        parts.push(name.to_string());

        // 类型参数
        if let Some(type_params) = find_child_by_kind(node, "type_parameters") {
            parts.push(node_text(&type_params, source).to_string());
        }

        // 参数列表
        if let Some(params) = find_child_by_kind(node, "formal_parameters") {
            parts.push(node_text(&params, source).to_string());
        }

        // 返回类型
        if let Some(return_type) = find_child_by_kind(node, "type_annotation") {
            parts.push(node_text(&return_type, source).to_string());
        }

        format!("{};", parts.join(" "))
    }

    /// 提取箭头函数（变量声明中的）
    fn extract_arrow_function(&self, node: &Node, source: &str) -> Option<Symbol> {
        // 查找变量声明中的名称
        let declarator = node.parent()?;
        let name_node = find_child_by_kind(&declarator, "identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut parts = Vec::new();

        // const/let/var
        if let Some(parent) = declarator.parent() {
            if parent.kind() == "lexical_declaration" {
                let mut cursor = parent.walk();
                for child in parent.children(&mut cursor) {
                    if child.kind() == "const" || child.kind() == "let" || child.kind() == "var" {
                        parts.push(node_text(&child, source).to_string());
                        break;
                    }
                }
            }
        }

        parts.push(name.clone());
        parts.push("=".to_string());

        // async
        if self.has_async_modifier(node, source) {
            parts.push("async".to_string());
        }

        // 参数
        if let Some(params) = find_child_by_kind(node, "formal_parameters") {
            parts.push(node_text(&params, source).to_string());
        }

        // 返回类型
        if let Some(return_type) = find_child_by_kind(node, "type_annotation") {
            parts.push(node_text(&return_type, source).to_string());
        }

        parts.push("=>".to_string());
        parts.push("{ ... }".to_string());

        Some(Symbol {
            kind: SymbolKind::Function,
            name,
            signature: format!("{};", parts.join(" ")),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取类定义
    fn extract_class(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "type_identifier")
            .or_else(|| find_child_by_kind(node, "identifier"))?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // export
        if self.has_export_modifier(node) {
            signature.push_str("export ");
        }

        // abstract
        if self.has_modifier(node, "abstract", source) {
            signature.push_str("abstract ");
        }

        signature.push_str("class ");
        signature.push_str(&name);

        // 类型参数
        if let Some(type_params) = find_child_by_kind(node, "type_parameters") {
            signature.push_str(node_text(&type_params, source));
        }

        // 继承
        if let Some(heritage) = find_child_by_kind(node, "class_heritage") {
            signature.push(' ');
            signature.push_str(node_text(&heritage, source).trim());
        }

        signature.push_str(" { ... }");

        Some(Symbol {
            kind: SymbolKind::Struct,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取接口定义
    fn extract_interface(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // export
        if self.has_export_modifier(node) {
            signature.push_str("export ");
        }

        signature.push_str("interface ");
        signature.push_str(&name);

        // 类型参数
        if let Some(type_params) = find_child_by_kind(node, "type_parameters") {
            signature.push_str(node_text(&type_params, source));
        }

        // 继承
        if let Some(extends) = find_child_by_kind(node, "extends_clause") {
            signature.push(' ');
            signature.push_str(node_text(&extends, source).trim());
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

    /// 提取类型别名
    fn extract_type_alias(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // export
        if self.has_export_modifier(node) {
            signature.push_str("export ");
        }

        signature.push_str("type ");
        signature.push_str(&name);

        // 类型参数
        if let Some(type_params) = find_child_by_kind(node, "type_parameters") {
            signature.push_str(node_text(&type_params, source));
        }

        signature.push_str(" = ...");

        Some(Symbol {
            kind: SymbolKind::Type,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取方法定义
    fn extract_method(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "property_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut parts = Vec::new();

        // 访问修饰符
        if self.has_modifier(node, "public", source) {
            parts.push("public".to_string());
        } else if self.has_modifier(node, "private", source) {
            parts.push("private".to_string());
        } else if self.has_modifier(node, "protected", source) {
            parts.push("protected".to_string());
        }

        // static
        if self.has_modifier(node, "static", source) {
            parts.push("static".to_string());
        }

        // async
        if self.has_async_modifier(node, source) {
            parts.push("async".to_string());
        }

        parts.push(name.clone());

        // 参数
        if let Some(params) = find_child_by_kind(node, "formal_parameters") {
            parts.push(node_text(&params, source).to_string());
        }

        // 返回类型
        if let Some(return_type) = find_child_by_kind(node, "type_annotation") {
            parts.push(node_text(&return_type, source).to_string());
        }

        Some(Symbol {
            kind: SymbolKind::Function,
            name,
            signature: format!("{};", parts.join(" ")),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 检查是否有 export 修饰符
    fn has_export_modifier(&self, node: &Node) -> bool {
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return true;
            }
        }
        false
    }

    /// 检查是否有特定修饰符
    fn has_modifier(&self, node: &Node, modifier: &str, source: &str) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if node_text(&child, source).trim() == modifier {
                return true;
            }
        }
        false
    }

    /// 检查是否有 async 修饰符
    fn has_async_modifier(&self, node: &Node, source: &str) -> bool {
        self.has_modifier(node, "async", source)
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn extract_symbols(&self, source: &str) -> Result<Vec<Symbol>> {
        let tree = parse_source(source, self.language.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse TypeScript source"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();

        self.walk_tree(&root, source, &mut symbols);

        Ok(symbols)
    }

    fn language_name(&self) -> &str {
        "typescript"
    }
}

impl TypeScriptExtractor {
    /// 递归遍历 AST
    fn walk_tree(&self, node: &Node, source: &str, symbols: &mut Vec<Symbol>) {
        match node.kind() {
            "function_declaration" => {
                if let Some(symbol) = self.extract_function(node, source) {
                    symbols.push(symbol);
                }
            }
            "arrow_function" => {
                if let Some(symbol) = self.extract_arrow_function(node, source) {
                    symbols.push(symbol);
                }
            }
            "class_declaration" => {
                if let Some(symbol) = self.extract_class(node, source) {
                    symbols.push(symbol);
                }
            }
            "interface_declaration" => {
                if let Some(symbol) = self.extract_interface(node, source) {
                    symbols.push(symbol);
                }
            }
            "type_alias_declaration" => {
                if let Some(symbol) = self.extract_type_alias(node, source) {
                    symbols.push(symbol);
                }
            }
            "method_definition" => {
                if let Some(symbol) = self.extract_method(node, source) {
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
function greet(name: string): string {
    return `Hello, ${name}!`;
}
"#;

        let extractor = TypeScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].name, "greet");
    }

    #[test]
    fn test_extract_arrow_function() {
        let source = r#"
const add = (a: number, b: number): number => {
    return a + b;
};
"#;

        let extractor = TypeScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "add");
    }

    #[test]
    fn test_extract_class() {
        let source = r#"
export class User {
    constructor(public name: string) {}
}
"#;

        let extractor = TypeScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert!(symbols.iter().any(|s| s.name == "User"));
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"
interface UserData {
    id: number;
    name: string;
}
"#;

        let extractor = TypeScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Trait);
        assert_eq!(symbols[0].name, "UserData");
    }

    #[test]
    fn test_extract_type_alias() {
        let source = r#"
type UserId = string | number;
"#;

        let extractor = TypeScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Type);
        assert_eq!(symbols[0].name, "UserId");
    }
}
