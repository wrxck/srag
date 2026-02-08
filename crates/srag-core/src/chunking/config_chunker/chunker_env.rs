// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};

pub fn chunk_env_file(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let key = trimmed.split('=').next().unwrap_or(trimmed).trim();

        chunks.push(Chunk {
            id: None,
            file_id: 0,
            content: line.to_string(),
            symbol: Some(key.to_string()),
            symbol_kind: Some("env_var".to_string()),
            start_line: (i + 1) as u32,
            end_line: (i + 1) as u32,
            language: Language::Env,
            suspicious: false,
        });
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_basic() {
        let text = "API_KEY=secret\nDB_URL=postgres://localhost";
        let chunks = chunk_env_file(text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol, Some("API_KEY".to_string()));
        assert_eq!(chunks[1].symbol, Some("DB_URL".to_string()));
    }

    #[test]
    fn test_env_skips_comments() {
        let text = "# comment\nKEY=value\n# another comment";
        let chunks = chunk_env_file(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("KEY".to_string()));
    }

    #[test]
    fn test_env_skips_empty_lines() {
        let text = "\n\nKEY=value\n\n";
        let chunks = chunk_env_file(text);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_env_empty() {
        let chunks = chunk_env_file("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_env_with_equals_in_value() {
        let text = "URL=http://host?foo=bar";
        let chunks = chunk_env_file(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("URL".to_string()));
    }
}
