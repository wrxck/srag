// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::{
    ConversationTurn, EmbeddingResult, GenerationResult, ModelStatus, QueryResult, SourceReference,
};

#[test]
fn test_embedding_result_serialization() {
    let result = EmbeddingResult {
        vectors: vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
    };

    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: EmbeddingResult = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.vectors.len(), 2);
    assert_eq!(parsed.vectors[0], vec![0.1, 0.2, 0.3]);
}

#[test]
fn test_embedding_result_empty() {
    let result = EmbeddingResult { vectors: vec![] };
    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: EmbeddingResult = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.vectors.is_empty());
}

#[test]
fn test_embedding_result_single_vector() {
    let result = EmbeddingResult {
        vectors: vec![vec![0.5; 384]],
    };
    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: EmbeddingResult = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.vectors.len(), 1);
    assert_eq!(parsed.vectors[0].len(), 384);
}

#[test]
fn test_generation_result() {
    let result = GenerationResult {
        text: "Generated output text".into(),
        tokens_used: 150,
    };

    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: GenerationResult = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.text, "Generated output text");
    assert_eq!(parsed.tokens_used, 150);
}

#[test]
fn test_generation_result_empty_text() {
    let result = GenerationResult {
        text: "".into(),
        tokens_used: 0,
    };
    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: GenerationResult = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.text.is_empty());
    assert_eq!(parsed.tokens_used, 0);
}

#[test]
fn test_model_status_full() {
    let status = ModelStatus {
        embedder_loaded: true,
        llm_loaded: false,
        reranker_loaded: true,
        embedder_memory_mb: Some(512.5),
        llm_memory_mb: None,
        reranker_memory_mb: Some(256.0),
    };

    let json_str = serde_json::to_string(&status).unwrap();
    let parsed: ModelStatus = serde_json::from_str(&json_str).unwrap();

    assert!(parsed.embedder_loaded);
    assert!(!parsed.llm_loaded);
    assert!(parsed.reranker_loaded);
    assert_eq!(parsed.embedder_memory_mb, Some(512.5));
    assert_eq!(parsed.llm_memory_mb, None);
}

#[test]
fn test_model_status_defaults() {
    let json_str = r#"{
        "embedder_loaded": true,
        "llm_loaded": false,
        "embedder_memory_mb": null,
        "llm_memory_mb": null
    }"#;

    let status: ModelStatus = serde_json::from_str(json_str).unwrap();
    assert!(!status.reranker_loaded);
    assert!(status.reranker_memory_mb.is_none());
}

#[test]
fn test_model_status_all_unloaded() {
    let status = ModelStatus {
        embedder_loaded: false,
        llm_loaded: false,
        reranker_loaded: false,
        embedder_memory_mb: None,
        llm_memory_mb: None,
        reranker_memory_mb: None,
    };
    let json_str = serde_json::to_string(&status).unwrap();
    let parsed: ModelStatus = serde_json::from_str(&json_str).unwrap();
    assert!(!parsed.embedder_loaded);
    assert!(!parsed.llm_loaded);
    assert!(!parsed.reranker_loaded);
}

#[test]
fn test_source_reference() {
    let source = SourceReference {
        file_path: "src/main.rs".into(),
        start_line: 10,
        end_line: 20,
        symbol: Some("main".into()),
        content: "fn main() { println!(\"Hello\"); }".into(),
    };

    let json_str = serde_json::to_string(&source).unwrap();
    let parsed: SourceReference = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.file_path, "src/main.rs");
    assert_eq!(parsed.start_line, 10);
    assert_eq!(parsed.end_line, 20);
    assert_eq!(parsed.symbol, Some("main".into()));
}

#[test]
fn test_source_reference_no_symbol() {
    let source = SourceReference {
        file_path: "README.md".into(),
        start_line: 1,
        end_line: 5,
        symbol: None,
        content: "# Title".into(),
    };
    let json_str = serde_json::to_string(&source).unwrap();
    let parsed: SourceReference = serde_json::from_str(&json_str).unwrap();
    assert!(parsed.symbol.is_none());
}

#[test]
fn test_query_result() {
    let result = QueryResult {
        answer: "The main function is in src/main.rs".into(),
        sources: vec![SourceReference {
            file_path: "src/main.rs".into(),
            start_line: 1,
            end_line: 5,
            symbol: Some("main".into()),
            content: "fn main() {}".into(),
        }],
    };

    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: QueryResult = serde_json::from_str(&json_str).unwrap();

    assert!(!parsed.answer.is_empty());
    assert_eq!(parsed.sources.len(), 1);
}

#[test]
fn test_query_result_empty_sources() {
    let result = QueryResult {
        answer: "No relevant sources found".into(),
        sources: vec![],
    };

    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: QueryResult = serde_json::from_str(&json_str).unwrap();

    assert!(parsed.sources.is_empty());
}

#[test]
fn test_query_result_multiple_sources() {
    let result = QueryResult {
        answer: "Found in multiple files".into(),
        sources: vec![
            SourceReference {
                file_path: "a.rs".into(),
                start_line: 1,
                end_line: 10,
                symbol: None,
                content: "code a".into(),
            },
            SourceReference {
                file_path: "b.rs".into(),
                start_line: 5,
                end_line: 15,
                symbol: None,
                content: "code b".into(),
            },
        ],
    };
    let json_str = serde_json::to_string(&result).unwrap();
    let parsed: QueryResult = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.sources.len(), 2);
}

#[test]
fn test_conversation_turn() {
    let turn = ConversationTurn {
        id: Some(1),
        session_id: "session-abc123".into(),
        role: "user".into(),
        content: "What does this function do?".into(),
        sources: Some(r#"[{"file": "main.rs"}]"#.into()),
        created_at: "2024-01-01T12:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&turn).unwrap();
    let parsed: ConversationTurn = serde_json::from_str(&json_str).unwrap();

    assert_eq!(parsed.session_id, "session-abc123");
    assert_eq!(parsed.role, "user");
    assert!(parsed.sources.is_some());
}

#[test]
fn test_conversation_turn_no_sources() {
    let turn = ConversationTurn {
        id: None,
        session_id: "session-xyz".into(),
        role: "assistant".into(),
        content: "This is the response".into(),
        sources: None,
        created_at: "2024-01-01T12:00:00Z".into(),
    };

    let json_str = serde_json::to_string(&turn).unwrap();
    let parsed: ConversationTurn = serde_json::from_str(&json_str).unwrap();

    assert!(parsed.sources.is_none());
}

#[test]
fn test_conversation_turn_roles() {
    for role in ["user", "assistant", "system"] {
        let turn = ConversationTurn {
            id: None,
            session_id: "test".into(),
            role: role.into(),
            content: "test".into(),
            sources: None,
            created_at: "2024-01-01".into(),
        };
        let json_str = serde_json::to_string(&turn).unwrap();
        let parsed: ConversationTurn = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.role, role);
    }
}
