// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};
use srag_common::Result;
use tree_sitter::Parser;

pub fn chunk_with_tree_sitter(text: &str, language: Language) -> Result<Vec<Chunk>> {
    let Some(ts_language) = get_tree_sitter_language(language) else {
        return Err(srag_common::Error::Chunking(format!(
            "no tree-sitter grammar for {:?}",
            language
        )));
    };

    let mut parser = Parser::new();
    parser
        .set_language(&ts_language)
        .map_err(|e| srag_common::Error::Chunking(e.to_string()))?;

    let Some(tree) = parser.parse(text, None) else {
        return Err(srag_common::Error::Chunking("Failed to parse file".into()));
    };

    let mut chunks = Vec::new();
    let root = tree.root_node();
    let node_kinds = extractable_kinds(language);

    collect_nodes(root, text, language, &node_kinds, &mut chunks);

    // if we didn't find any extractable nodes, fall back to root-level children
    if chunks.is_empty() {
        collect_top_level_chunks(root, text, language, &mut chunks);
    }

    Ok(chunks)
}

fn collect_nodes(
    node: tree_sitter::Node,
    source: &str,
    language: Language,
    kinds: &[&str],
    chunks: &mut Vec<Chunk>,
) {
    if kinds.contains(&node.kind()) {
        let start = node.start_position();
        let end = node.end_position();
        let content = &source[node.start_byte()..node.end_byte()];

        if !content.trim().is_empty() && content.len() > 10 {
            let symbol = extract_symbol_name(node, source);
            chunks.push(Chunk {
                id: None,
                file_id: 0,
                content: content.to_string(),
                symbol,
                symbol_kind: Some(node.kind().to_string()),
                start_line: (start.row + 1) as u32,
                end_line: (end.row + 1) as u32,
                language,
                suspicious: false,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, source, language, kinds, chunks);
    }
}

fn collect_top_level_chunks(
    root: tree_sitter::Node,
    source: &str,
    language: Language,
    chunks: &mut Vec<Chunk>,
) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        let start = child.start_position();
        let end = child.end_position();
        let content = &source[child.start_byte()..child.end_byte()];

        if !content.trim().is_empty() && content.len() > 10 {
            chunks.push(Chunk {
                id: None,
                file_id: 0,
                content: content.to_string(),
                symbol: None,
                symbol_kind: Some(child.kind().to_string()),
                start_line: (start.row + 1) as u32,
                end_line: (end.row + 1) as u32,
                language,
                suspicious: false,
            });
        }
    }
}

fn extract_symbol_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier"
            | "name"
            | "type_identifier"
            | "property_identifier"
            | "field_identifier" => {
                return Some(source[child.start_byte()..child.end_byte()].to_string());
            }
            _ => {}
        }
    }
    None
}

fn get_tree_sitter_language(lang: Language) -> Option<tree_sitter::Language> {
    match lang {
        Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        Language::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
        Language::C => Some(tree_sitter_c::LANGUAGE.into()),
        Language::Cpp => Some(tree_sitter_cpp::LANGUAGE.into()),
        Language::Java => Some(tree_sitter_java::LANGUAGE.into()),
        Language::Ruby => Some(tree_sitter_ruby::LANGUAGE.into()),
        _ => None,
    }
}

fn extractable_kinds(lang: Language) -> Vec<&'static str> {
    match lang {
        Language::Rust => vec![
            "function_item",
            "impl_item",
            "struct_item",
            "enum_item",
            "trait_item",
            "mod_item",
            "macro_definition",
            "const_item",
            "static_item",
            "type_item",
        ],
        Language::Python => vec![
            "function_definition",
            "class_definition",
            "decorated_definition",
        ],
        Language::JavaScript | Language::TypeScript => vec![
            "function_declaration",
            "class_declaration",
            "method_definition",
            "arrow_function",
            "export_statement",
            "lexical_declaration",
        ],
        Language::Go => vec![
            "function_declaration",
            "method_declaration",
            "type_declaration",
        ],
        Language::C | Language::Cpp => vec![
            "function_definition",
            "struct_specifier",
            "class_specifier",
            "enum_specifier",
            "namespace_definition",
        ],
        Language::Java => vec![
            "method_declaration",
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "constructor_declaration",
        ],
        Language::Ruby => vec!["method", "class", "module", "singleton_method"],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_function() {
        let code = "fn hello() {\n    println!(\"hi\");\n}";
        let chunks = chunk_with_tree_sitter(code, Language::Rust).unwrap();
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.symbol == Some("hello".to_string())));
    }

    #[test]
    fn test_rust_struct() {
        let code = "struct Point {\n    x: i32,\n    y: i32,\n}";
        let chunks = chunk_with_tree_sitter(code, Language::Rust).unwrap();
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.symbol == Some("Point".to_string())));
    }

    #[test]
    fn test_rust_impl() {
        let code = "impl Point {\n    fn new() -> Self { Self { x: 0, y: 0 } }\n}";
        let chunks = chunk_with_tree_sitter(code, Language::Rust).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_python_function() {
        let code = "def greet(name):\n    print(f'Hello {name}')";
        let chunks = chunk_with_tree_sitter(code, Language::Python).unwrap();
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.symbol == Some("greet".to_string())));
    }

    #[test]
    fn test_python_class() {
        let code = "class MyClass:\n    def __init__(self):\n        pass";
        let chunks = chunk_with_tree_sitter(code, Language::Python).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_javascript_function() {
        let code = "function hello() {\n    console.log('hi');\n}";
        let chunks = chunk_with_tree_sitter(code, Language::JavaScript).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_go_function() {
        let code = "func main() {\n    fmt.Println(\"hi\")\n}";
        let chunks = chunk_with_tree_sitter(code, Language::Go).unwrap();
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_minimum_chunk_size() {
        let code = "fn a() {}";
        let chunks = chunk_with_tree_sitter(code, Language::Rust).unwrap();
        assert!(chunks.is_empty() || chunks[0].content.len() > 10);
    }

    #[test]
    fn test_unsupported_language() {
        let result = chunk_with_tree_sitter("test", Language::Markdown);
        assert!(result.is_err());
    }

    #[test]
    fn test_line_numbers() {
        let code = "\n\nfn test() {\n    // body\n}";
        let chunks = chunk_with_tree_sitter(code, Language::Rust).unwrap();
        if !chunks.is_empty() {
            assert!(chunks[0].start_line >= 3);
        }
    }

    #[test]
    fn test_symbol_kind_preserved() {
        let code = "fn test() { let x = 1; }";
        let chunks = chunk_with_tree_sitter(code, Language::Rust).unwrap();
        if !chunks.is_empty() {
            assert!(chunks[0].symbol_kind.is_some());
        }
    }
}
