// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use serde::{Deserialize, Serialize};

/// supported programming languages for syntax-aware chunking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
    Java,
    Ruby,
    Shell,
    Markdown,
    Toml,
    Yaml,
    Json,
    Html,
    Css,
    Sql,
    Env,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "rs" => Language::Rust,
            "py" | "pyi" => Language::Python,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "ts" | "mts" | "cts" | "tsx" | "jsx" => Language::TypeScript,
            "go" => Language::Go,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Language::Cpp,
            "java" => Language::Java,
            "rb" => Language::Ruby,
            "sh" | "bash" | "zsh" | "fish" => Language::Shell,
            "md" | "mdx" => Language::Markdown,
            "toml" => Language::Toml,
            "yml" | "yaml" => Language::Yaml,
            "json" => Language::Json,
            "html" | "htm" => Language::Html,
            "css" | "scss" | "less" => Language::Css,
            "sql" => Language::Sql,
            _ => Language::Unknown,
        }
    }

    pub fn from_filename(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        if lower == ".env" || lower.starts_with(".env.") || lower.ends_with(".env") {
            return Some(Language::Env);
        }
        None
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::JavaScript => "javascript",
            Language::TypeScript => "typescript",
            Language::Go => "go",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::Java => "java",
            Language::Ruby => "ruby",
            Language::Shell => "shell",
            Language::Markdown => "markdown",
            Language::Toml => "toml",
            Language::Yaml => "yaml",
            Language::Json => "json",
            Language::Html => "html",
            Language::Css => "css",
            Language::Sql => "sql",
            Language::Env => "env",
            Language::Unknown => "unknown",
        }
    }

    pub fn has_tree_sitter_support(&self) -> bool {
        matches!(
            self,
            Language::Rust
                | Language::Python
                | Language::JavaScript
                | Language::TypeScript
                | Language::Go
                | Language::C
                | Language::Cpp
                | Language::Java
                | Language::Ruby
        )
    }
}

/// a chunk of source code extracted from a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// unique id for this chunk (assigned by sqlite)
    pub id: Option<i64>,
    /// id of the file this chunk belongs to
    pub file_id: i64,
    /// the chunk text content
    pub content: String,
    /// symbol name if extracted via AST (e.g., function name)
    pub symbol: Option<String>,
    /// symbol kind (function, class, struct, method, etc.)
    pub symbol_kind: Option<String>,
    /// start line in the source file (1-indexed)
    pub start_line: u32,
    /// end line in the source file (1-indexed)
    pub end_line: u32,
    /// language of the chunk
    pub language: Language,
    /// flagged by injection scanner as containing suspicious patterns
    #[serde(default)]
    pub suspicious: bool,
}

/// metadata for an indexed file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: Option<i64>,
    pub project_id: i64,
    pub path: String,
    pub blake3_hash: String,
    pub language: Language,
    pub size_bytes: u64,
    pub chunk_count: u32,
    pub indexed_at: String,
}

/// a project (indexed directory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Option<i64>,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub last_indexed_at: Option<String>,
}

/// ipc request/response types for JSON-RPC communication with the python ML service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

impl JsonRpcRequest {
    pub fn new(method: &str, params: serde_json::Value, id: u64) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
            id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// embedding result from the ML service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub vectors: Vec<Vec<f32>>,
}

/// LLM generation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub text: String,
    pub tokens_used: u32,
}

/// status of the ML service models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub embedder_loaded: bool,
    pub llm_loaded: bool,
    #[serde(default)]
    pub reranker_loaded: bool,
    pub embedder_memory_mb: Option<f32>,
    pub llm_memory_mb: Option<f32>,
    #[serde(default)]
    pub reranker_memory_mb: Option<f32>,
}

/// a source reference from a query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReference {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub symbol: Option<String>,
    pub content: String,
}

/// result of a single query operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub answer: String,
    pub sources: Vec<SourceReference>,
}

/// a conversation turn for chat history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub id: Option<i64>,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub sources: Option<String>,
    pub created_at: String,
}
