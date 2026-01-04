//! JavaScript 语言提取器
//!
//! 使用 tree-sitter-javascript 解析 JavaScript/JSX 代码
//! 提取函数、类、React 组件等骨架信息

use crate::repo_map::extractor::LanguageExtractor;
use crate::repo_map::parser::{find_child_by_kind, node_text, parse_source};
use crate::repo_map::{Symbol, SymbolKind};
use anyhow::Result;
use tree_sitter::Node;

/// JavaScript 提取器
pub struct JavaScriptExtractor {
    language: tree_sitter::Language,
}

impl JavaScriptExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            language: tree_sitter_javascript::language(),
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

        // 参数列表
        if let Some(params) = find_child_by_kind(node, "formal_parameters") {
            parts.push(node_text(&params, source).to_string());
        }

        format!("{} {{ ... }}", parts.join(" "))
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
            if parent.kind() == "lexical_declaration" || parent.kind() == "variable_declaration" {
                let mut cursor = parent.walk();
                for child in parent.children(&mut cursor) {
                    let text = node_text(&child, source);
                    if text == "const" || text == "let" || text == "var" {
                        parts.push(text.to_string());
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
        let name_node = find_child_by_kind(node, "identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // export
        if self.has_export_modifier(node) {
            signature.push_str("export ");
        }

        signature.push_str("class ");
        signature.push_str(&name);

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

    /// 提取方法定义
    fn extract_method(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "property_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut parts = Vec::new();

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

        Some(Symbol {
            kind: SymbolKind::Function,
            name,
            signature: format!("{} {{ ... }}", parts.join(" ")),
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

impl LanguageExtractor for JavaScriptExtractor {
    fn extract_symbols(&self, source: &str) -> Result<Vec<Symbol>> {
        let tree = parse_source(source, self.language.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse JavaScript source"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();

        self.walk_tree(&root, source, &mut symbols);

        Ok(symbols)
    }

    fn language_name(&self) -> &str {
        "javascript"
    }
}

impl JavaScriptExtractor {
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
function greet(name) {
    return `Hello, ${name}!`;
}
"#;

        let extractor = JavaScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].name, "greet");
    }

    #[test]
    fn test_extract_arrow_function() {
        let source = r#"
const add = (a, b) => {
    return a + b;
};
"#;

        let extractor = JavaScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "add");
    }

    #[test]
    fn test_extract_class() {
        let source = r#"
export class User {
    constructor(name) {
        this.name = name;
    }
}
"#;

        let extractor = JavaScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert!(symbols.iter().any(|s| s.name == "User"));
    }

    #[test]
    fn test_extract_react_component() {
        let source = r#"
const Button = ({ onClick, children }) => {
    return <button onClick={onClick}>{children}</button>;
};
"#;

        let extractor = JavaScriptExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Button");
    }
}
