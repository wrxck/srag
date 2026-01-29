// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{Chunk, Language};

pub fn chunk_env_file(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        // skip empty lines and comments
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

pub fn chunk_toml_file(text: &str, language: Language) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_section = String::new();
    let mut current_lines = Vec::new();
    let mut start_line: u32 = 1;

    for (i, line) in text.lines().enumerate() {
        let line_num = (i + 1) as u32;
        let trimmed = line.trim();

        if trimmed.starts_with('[') && trimmed.contains(']') {
            // flush previous section
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

    // flush last section
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

pub fn chunk_yaml_file(text: &str, language: Language) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_key = String::new();
    let mut current_lines = Vec::new();
    let mut start_line: u32 = 1;

    for (i, line) in text.lines().enumerate() {
        let line_num = (i + 1) as u32;

        // top-level key: starts at column 0, is not a comment, not blank
        let is_top_level = !line.is_empty()
            && !line.starts_with(' ')
            && !line.starts_with('\t')
            && !line.starts_with('#')
            && !line.starts_with("---")
            && !line.starts_with("...");

        if is_top_level {
            // flush previous block
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

    // flush last block
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

    // scan the raw text to find top-level key boundaries
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut depth: i32 = 0;
    let mut i = 0;
    let mut line: u32 = 1;

    // find opening brace
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

    // parse top-level key-value pairs
    while i < chars.len() && depth >= 1 {
        let ch = chars[i];

        if ch == '\n' {
            line += 1;
        }

        // skip whitespace and commas between entries at depth 1
        if depth == 1 && (ch.is_whitespace() || ch == ',') {
            i += 1;
            continue;
        }

        // closing brace at depth 1 means end of object
        if depth == 1 && ch == '}' {
            break;
        }

        // expect a quoted key at depth 1
        if depth == 1 && ch == '"' {
            let key_start_line = line;
            // parse key string
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
            i += 1; // skip closing quote

            // skip colon
            while i < chars.len() && chars[i] != ':' {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 1; // skip colon

            // skip whitespace before value
            while i < chars.len() && chars[i].is_whitespace() {
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }

            // capture the value by tracking depth
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
                            // this closing brace belongs to the outer object
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

        // unexpected character at depth 1
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
