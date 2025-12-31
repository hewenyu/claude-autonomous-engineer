//! Parser 辅助工具
//!
//! 提供通用的解析辅助函数

use tree_sitter::{Node, Parser, Tree};

/// 解析源代码为 Tree-sitter AST
pub fn parse_source(source: &str, language: tree_sitter::Language) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    parser.parse(source, None)
}

/// 提取节点的文本内容
pub fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    let start = node.start_byte();
    let end = node.end_byte();
    &source[start..end]
}

/// 查找指定类型的子节点
pub fn find_child_by_kind<'a>(node: &'a Node, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).find(|child| child.kind() == kind)
}

/// 提取所有指定类型的子节点
pub fn find_children_by_kind<'a>(node: &'a Node, kind: &str) -> Vec<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.kind() == kind)
        .collect()
}
