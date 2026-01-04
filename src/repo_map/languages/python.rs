//! Python 语言提取器
//!
//! 使用 tree-sitter-python 解析 Python 代码，提取函数、类、方法等骨架信息

use crate::repo_map::extractor::LanguageExtractor;
use crate::repo_map::parser::{find_child_by_kind, node_text, parse_source};
use crate::repo_map::{Symbol, SymbolKind};
use anyhow::Result;
use tree_sitter::Node;

/// Python 提取器
pub struct PythonExtractor {
    language: tree_sitter::Language,
}

impl PythonExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            language: tree_sitter_python::language(),
        })
    }

    /// 提取函数定义
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

        // 检查装饰器
        let decorators = self.extract_decorators(node, source);
        if !decorators.is_empty() {
            for decorator in &decorators {
                parts.push(format!("@{}", decorator));
            }
        }

        // 检查是否是异步函数
        let is_async = self.has_async_keyword(node);
        if is_async {
            parts.push("async".to_string());
        }

        // 函数定义
        let mut def_line = format!("def {}", name);

        // 参数列表
        if let Some(params_node) = find_child_by_kind(node, "parameters") {
            def_line.push_str(node_text(&params_node, source));
        }

        // 返回类型注解
        if let Some(return_type) = find_child_by_kind(node, "type") {
            def_line.push_str(" -> ");
            def_line.push_str(node_text(&return_type, source).trim());
        }

        def_line.push(':');
        parts.push(def_line);

        parts.join("\n")
    }

    /// 提取类定义
    fn extract_class(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();

        // 装饰器
        let decorators = self.extract_decorators(node, source);
        if !decorators.is_empty() {
            for decorator in &decorators {
                signature.push_str(&format!("@{}\n", decorator));
            }
        }

        // class 关键字
        signature.push_str("class ");
        signature.push_str(&name);

        // 基类
        if let Some(args_node) = find_child_by_kind(node, "argument_list") {
            signature.push_str(node_text(&args_node, source));
        }

        signature.push_str(":");

        Some(Symbol {
            kind: SymbolKind::Struct, // 使用 Struct 表示类
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 提取装饰器
    fn extract_decorators(&self, node: &Node, source: &str) -> Vec<String> {
        let mut decorators = Vec::new();

        // 检查前面的兄弟节点是否有装饰器
        if let Some(parent) = node.parent() {
            let mut cursor = parent.walk();
            for child in parent.children(&mut cursor) {
                if child.id() == node.id() {
                    break;
                }
                if child.kind() == "decorator" {
                    // 提取装饰器名称（去掉 @ 符号）
                    let text = node_text(&child, source);
                    decorators.push(text.trim_start_matches('@').trim().to_string());
                }
            }
        }

        decorators
    }

    /// 检查是否有 async 关键字
    fn has_async_keyword(&self, node: &Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "async" {
                return true;
            }
        }
        false
    }
}

impl LanguageExtractor for PythonExtractor {
    fn extract_symbols(&self, source: &str) -> Result<Vec<Symbol>> {
        let tree = parse_source(source, self.language.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Python source"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();

        self.walk_tree(&root, source, &mut symbols);

        Ok(symbols)
    }

    fn language_name(&self) -> &str {
        "python"
    }
}

impl PythonExtractor {
    /// 递归遍历 AST
    fn walk_tree(&self, node: &Node, source: &str, symbols: &mut Vec<Symbol>) {
        match node.kind() {
            "function_definition" => {
                if let Some(symbol) = self.extract_function(node, source) {
                    symbols.push(symbol);
                }
            }
            "class_definition" => {
                if let Some(symbol) = self.extract_class(node, source) {
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
def hello(name: str) -> str:
    return f"Hello, {name}!"
"#;

        let extractor = PythonExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].name, "hello");
        assert!(symbols[0].signature.contains("def hello"));
    }

    #[test]
    fn test_extract_async_function() {
        let source = r#"
async def fetch_data(url: str) -> dict:
    return {}
"#;

        let extractor = PythonExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("async"));
    }

    #[test]
    fn test_extract_class() {
        let source = r#"
class User:
    def __init__(self, name: str):
        self.name = name
"#;

        let extractor = PythonExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 2); // class + __init__ method
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].name, "User");
    }

    #[test]
    fn test_extract_decorated_function() {
        let source = r#"
@staticmethod
def create_user(name: str) -> User:
    return User(name)
"#;

        let extractor = PythonExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].signature.contains("@staticmethod"));
    }
}
