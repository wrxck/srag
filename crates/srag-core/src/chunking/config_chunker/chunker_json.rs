// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};

pub fn chunk_json_file(text: &str, language: Language) -> Vec<Chunk> {
    let total_lines = text.lines().count().max(1) as u32;
    let single_chunk = || {
        vec![Chunk {
            id: None,
            file_id: 0,
            content: text.to_string(),
            symbol: None,
            symbol_kind: None,
            start_line: 1,
            end_line: total_lines,
            language,
            suspicious: false,
        }]
    };

    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut depth: i32 = 0;
    let mut i = 0;
    let mut line: u32 = 1;

    while i < chars.len() {
        if chars[i] == '\n' {
            line += 1;
        }
        if !chars[i].is_whitespace() {
            if chars[i] == '{' {
                depth = 1;
                i += 1;
                break;
            } else {
                return single_chunk();
            }
        }
        i += 1;
    }

    while i < chars.len() && depth >= 1 {
        let ch = chars[i];

        if ch == '\n' {
            line += 1;
        }

        if depth == 1 && (ch.is_whitespace() || ch == ',') {
            i += 1;
            continue;
        }

        if depth == 1 && ch == '}' {
            break;
        }

        if depth == 1 && ch == '"' {
            let key_start_line = line;
            i += 1;
            let mut key = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                    if i < chars.len() {
                        key.push(chars[i]);
                    }
                } else {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    key.push(chars[i]);
                }
                i += 1;
            }
            i += 1;

            while i < chars.len() && chars[i] != ':' {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 1;

            while i < chars.len() && chars[i].is_whitespace() {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }

            let value_start = i;
            let value_start_line = key_start_line;
            let mut value_depth: i32 = 0;
            let mut val_in_string = false;
            let mut val_escape = false;

            loop {
                if i >= chars.len() {
                    break;
                }
                let vc = chars[i];
                if vc == '\n' {
                    line += 1;
                }

                if val_escape {
                    val_escape = false;
                    i += 1;
                    continue;
                }

                if val_in_string {
                    if vc == '\\' {
                        val_escape = true;
                    } else if vc == '"' {
                        val_in_string = false;
                    }
                    i += 1;
                    continue;
                }

                match vc {
                    '"' => val_in_string = true,
                    '{' | '[' => value_depth += 1,
                    '}' | ']' => {
                        if value_depth == 0 {
                            break;
                        }
                        value_depth -= 1;
                        if value_depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    ',' if value_depth == 0 => break,
                    _ => {}
                }
                i += 1;
            }

            let value_end_line = line;
            let raw_slice: String = chars[value_start..i].iter().collect();
            let content = format!("\"{}\": {}", key, raw_slice.trim());

            chunks.push(Chunk {
                id: None,
                file_id: 0,
                content,
                symbol: Some(key),
                symbol_kind: Some("key".to_string()),
                start_line: value_start_line,
                end_line: value_end_line,
                language,
                suspicious: false,
            });

            continue;
        }

        i += 1;
    }

    if chunks.is_empty() {
        return single_chunk();
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_simple_object() {
        let text = r#"{"name": "test", "version": "1.0"}"#;
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().any(|c| c.symbol == Some("name".to_string())));
        assert!(chunks
            .iter()
            .any(|c| c.symbol == Some("version".to_string())));
    }

    #[test]
    fn test_json_nested_object() {
        let text = r#"{"outer": {"inner": "value"}}"#;
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("outer".to_string()));
    }

    #[test]
    fn test_json_array_value() {
        let text = r#"{"items": [1, 2, 3]}"#;
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("items".to_string()));
    }

    #[test]
    fn test_json_empty_object() {
        let text = "{}";
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].symbol.is_none());
    }

    #[test]
    fn test_json_not_an_object() {
        let text = "[1, 2, 3]";
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].symbol.is_none());
    }

    #[test]
    fn test_json_escaped_quotes() {
        let text = r#"{"key": "value with \"quotes\""}"#;
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol, Some("key".to_string()));
    }

    #[test]
    fn test_json_multiline() {
        let text = r#"{
  "name": "test",
  "nested": {
    "key": "value"
  }
}"#;
        let chunks = chunk_json_file(text, Language::Json);
        assert_eq!(chunks.len(), 2);
    }
}
