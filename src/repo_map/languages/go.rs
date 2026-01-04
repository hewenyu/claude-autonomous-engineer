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
        // 方法名是 field_identifier，函数名是 identifier
        let name_node = find_child_by_kind(node, "field_identifier")
            .or_else(|| find_child_by_kind(node, "identifier"))?;
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
        parts.push("func".to_string());

        // 收集所有 parameter_list（方法会有 receiver + params，函数只有 params）
        let mut param_lists = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter_list" {
                param_lists.push(child);
            }
        }

        // 如果有两个 parameter_list，第一个是 receiver（方法）
        if param_lists.len() == 2 {
            parts.push(node_text(&param_lists[0], source).to_string());
        }

        // 函数/方法名
        parts.push(name.to_string());

        // 参数列表（方法的第二个 parameter_list，函数的第一个）
        if !param_lists.is_empty() {
            let params = param_lists.last().unwrap();
            parts.push(node_text(params, source).to_string());
        }

        // 返回类型（在最后一个 parameter_list 之后的类型节点）
        let mut found_last_params = false;
        let last_params_id = param_lists.last().map(|n| n.id());

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if Some(child.id()) == last_params_id {
                found_last_params = true;
                continue;
            }

            if found_last_params {
                match child.kind() {
                    "type_identifier" | "pointer_type" | "slice_type" | "array_type"
                    | "struct_type" | "interface_type" | "qualified_type" => {
                        parts.push(node_text(&child, source).to_string());
                        break;
                    }
                    _ => {}
                }
            }
        }

        format!("{};", parts.join(" "))
    }

    /// 提取类型声明（结构体、接口、类型别名）
    fn extract_type_declaration(&self, node: &Node, source: &str) -> Option<Symbol> {
        // type_declaration 包含 type_spec
        let type_spec = find_child_by_kind(node, "type_spec")?;
        let name_node = find_child_by_kind(&type_spec, "type_identifier")?;
        let name = node_text(&name_node, source).to_string();

        // 优先检查是否有 struct_type 或 interface_type
        if find_child_by_kind(&type_spec, "struct_type").is_some() {
            return Some(Symbol {
                kind: SymbolKind::Struct,
                name: name.clone(),
                signature: format!("type {} struct {{ ... }}", name),
                line_start: node.start_position().row + 1,
                line_end: node.end_position().row + 1,
            });
        }

        if find_child_by_kind(&type_spec, "interface_type").is_some() {
            return Some(Symbol {
                kind: SymbolKind::Trait,
                name: name.clone(),
                signature: format!("type {} interface {{ ... }}", name),
                line_start: node.start_position().row + 1,
                line_end: node.end_position().row + 1,
            });
        }

        // 其他情况是类型别名
        Some(Symbol {
            kind: SymbolKind::Type,
            name: name.clone(),
            signature: format!("type {} = ...", name),
            line_start: node.start_position().row + 1,
            line_end: node.end_position().row + 1,
        })
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
                if let Some(symbol) = self.extract_type_declaration(node, source) {
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
