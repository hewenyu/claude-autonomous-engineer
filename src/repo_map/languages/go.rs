//! Go 语言提取器
//!
//! 使用 tree-sitter-go 解析 Go 代码，提取函数、结构体、接口等骨架信息

use crate::repo_map::extractor::LanguageExtractor;
use crate::repo_map::parser::{find_child_by_kind, node_text, parse_source};
use crate::repo_map::{Symbol, SymbolKind};
use anyhow::Result;
use tree_sitter::Node;

/// Go 提取器
pub struct GoExtractor {
    language: tree_sitter::Language,
}

impl GoExtractor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            language: tree_sitter_go::language(),
        })
    }

    /// 提取函数定义
    fn extract_function(&self, node: &Node, source: &str) -> Option<Symbol> {
        let name_node = find_child_by_kind(node, "identifier")?;
        let name = node_text(&name_node, source).to_string();

        let signature = self.build_function_signature(node, source);

        Some(Symbol {
            kind: SymbolKind::Function,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 构建函数签名
    fn build_function_signature(&self, node: &Node, source: &str) -> String {
        let mut parts = Vec::new();

        // func 关键字
        parts.push("func".to_string());

        // 接收者（如果是方法）
        if let Some(receiver) = find_child_by_kind(node, "parameter_list") {
            // 第一个 parameter_list 可能是接收者
            let text = node_text(&receiver, source);
            if text.starts_with('(') && !text.contains(')') || text.len() < 50 {
                parts.push(text.to_string());
            }
        }

        // 函数名
        if let Some(name_node) = find_child_by_kind(node, "identifier") {
            parts.push(node_text(&name_node, source).to_string());
        }

        // 参数列表
        let mut found_params = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter_list" && !found_params {
                // 跳过接收者，找实际参数
                if found_params {
                    parts.push(node_text(&child, source).to_string());
                    break;
                }
                found_params = true;
                parts.push(node_text(&child, source).to_string());
            }
        }

        // 返回类型
        if let Some(result) = self.find_result_type(node, source) {
            parts.push(result);
        }

        format!("{};", parts.join(" "))
    }

    /// 查找返回类型
    fn find_result_type(&self, node: &Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "parameter_list" => {
                    // 最后一个 parameter_list 可能是返回类型
                    let text = node_text(&child, source);
                    if text.starts_with('(') {
                        return Some(text.to_string());
                    }
                }
                "type_identifier" | "pointer_type" | "slice_type" | "array_type" => {
                    return Some(node_text(&child, source).to_string());
                }
                _ => continue,
            }
        }
        None
    }

    /// 提取结构体定义
    fn extract_struct(&self, node: &Node, source: &str) -> Option<Symbol> {
        // 查找 type_spec 节点
        let type_spec = if node.kind() == "type_spec" {
            node
        } else {
            &find_child_by_kind(node, "type_spec")?
        };

        let name_node = find_child_by_kind(type_spec, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();
        signature.push_str("type ");
        signature.push_str(&name);
        signature.push_str(" struct { ... }");

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
        let type_spec = if node.kind() == "type_spec" {
            node
        } else {
            &find_child_by_kind(node, "type_spec")?
        };

        let name_node = find_child_by_kind(type_spec, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        let mut signature = String::new();
        signature.push_str("type ");
        signature.push_str(&name);
        signature.push_str(" interface { ... }");

        Some(Symbol {
            kind: SymbolKind::Trait, // 使用 Trait 表示接口
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

        let signature = format!("type {} = ...", name);

        Some(Symbol {
            kind: SymbolKind::Type,
            name,
            signature,
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
    }

    /// 检查节点是否包含结构体定义
    fn has_struct_type(&self, node: &Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "struct_type" {
                return true;
            }
        }
        false
    }

    /// 检查节点是否包含接口定义
    fn has_interface_type(&self, node: &Node) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "interface_type" {
                return true;
            }
        }
        false
    }
}

impl LanguageExtractor for GoExtractor {
    fn extract_symbols(&self, source: &str) -> Result<Vec<Symbol>> {
        let tree = parse_source(source, self.language.clone())
            .ok_or_else(|| anyhow::anyhow!("Failed to parse Go source"))?;

        let root = tree.root_node();
        let mut symbols = Vec::new();

        self.walk_tree(&root, source, &mut symbols);

        Ok(symbols)
    }

    fn language_name(&self) -> &str {
        "go"
    }
}

impl GoExtractor {
    /// 递归遍历 AST
    fn walk_tree(&self, node: &Node, source: &str, symbols: &mut Vec<Symbol>) {
        match node.kind() {
            "function_declaration" | "method_declaration" => {
                if let Some(symbol) = self.extract_function(node, source) {
                    symbols.push(symbol);
                }
            }
            "type_declaration" => {
                // Go 的 type 声明可以是 struct, interface, 或 type alias
                if self.has_struct_type(node) {
                    if let Some(symbol) = self.extract_struct(node, source) {
                        symbols.push(symbol);
                    }
                } else if self.has_interface_type(node) {
                    if let Some(symbol) = self.extract_interface(node, source) {
                        symbols.push(symbol);
                    }
                } else {
                    if let Some(symbol) = self.extract_type_alias(node, source) {
                        symbols.push(symbol);
                    }
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
func Add(a int, b int) int {
    return a + b
}
"#;

        let extractor = GoExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Function);
        assert_eq!(symbols[0].name, "Add");
        assert!(symbols[0].signature.contains("func Add"));
    }

    #[test]
    fn test_extract_method() {
        let source = r#"
func (u *User) GetName() string {
    return u.name
}
"#;

        let extractor = GoExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "GetName");
    }

    #[test]
    fn test_extract_struct() {
        let source = r#"
type User struct {
    name string
    age  int
}
"#;

        let extractor = GoExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(symbols[0].name, "User");
    }

    #[test]
    fn test_extract_interface() {
        let source = r#"
type Reader interface {
    Read(p []byte) (n int, err error)
}
"#;

        let extractor = GoExtractor::new().unwrap();
        let symbols = extractor.extract_symbols(source).unwrap();

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, SymbolKind::Trait);
        assert_eq!(symbols[0].name, "Reader");
    }
}
