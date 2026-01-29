// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};
use srag_common::Result;

const MAX_CHUNK_LINES: usize = 60; //is this even optimal?
const OVERLAP_LINES: usize = 5;

pub fn chunk_by_lines(text: &str, language: Language) -> Result<Vec<Chunk>> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < lines.len() {
        let end = (start + MAX_CHUNK_LINES).min(lines.len());
        let chunk_text = lines[start..end].join("\n");

        if !chunk_text.trim().is_empty() {
            chunks.push(Chunk {
                id: None,
                file_id: 0,
                content: chunk_text,
                symbol: None,
                symbol_kind: None,
                start_line: (start + 1) as u32,
                end_line: end as u32,
                language,
                suspicious: false,
            });
        }

        if end >= lines.len() {
            break;
        }

        start = end - OVERLAP_LINES;
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_chunking() {
        let text = "line1\nline2\nline3";
        let chunks = chunk_by_lines(text, Language::Unknown).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("line1"));
    }

    #[test]
    fn test_empty_text() {
        let chunks = chunk_by_lines("", Language::Unknown).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_single_line() {
        let chunks = chunk_by_lines("single", Language::Rust).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 1);
    }

    #[test]
    fn test_whitespace_only_lines() {
        let text = "   \n\t\n   ";
        let chunks = chunk_by_lines(text, Language::Unknown).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_line_numbers() {
        let text = "a\nb\nc";
        let chunks = chunk_by_lines(text, Language::Unknown).unwrap();
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
    }

    #[test]
    fn test_language_preserved() {
        let chunks = chunk_by_lines("code", Language::Python).unwrap();
        assert_eq!(chunks[0].language, Language::Python);
    }

    #[test]
    fn test_long_file_creates_overlapping_chunks() {
        let lines: Vec<&str> = (0..100).map(|_| "line content here").collect();
        let text = lines.join("\n");
        let chunks = chunk_by_lines(&text, Language::Unknown).unwrap();
        assert!(chunks.len() > 1);
        if chunks.len() >= 2 {
            assert!(chunks[1].start_line < chunks[0].end_line);
        }
    }
}
