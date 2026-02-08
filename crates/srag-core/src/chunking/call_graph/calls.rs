// SPDX-Licence-Identifier: GPL-3.0

use srag_common::types::Language;
use tree_sitter::Node;

pub fn try_extract_call(node: Node, source: &str, language: Language) -> Option<String> {
    let kind = node.kind();

    let is_call = match language {
        Language::Rust => kind == "call_expression",
        Language::Python => kind == "call",
        Language::JavaScript | Language::TypeScript => kind == "call_expression",
        Language::Go => kind == "call_expression",
        Language::C | Language::Cpp => kind == "call_expression",
        Language::Java => kind == "method_invocation",
        Language::Ruby => matches!(kind, "call" | "method_call"),
        _ => false,
    };

    if !is_call {
        return None;
    }

    extract_callee_name(node, source, language)
}

fn extract_callee_name(node: Node, source: &str, language: Language) -> Option<String> {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let child_kind = child.kind();

        match language {
            Language::Rust | Language::Go | Language::C | Language::Cpp => {
                if child_kind == "identifier" {
                    return Some(source[child.start_byte()..child.end_byte()].to_string());
                }
                if child_kind == "field_expression" || child_kind == "selector_expression" {
                    return extract_method_name(child, source);
                }
                if child_kind == "scoped_identifier" {
                    return extract_last_identifier(child, source);
                }
            }
            Language::Python => {
                if child_kind == "identifier" {
                    return Some(source[child.start_byte()..child.end_byte()].to_string());
                }
                if child_kind == "attribute" {
                    return extract_last_identifier(child, source);
                }
            }
            Language::JavaScript | Language::TypeScript => {
                if child_kind == "identifier" {
                    return Some(source[child.start_byte()..child.end_byte()].to_string());
                }
                if child_kind == "member_expression" {
                    return extract_method_name(child, source);
                }
            }
            Language::Java => {
                if child_kind == "identifier" {
                    return Some(source[child.start_byte()..child.end_byte()].to_string());
                }
            }
            Language::Ruby => {
                if child_kind == "identifier" {
                    return Some(source[child.start_byte()..child.end_byte()].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_method_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let mut last_identifier = None;

    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier" | "property_identifier" | "field_identifier"
        ) {
            last_identifier = Some(source[child.start_byte()..child.end_byte()].to_string());
        }
    }

    last_identifier
}

fn extract_last_identifier(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let mut last = None;

    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            last = Some(source[child.start_byte()..child.end_byte()].to_string());
        } else if child.child_count() > 0 {
            if let Some(nested) = extract_last_identifier(child, source) {
                last = Some(nested);
            }
        }
    }

    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::call_graph::get_tree_sitter_language;
    use tree_sitter::Parser;

    fn parse_and_extract_calls(code: &str, language: Language) -> Vec<String> {
        let ts_lang = get_tree_sitter_language(language).unwrap();
        let mut parser = Parser::new();
        parser.set_language(&ts_lang).unwrap();
        let tree = parser.parse(code, None).unwrap();

        let mut calls = Vec::new();
        collect_calls(tree.root_node(), code, language, &mut calls);
        calls
    }

    fn collect_calls(node: Node, source: &str, language: Language, calls: &mut Vec<String>) {
        if let Some(callee) = try_extract_call(node, source, language) {
            calls.push(callee);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect_calls(child, source, language, calls);
        }
    }

    #[test]
    fn test_rust_simple_call() {
        let calls = parse_and_extract_calls("fn main() { foo(); }", Language::Rust);
        assert!(calls.contains(&"foo".to_string()));
    }

    #[test]
    fn test_rust_method_call() {
        let calls = parse_and_extract_calls("fn main() { self.bar(); }", Language::Rust);
        assert!(calls.contains(&"bar".to_string()));
    }

    #[test]
    fn test_python_call() {
        let calls = parse_and_extract_calls("def main():\n    foo()", Language::Python);
        assert!(calls.contains(&"foo".to_string()));
    }

    #[test]
    fn test_javascript_call() {
        let calls = parse_and_extract_calls("function main() { helper(); }", Language::JavaScript);
        assert!(calls.contains(&"helper".to_string()));
    }

    #[test]
    fn test_go_call() {
        let calls = parse_and_extract_calls("func main() { helper() }", Language::Go);
        assert!(calls.contains(&"helper".to_string()));
    }
}
