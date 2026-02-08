// SPDX-License-Identifier: GPL-3.0

mod calls;
mod definitions;

use srag_common::types::{Definition, FunctionCall, Language};
use tree_sitter::{Node, Parser};

use super::tree_sitter_chunker::get_tree_sitter_language;

pub use calls::try_extract_call;
pub use definitions::try_extract_definition;

pub struct CallGraphData {
    pub definitions: Vec<Definition>,
    pub calls: Vec<FunctionCall>,
}

/// Extract call graph data from source code.
///
/// NOTE: `chunk_id` is assigned to all definitions and calls found in the text.
/// For correct per-chunk attribution, callers should invoke this once per chunk
/// (passing only that chunk's text), not once for the entire file.
pub fn extract_call_graph(
    text: &str,
    language: Language,
    file_id: i64,
    chunk_id: i64,
) -> Option<CallGraphData> {
    let ts_language = get_tree_sitter_language(language)?;

    let mut parser = Parser::new();
    parser.set_language(&ts_language).ok()?;

    let tree = parser.parse(text, None)?;
    let root = tree.root_node();

    let mut definitions = Vec::new();
    let mut calls = Vec::new();

    extract_from_node(
        root,
        text,
        language,
        file_id,
        chunk_id,
        None,
        None,
        &mut definitions,
        &mut calls,
        0,
    );

    Some(CallGraphData { definitions, calls })
}

#[allow(clippy::too_many_arguments)]
fn extract_from_node(
    node: Node,
    source: &str,
    language: Language,
    file_id: i64,
    chunk_id: i64,
    current_func: Option<&str>,
    current_scope: Option<&str>,
    definitions: &mut Vec<Definition>,
    calls: &mut Vec<FunctionCall>,
    depth: usize,
) {
    if depth > 50 {
        return; // prevent stack overflow on deeply nested code
    }

    if let Some(def) = try_extract_definition(node, source, language, file_id, chunk_id) {
        let func_name = def.name.clone();
        let scope = def.scope.clone();
        definitions.push(def);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            extract_from_node(
                child,
                source,
                language,
                file_id,
                chunk_id,
                Some(&func_name),
                scope.as_deref(),
                definitions,
                calls,
                depth + 1,
            );
        }
        return;
    }

    if let Some(callee) = try_extract_call(node, source, language) {
        calls.push(FunctionCall {
            id: None,
            chunk_id,
            file_id,
            caller_name: current_func.map(|s| s.to_string()),
            caller_scope: current_scope.map(|s| s.to_string()),
            callee_name: callee,
            line_number: (node.start_position().row + 1) as u32,
            language,
            callee_definition_id: None,
        });
    }

    if is_scope_node(node.kind(), language) {
        let scope_name = extract_scope_name(node, source, language);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            extract_from_node(
                child,
                source,
                language,
                file_id,
                chunk_id,
                current_func,
                scope_name.as_deref(),
                definitions,
                calls,
                depth + 1,
            );
        }
    } else {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            extract_from_node(
                child,
                source,
                language,
                file_id,
                chunk_id,
                current_func,
                current_scope,
                definitions,
                calls,
                depth + 1,
            );
        }
    }
}

pub(super) fn is_scope_node(kind: &str, language: Language) -> bool {
    match language {
        Language::Rust => matches!(kind, "impl_item" | "mod_item" | "trait_item"),
        Language::Python => kind == "class_definition",
        Language::JavaScript | Language::TypeScript => kind == "class_declaration",
        Language::Go => matches!(kind, "method_declaration"),
        Language::C | Language::Cpp => {
            matches!(
                kind,
                "class_specifier" | "struct_specifier" | "namespace_definition"
            )
        }
        Language::Java => matches!(kind, "class_declaration" | "interface_declaration"),
        Language::Ruby => matches!(kind, "class" | "module"),
        _ => false,
    }
}

pub(super) fn extract_scope_name(node: Node, source: &str, language: Language) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "identifier" | "type_identifier" | "name") {
            return Some(source[child.start_byte()..child.end_byte()].to_string());
        }
    }

    if language == Language::Rust && node.kind() == "impl_item" {
        let mut cursor2 = node.walk();
        for child in node.children(&mut cursor2) {
            if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                return extract_first_identifier(child, source);
            }
        }
    }

    None
}

fn extract_first_identifier(node: Node, source: &str) -> Option<String> {
    if matches!(node.kind(), "identifier" | "type_identifier") {
        return Some(source[node.start_byte()..node.end_byte()].to_string());
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(id) = extract_first_identifier(child, source) {
            return Some(id);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_function_definition() {
        let code = "fn hello() {\n    println!(\"hi\");\n}\n\nfn caller() {\n    hello();\n}";
        let data = extract_call_graph(code, Language::Rust, 1, 1).unwrap();
        assert_eq!(data.definitions.len(), 2);
        assert!(data.definitions.iter().any(|d| d.name == "hello"));
        assert!(data.calls.iter().any(|c| c.callee_name == "hello"));
    }

    #[test]
    fn test_caller_tracking() {
        let code = "fn a() {\n    b();\n}\n\nfn b() {\n    c();\n}\n\nfn c() {}";
        let data = extract_call_graph(code, Language::Rust, 1, 1).unwrap();
        let call_to_b = data.calls.iter().find(|c| c.callee_name == "b").unwrap();
        assert_eq!(call_to_b.caller_name, Some("a".to_string()));
    }
}
