// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};

pub fn chunk_toml_file(text: &str, language: Language) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_section = String::new();
    let mut current_lines = Vec::new();
    let mut start_line: u32 = 1;

    for (i, line) in text.lines().enumerate() {
        let line_num = (i + 1) as u32;
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.contains(']') {
            if !current_lines.is_empty() {
                let content = current_lines.join("\n");
                if !content.trim().is_empty() {
                    chunks.push(Chunk {
                        id: None,
                        file_id: 0,
                        content,
                        symbol: Some(current_section.clone()),
                        symbol_kind: Some("section".to_string()),
                        start_line,
                        end_line: line_num - 1,
                        language,
                        suspicious: false,
                    });
                }
            }
            current_section = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .to_string();
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
                symbol: if current_section.is_empty() {
                    None
                } else {
                    Some(current_section)
                },
                symbol_kind: Some("section".to_string()),
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
    fn test_toml_single_section() {
        let text = "[package]\nname = \"test\"\nversion = \"1.0\"";
        let chunks = chunk_toml_file(text, Language::Toml);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("package".to_string()));
    }

    #[test]
    fn test_toml_multiple_sections() {
        let text = "[package]\nname = \"test\"\n\n[dependencies]\nserde = \"1.0\"";
        let chunks = chunk_toml_file(text, Language::Toml);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol, Some("package".to_string()));
        assert_eq!(chunks[1].symbol, Some("dependencies".to_string()));
    }

    #[test]
    fn test_toml_nested_section() {
        let text = "[package]\nname = \"test\"\n\n[dependencies.serde]\nversion = \"1.0\"";
        let chunks = chunk_toml_file(text, Language::Toml);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].symbol, Some("dependencies.serde".to_string()));
    }

    #[test]
    fn test_toml_content_before_first_section() {
        let text = "# top comment\nkey = \"value\"\n\n[section]\nname = \"test\"";
        let chunks = chunk_toml_file(text, Language::Toml);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].symbol.is_none() || chunks[0].symbol == Some("".to_string()));
    }

    #[test]
    fn test_toml_empty() {
        let chunks = chunk_toml_file("", Language::Toml);
        assert!(chunks.is_empty());
    }
}
