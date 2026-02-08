// SPDX-Licence-Identifier: GPL-3.0

use srag_common::types::{Definition, Language};
use tree_sitter::Node;

pub fn try_extract_definition(
    node: Node,
    source: &str,
    language: Language,
    file_id: i64,
    chunk_id: i64,
) -> Option<Definition> {
    let kind = node.kind();
    let (is_def, def_kind) = is_definition_node(kind, language);
    if !is_def {
        return None;
    }

    let name = extract_definition_name(node, source, language)?;
    let scope = extract_parent_scope(node, source, language);

    Some(Definition {
        id: None,
        chunk_id,
        file_id,
        name,
        kind: def_kind.to_string(),
        scope,
        language,
        start_line: (node.start_position().row + 1) as u32,
        end_line: (node.end_position().row + 1) as u32,
        signature: extract_signature(node, source),
    })
}

fn is_definition_node(kind: &str, language: Language) -> (bool, &'static str) {
    match language {
        Language::Rust => match kind {
            "function_item" => (true, "function"),
            "impl_item" => (true, "impl"),
            "struct_item" => (true, "struct"),
            "enum_item" => (true, "enum"),
            "trait_item" => (true, "trait"),
            _ => (false, ""),
        },
        Language::Python => match kind {
            "function_definition" => (true, "function"),
            "class_definition" => (true, "class"),
            _ => (false, ""),
        },
        Language::JavaScript | Language::TypeScript => match kind {
            "function_declaration" => (true, "function"),
            "method_definition" => (true, "method"),
            "arrow_function" => (true, "function"),
            "class_declaration" => (true, "class"),
            _ => (false, ""),
        },
        Language::Go => match kind {
            "function_declaration" => (true, "function"),
            "method_declaration" => (true, "method"),
            _ => (false, ""),
        },
        Language::C | Language::Cpp => match kind {
            "function_definition" => (true, "function"),
            "class_specifier" => (true, "class"),
            "struct_specifier" => (true, "struct"),
            _ => (false, ""),
        },
        Language::Java => match kind {
            "method_declaration" => (true, "method"),
            "constructor_declaration" => (true, "constructor"),
            "class_declaration" => (true, "class"),
            "interface_declaration" => (true, "interface"),
            _ => (false, ""),
        },
        Language::Ruby => match kind {
            "method" => (true, "method"),
            "singleton_method" => (true, "method"),
            "class" => (true, "class"),
            "module" => (true, "module"),
            _ => (false, ""),
        },
        _ => (false, ""),
    }
}

fn extract_definition_name(node: Node, source: &str, language: Language) -> Option<String> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let child_kind = child.kind();

        let is_name = match language {
            Language::Rust => matches!(child_kind, "identifier" | "type_identifier"),
            Language::Python => child_kind == "identifier",
            Language::JavaScript | Language::TypeScript => {
                matches!(child_kind, "identifier" | "property_identifier")
            }
            Language::Go => child_kind == "identifier",
            Language::C | Language::Cpp => {
                matches!(child_kind, "identifier" | "field_identifier")
                    || child_kind == "function_declarator"
            }
            Language::Java => child_kind == "identifier",
            Language::Ruby => child_kind == "identifier",
            _ => false,
        };

        if is_name {
            if child_kind == "function_declarator" {
                return extract_definition_name(child, source, language);
            }
            return Some(source[child.start_byte()..child.end_byte()].to_string());
        }
    }

    None
}

fn extract_parent_scope(node: Node, source: &str, language: Language) -> Option<String> {
    let mut current = node.parent();

    while let Some(parent) = current {
        if is_scope_node(parent.kind(), language) {
            return extract_scope_name(parent, source);
        }
        current = parent.parent();
    }

    None
}

fn is_scope_node(kind: &str, language: Language) -> bool {
    match language {
        Language::Rust => matches!(kind, "impl_item" | "mod_item" | "trait_item"),
        Language::Python => kind == "class_definition",
        Language::JavaScript | Language::TypeScript => kind == "class_declaration",
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

fn extract_scope_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "identifier" | "type_identifier" | "name") {
            return Some(source[child.start_byte()..child.end_byte()].to_string());
        }
    }
    None
}

fn extract_signature(node: Node, source: &str) -> Option<String> {
    let start = node.start_byte();
    let text = &source[start..];

    if let Some(brace_pos) = text.find('{') {
        let sig = text[..brace_pos].trim();
        if sig.len() < 200 {
            return Some(sig.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::call_graph::get_tree_sitter_language;
    use tree_sitter::Parser;

    fn parse_and_extract(code: &str, language: Language) -> Vec<Definition> {
        let ts_lang = get_tree_sitter_language(language).unwrap();
        let mut parser = Parser::new();
        parser.set_language(&ts_lang).unwrap();
        let tree = parser.parse(code, None).unwrap();

        let mut defs = Vec::new();
        collect_definitions(tree.root_node(), code, language, 1, 1, &mut defs);
        defs
    }

    fn collect_definitions(
        node: Node,
        source: &str,
        language: Language,
        file_id: i64,
        chunk_id: i64,
        defs: &mut Vec<Definition>,
    ) {
        if let Some(def) = try_extract_definition(node, source, language, file_id, chunk_id) {
            defs.push(def);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_definitions(child, source, language, file_id, chunk_id, defs);
        }
    }

    #[test]
    fn test_rust_function() {
        let defs = parse_and_extract("fn hello() { }", Language::Rust);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "hello");
        assert_eq!(defs[0].kind, "function");
    }

    #[test]
    fn test_python_class() {
        let defs = parse_and_extract("class Foo:\n    pass", Language::Python);
        assert!(defs.iter().any(|d| d.name == "Foo" && d.kind == "class"));
    }

    #[test]
    fn test_go_function() {
        let defs = parse_and_extract("func main() { }", Language::Go);
        assert!(defs.iter().any(|d| d.name == "main"));
    }
}
