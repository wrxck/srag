// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};

pub fn chunk_yaml_file(text: &str, language: Language) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_key = String::new();
    let mut current_lines = Vec::new();
    let mut start_line: u32 = 1;

    for (i, line) in text.lines().enumerate() {
        let line_num = (i + 1) as u32;

        let is_top_level = !line.is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && !line.starts_with('#')
            && !line.starts_with("---")
            && !line.starts_with("...");

        if is_top_level {
            if !current_lines.is_empty() {
                let content = current_lines.join("\n");

                if !content.trim().is_empty() {
                    chunks.push(Chunk {
                        id: None,
                        file_id: 0,
                        content,
                        symbol: if current_key.is_empty() {
                            None
                        } else {
                            Some(current_key.clone())
                        },
                        symbol_kind: Some("key".to_string()),
                        start_line,
                        end_line: line_num - 1,
                        language,
                        suspicious: false,
                    });
                }
            }
            current_key = line.split(':').next().unwrap_or("").trim().to_string();
            current_lines = vec![line.to_string()];
            start_line = line_num;
        } else {
            current_lines.push(line.to_string());
        }
    }

    if !current_lines.is_empty() {
        let content = current_lines.join("\n");
        if !content.trim().is_empty() {
            let end_line = text.lines().count() as u32;
            chunks.push(Chunk {
                id: None,
                file_id: 0,
                content,
                symbol: if current_key.is_empty() {
                    None
                } else {
                    Some(current_key)
                },
                symbol_kind: Some("key".to_string()),
                start_line,
                end_line,
                language,
                suspicious: false,
            });
        }
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_single_key() {
        let text = "name: test";
        let chunks = chunk_yaml_file(text, Language::Yaml);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("name".to_string()));
    }

    #[test]
    fn test_yaml_multiple_keys() {
        let text = "name: test\nversion: 1.0\n\ndependencies:\n  - serde";
        let chunks = chunk_yaml_file(text, Language::Yaml);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn test_yaml_with_document_markers() {
        let text = "---\nname: test\n...";
        let chunks = chunk_yaml_file(text, Language::Yaml);
        assert!(!chunks.is_empty());
        assert!(chunks.iter().any(|c| c.symbol == Some("name".to_string())));
    }

    #[test]
    fn test_yaml_nested_content() {
        let text = "parent:\n  child1: value1\n  child2: value2";
        let chunks = chunk_yaml_file(text, Language::Yaml);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("child1"));
    }

    #[test]
    fn test_yaml_empty() {
        let chunks = chunk_yaml_file("", Language::Yaml);
        assert!(chunks.is_empty());
    }
}
