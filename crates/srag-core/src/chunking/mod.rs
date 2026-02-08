// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};
use srag_common::Result;

pub mod call_graph;
mod config_chunker;
pub mod injection_scanner;
mod line_chunker;
mod tree_sitter_chunker;

pub fn chunk_file(content: &[u8], language: Language) -> Result<Vec<Chunk>> {
    let text = match std::str::from_utf8(content) {
        Ok(t) => t,
        Err(_) => return Ok(Vec::new()),
    };

    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    // route config file types to dedicated chunkers
    match language {
        Language::Env => return Ok(config_chunker::chunk_env_file(text)),
        Language::Toml => return Ok(config_chunker::chunk_toml_file(text, language)),
        Language::Yaml => return Ok(config_chunker::chunk_yaml_file(text, language)),
        Language::Json => return Ok(config_chunker::chunk_json_file(text, language)),
        _ => {}
    }

    if language.has_tree_sitter_support() {
        match tree_sitter_chunker::chunk_with_tree_sitter(text, language) {
            Ok(chunks) if !chunks.is_empty() => return Ok(chunks),
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("tree-sitter chunking failed, falling back to lines: {}", e);
            }
        }
    }

    line_chunker::chunk_by_lines(text, language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_file_empty() {
        let result = chunk_file(b"", Language::Rust).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_chunk_file_whitespace_only() {
        let result = chunk_file(b"   \n\n\t  ", Language::Rust).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_chunk_file_invalid_utf8() {
        let invalid = vec![0xff, 0xfe, 0x00, 0x01];
        let result = chunk_file(&invalid, Language::Rust).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_chunk_file_routes_env() {
        let content = b"KEY=value";
        let result = chunk_file(content, Language::Env).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, Language::Env);
    }

    #[test]
    fn test_chunk_file_routes_toml() {
        let content = b"[section]\nkey = \"value\"";
        let result = chunk_file(content, Language::Toml).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].language, Language::Toml);
    }

    #[test]
    fn test_chunk_file_routes_yaml() {
        let content = b"key: value";
        let result = chunk_file(content, Language::Yaml).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_chunk_file_routes_json() {
        let content = b"{\"key\": \"value\"}";
        let result = chunk_file(content, Language::Json).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_chunk_file_rust_tree_sitter() {
        let content = b"fn main() {\n    println!(\"hello world from srag\");\n    let x = 42;\n}";
        let result = chunk_file(content, Language::Rust).unwrap();
        assert!(!result.is_empty());
        assert!(result.iter().any(|c| c.symbol == Some("main".to_string())));
    }

    #[test]
    fn test_chunk_file_python_tree_sitter() {
        let content = b"def hello():\n    print('hi')";
        let result = chunk_file(content, Language::Python).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_chunk_file_fallback_to_lines() {
        let content = b"just some text\nwithout structure";
        let result = chunk_file(content, Language::Markdown).unwrap();
        assert!(!result.is_empty());
    }
}
