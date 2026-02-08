// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub auto_index_cwd: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_throttle_ms")]
    pub throttle_ms: u64,
    #[serde(default)]
    pub include_dependencies: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryConfig {
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default = "default_ef_search")]
    pub ef_search: usize,
    #[serde(default = "default_context_tokens")]
    pub context_tokens: usize,
    #[serde(default = "default_history_turns")]
    pub history_turns: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_rerank")]
    pub rerank: bool,
    #[serde(default = "default_broad_k")]
    pub broad_k: usize,
    #[serde(default = "default_hybrid_search")]
    pub hybrid_search: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    #[serde(default = "default_nice_level")]
    pub nice_level: i32,
    #[serde(default = "default_llm_idle_timeout_secs")]
    pub llm_idle_timeout_secs: u64,
    #[serde(default = "default_memory_budget_mb")]
    pub memory_budget_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_model_filename")]
    pub model_filename: String,
    #[serde(default = "default_model_url")]
    pub model_url: String,
    #[serde(default = "default_llm_threads")]
    pub threads: usize,
    #[serde(default = "default_llm_context_size")]
    pub context_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApiProvider {
    #[default]
    Local,
    Anthropic,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default)]
    pub provider: ApiProvider,
    #[serde(default = "default_api_model")]
    pub model: String,
    #[serde(default = "default_api_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_true")]
    pub redact_secrets: bool,
    #[serde(default = "default_true")]
    pub log_redactions: bool,
}

fn default_max_file_size() -> u64 {
    1_048_576
}
fn default_batch_size() -> usize {
    32
}
fn default_throttle_ms() -> u64 {
    10
}
fn default_top_k() -> usize {
    10
}
fn default_ef_search() -> usize {
    48
}
fn default_context_tokens() -> usize {
    2048
}
fn default_history_turns() -> usize {
    6
}
fn default_temperature() -> f32 {
    0.1
}
fn default_max_tokens() -> u32 {
    1024
}
fn default_rerank() -> bool {
    true
}
fn default_broad_k() -> usize {
    50
}
fn default_hybrid_search() -> bool {
    true
}
fn default_debounce_ms() -> u64 {
    500
}
fn default_nice_level() -> i32 {
    10
}
fn default_llm_idle_timeout_secs() -> u64 {
    300
}
fn default_memory_budget_mb() -> u64 {
    2048
}
fn default_model_filename() -> String {
    "Llama-3.2-1B-Instruct-Q4_K_M.gguf".into()
}
fn default_model_url() -> String {
    "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf".into()
}
fn default_llm_threads() -> usize {
    0
}
fn default_llm_context_size() -> usize {
    4096
}
fn default_api_model() -> String {
    "claude-sonnet-4-20250514".into()
}
fn default_api_max_tokens() -> u32 {
    2048
}
pub(crate) fn default_true() -> bool {
    true
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            max_file_size_bytes: default_max_file_size(),
            batch_size: default_batch_size(),
            throttle_ms: default_throttle_ms(),
            include_dependencies: false,
        }
    }
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            top_k: default_top_k(),
            ef_search: default_ef_search(),
            context_tokens: default_context_tokens(),
            history_turns: default_history_turns(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            rerank: default_rerank(),
            broad_k: default_broad_k(),
            hybrid_search: default_hybrid_search(),
        }
    }
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: default_debounce_ms(),
        }
    }
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            nice_level: default_nice_level(),
            llm_idle_timeout_secs: default_llm_idle_timeout_secs(),
            memory_budget_mb: default_memory_budget_mb(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            model_filename: default_model_filename(),
            model_url: default_model_url(),
            threads: default_llm_threads(),
            context_size: default_llm_context_size(),
        }
    }
}

const ALLOWED_MODEL_DOMAINS: &[&str] = &["huggingface.co", "github.com"];

impl LlmConfig {
    /// validate the model_url to ensure it uses https and points to allowed domains.
    pub fn validate(&self) -> Result<(), String> {
        if self.model_url.is_empty() {
            return Ok(());
        }

        if !self.model_url.starts_with("https://") {
            return Err(format!(
                "llm.model_url must use https, got: {}",
                self.model_url
            ));
        }

        let url_without_scheme = self.model_url.trim_start_matches("https://");
        let domain = url_without_scheme.split('/').next().unwrap_or("");

        let is_allowed = ALLOWED_MODEL_DOMAINS
            .iter()
            .any(|allowed| domain == *allowed || domain.ends_with(&format!(".{}", allowed)));

        if !is_allowed {
            return Err(format!(
                "llm.model_url must point to an allowed domain ({:?}), got: {}",
                ALLOWED_MODEL_DOMAINS, domain
            ));
        }

        Ok(())
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            provider: ApiProvider::Local,
            model: default_api_model(),
            max_tokens: default_api_max_tokens(),
            redact_secrets: true,
            log_redactions: true,
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            auto_index_cwd: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_provider_serde() {
        assert_eq!(
            serde_json::to_string(&ApiProvider::Local).unwrap(),
            "\"local\""
        );
        assert_eq!(
            serde_json::to_string(&ApiProvider::Anthropic).unwrap(),
            "\"anthropic\""
        );
        assert_eq!(
            serde_json::to_string(&ApiProvider::OpenAI).unwrap(),
            "\"openai\""
        );
    }

    #[test]
    fn test_indexing_defaults() {
        let config = IndexingConfig::default();
        assert_eq!(config.max_file_size_bytes, 1_048_576);
        assert_eq!(config.batch_size, 32);
        assert!(!config.include_dependencies);
    }

    #[test]
    fn test_query_defaults() {
        let config = QueryConfig::default();
        assert_eq!(config.top_k, 10);
        assert!(config.rerank);
        assert!(config.hybrid_search);
    }

    #[test]
    fn test_llm_config_validate_valid_url() {
        let config = LlmConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_llm_config_validate_empty_url() {
        let mut config = LlmConfig::default();
        config.model_url = String::new();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_llm_config_validate_http_rejected() {
        let mut config = LlmConfig::default();
        config.model_url = "http://huggingface.co/model.gguf".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_llm_config_validate_invalid_domain() {
        let mut config = LlmConfig::default();
        config.model_url = "https://evil.com/malware.gguf".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_llm_config_validate_allowed_domains() {
        let mut config = LlmConfig::default();

        config.model_url = "https://huggingface.co/model.gguf".into();
        assert!(config.validate().is_ok());

        config.model_url = "https://github.com/repo/model.gguf".into();
        assert!(config.validate().is_ok());
    }
}
