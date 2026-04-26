use anyhow::{anyhow, Result};
use serde::Serialize;
use tree_sitter::{Node, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Go,
    Rust,
}

#[derive(Debug, Serialize, Clone)]
pub struct UastFunction {
    pub name: String,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub signature: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UastImport {
    pub source: String,
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UastDocument {
    pub language: String,
    pub functions: Vec<UastFunction>,
    pub imports: Vec<UastImport>,
}

pub fn detect_language(path: &str) -> Option<Language> {
    if path.ends_with(".go") {
        Some(Language::Go)
    } else if path.ends_with(".rs") {
        Some(Language::Rust)
    } else {
        None
    }
}

pub fn parse_uast(language: Language, source: &str) -> Result<UastDocument> {
    let mut parser = Parser::new();
    match language {
        Language::Go => parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .map_err(|err| anyhow!("tree-sitter-go error: {err:?}"))?,
        Language::Rust => parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|err| anyhow!("tree-sitter-rust error: {err:?}"))?,
    }

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("failed to parse source"))?;

    let mut functions = Vec::new();
    let mut imports = Vec::new();

    let root = tree.root_node();
    collect_nodes(language, root, source.as_bytes(), &mut functions, &mut imports);

    Ok(UastDocument {
        language: match language {
            Language::Go => "go",
            Language::Rust => "rust",
        }
        .to_string(),
        functions,
        imports,
    })
}

fn collect_nodes(
    language: Language,
    node: Node<'_>,
    source: &[u8],
    functions: &mut Vec<UastFunction>,
    imports: &mut Vec<UastImport>,
) {
    match language {
        Language::Go => collect_go_nodes(node, source, functions, imports),
        Language::Rust => collect_rust_nodes(node, source, functions, imports),
    }
}

fn collect_go_nodes(
    node: Node<'_>,
    source: &[u8],
    functions: &mut Vec<UastFunction>,
    imports: &mut Vec<UastImport>,
) {
    if node.kind() == "function_declaration" || node.kind() == "method_declaration" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = node_text(name_node, source).unwrap_or_default();
            let signature = node_signature(node, source);
            functions.push(build_function(&name, node, signature));
        }
    }

    if node.kind() == "import_spec" {
        let source_node = node.child_by_field_name("path");
        if let Some(source_node) = source_node {
            let raw = node_text(source_node, source).unwrap_or_default();
            let cleaned = raw.trim_matches('"').to_string();
            let target = node
                .child_by_field_name("name")
                .and_then(|name_node| node_text(name_node, source));
            if !cleaned.is_empty() {
                imports.push(UastImport {
                    source: cleaned,
                    target,
                });
            }
        }
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_go_nodes(child, source, functions, imports);
        }
    }
}

fn collect_rust_nodes(
    node: Node<'_>,
    source: &[u8],
    functions: &mut Vec<UastFunction>,
    imports: &mut Vec<UastImport>,
) {
    if node.kind() == "function_item" {
        if let Some(name_node) = node.child_by_field_name("name") {
            let name = node_text(name_node, source).unwrap_or_default();
            let signature = node_signature(node, source);
            functions.push(build_function(&name, node, signature));
        }
    }

    if node.kind() == "use_declaration" {
        if let Some(text) = node_text(node, source) {
            let cleaned = text.trim().trim_start_matches("use ").trim_end_matches(';');
            if !cleaned.is_empty() {
                imports.push(UastImport {
                    source: cleaned.to_string(),
                    target: None,
                });
            }
        }
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_rust_nodes(child, source, functions, imports);
        }
    }
}

fn build_function(name: &str, node: Node<'_>, signature: Option<String>) -> UastFunction {
    let start_line = Some(node.start_position().row as i32 + 1);
    let end_line = Some(node.end_position().row as i32 + 1);
    UastFunction {
        name: name.to_string(),
        start_line,
        end_line,
        signature,
    }
}

fn node_signature(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node_text(node, source)?;
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(line.to_string())
    }
}

fn node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    let start = node.start_byte();
    let end = node.end_byte();
    let slice = source.get(start..end)?;
    std::str::from_utf8(slice).ok().map(|text| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rust_functions_and_imports() {
        let source = "use std::fmt;\n\nfn hello() {}\n";
        let doc = parse_uast(Language::Rust, source).expect("parse");
        assert_eq!(doc.language, "rust");
        assert!(doc.functions.iter().any(|f| f.name == "hello"));
        assert!(doc.imports.iter().any(|i| i.source.contains("std::fmt")));
    }

    #[test]
    fn parse_go_functions_and_imports() {
        let source = "package main\n\nimport \"fmt\"\n\nfunc main() {}\n";
        let doc = parse_uast(Language::Go, source).expect("parse");
        assert_eq!(doc.language, "go");
        assert!(doc.functions.iter().any(|f| f.name == "main"));
        assert!(doc.imports.iter().any(|i| i.source == "fmt"));
    }
}
