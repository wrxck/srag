// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// embedding vector dimension for the default model (all-MiniLM-L6-v2)
pub const EMBEDDING_DIMENSION: usize = 384;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "Config::default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub indexing: IndexingConfig,
    #[serde(default)]
    pub query: QueryConfig,
    #[serde(default)]
    pub watcher: WatcherConfig,
    #[serde(default)]
    pub resource: ResourceConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: u64,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_throttle_ms")]
    pub throttle_ms: u64,
    /// include dependency directories (node_modules, vendor, etc). default: false
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ApiProvider {
    #[default]
    Local,
    Anthropic,
    OpenAI,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// which provider to use: local, anthropic, or openai
    #[serde(default)]
    pub provider: ApiProvider,
    /// model name for external API (e.g. "claude-sonnet-4-20250514", "gpt-4o")
    #[serde(default = "default_api_model")]
    pub model: String,
    /// max tokens for API response
    #[serde(default = "default_api_max_tokens")]
    pub max_tokens: u32,
    /// whether secret redaction is enabled (always true for external APIs)
    #[serde(default = "default_true")]
    pub redact_secrets: bool,
    /// log when secrets are redacted
    #[serde(default = "default_true")]
    pub log_redactions: bool,
}

fn default_max_file_size() -> u64 {
    1_048_576
} // 1MB
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
fn default_true() -> bool {
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

/// Common dependency/vendor directories across languages.
/// Excluded by default unless `include_dependencies` is true.
pub const DEPENDENCY_DIRS: &[&str] = &[
    // JavaScript/TypeScript
    "node_modules",
    "bower_components",
    // Rust
    "target",
    // Python
    ".venv",
    "venv",
    "env",
    ".env",
    "__pycache__",
    ".eggs",
    "*.egg-info",
    ".tox",
    ".nox",
    // Go
    "vendor",
    // Ruby
    "vendor/bundle",
    // PHP
    "vendor",
    // Java/Kotlin/Scala
    ".gradle",
    ".m2",
    "build",
    // .NET/C#
    "bin",
    "obj",
    "packages",
    // Elixir
    "deps",
    "_build",
    // Haskell
    ".stack-work",
    "dist-newstyle",
    // OCaml
    "_opam",
    // Swift/iOS
    "Pods",
    ".build",
    "DerivedData",
    // Dart/Flutter
    ".dart_tool",
    ".pub-cache",
    // Zig
    "zig-cache",
    "zig-out",
];

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

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: Self::default_data_dir(),
            indexing: IndexingConfig::default(),
            query: QueryConfig::default(),
            watcher: WatcherConfig::default(),
            resource: ResourceConfig::default(),
            llm: LlmConfig::default(),
            api: ApiConfig::default(),
            ignore_patterns: vec![
                // lock files
                "*.lock".into(),
                // minified/generated assets
                "*.min.js".into(),
                "*.min.css".into(),
                "*.map".into(),
                "*.wasm".into(),
                // compiled artifacts
                "*.pyc".into(),
                "*.o".into(),
                "*.so".into(),
                "*.dylib".into(),
                "*.dll".into(),
                "*.exe".into(),
                "*.bin".into(),
                // images/media
                "*.png".into(),
                "*.jpg".into(),
                "*.jpeg".into(),
                "*.gif".into(),
                "*.ico".into(),
                "*.svg".into(),
                "*.pdf".into(),
                // archives
                "*.zip".into(),
                "*.tar".into(),
                "*.gz".into(),
                // vcs
                ".git".into(),
                // build output (not dependencies)
                "dist".into(),
            ],
        }
    }
}

impl Config {
    fn default_data_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("srag")
    }

    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("srag")
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let config = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read config from {}", path.display()))?;
            let config: Config =
                toml::from_str(&content).with_context(|| "failed to parse config")?;
            config
        } else {
            Config::default()
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.indexing.batch_size == 0 {
            anyhow::bail!("indexing.batch_size must be > 0");
        }
        if self.indexing.max_file_size_bytes == 0 {
            anyhow::bail!("indexing.max_file_size_bytes must be > 0");
        }
        if self.query.top_k == 0 {
            anyhow::bail!("query.top_k must be > 0");
        }
        if self.query.ef_search == 0 {
            anyhow::bail!("query.ef_search must be > 0");
        }
        if self.query.context_tokens == 0 {
            anyhow::bail!("query.context_tokens must be > 0");
        }
        if self.query.max_tokens == 0 {
            anyhow::bail!("query.max_tokens must be > 0");
        }
        if self.query.temperature < 0.0 || self.query.temperature > 2.0 {
            anyhow::bail!("query.temperature must be between 0.0 and 2.0");
        }
        if self.query.broad_k == 0 {
            anyhow::bail!("query.broad_k must be > 0");
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let content = toml::to_string_pretty(self).with_context(|| "failed to serialise config")?;
        std::fs::write(&path, &content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("metadata.db")
    }

    pub fn vectors_dir(&self) -> PathBuf {
        self.data_dir.join("vectors")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn runtime_dir() -> PathBuf {
        #[cfg(unix)]
        {
            std::env::var("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/tmp"))
                .join("srag")
        }
        #[cfg(windows)]
        {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("srag")
                .join("run")
        }
    }

    pub fn port_file_path() -> PathBuf {
        Self::runtime_dir().join("ml.port")
    }

    pub fn token_file_path() -> PathBuf {
        Self::runtime_dir().join("ml.token")
    }

    pub fn watcher_pid_path() -> PathBuf {
        Self::runtime_dir().join("watcher.pid")
    }

    pub fn api_key_path(&self) -> PathBuf {
        Self::runtime_dir().join("api.key")
    }

    pub fn is_external_api(&self) -> bool {
        self.api.provider != ApiProvider::Local
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.data_dir,
            &self.vectors_dir(),
            &self.models_dir(),
            &self.logs_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let runtime = Self::runtime_dir();
        std::fs::create_dir_all(&runtime)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.indexing.max_file_size_bytes > 0);
        assert!(config.query.top_k > 0);
        assert!(!config.ignore_patterns.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.indexing.batch_size, config.indexing.batch_size);
    }

    #[test]
    fn test_config_validation_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_batch_size() {
        let mut config = Config::default();
        config.indexing.batch_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_max_file_size() {
        let mut config = Config::default();
        config.indexing.max_file_size_bytes = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_top_k() {
        let mut config = Config::default();
        config.query.top_k = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_temperature() {
        let mut config = Config::default();
        config.query.temperature = 3.0;
        assert!(config.validate().is_err());

        config.query.temperature = -0.5;
        assert!(config.validate().is_err());
    }

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
    fn test_is_external_api() {
        let mut config = Config::default();
        assert!(!config.is_external_api());

        config.api.provider = ApiProvider::Anthropic;
        assert!(config.is_external_api());

        config.api.provider = ApiProvider::OpenAI;
        assert!(config.is_external_api());
    }

    #[test]
    fn test_path_methods() {
        let config = Config::default();
        assert!(config.db_path().ends_with("metadata.db"));
        assert!(config.vectors_dir().ends_with("vectors"));
        assert!(config.models_dir().ends_with("models"));
        assert!(config.logs_dir().ends_with("logs"));
    }

    #[test]
    fn test_dependency_dirs_constant() {
        assert!(DEPENDENCY_DIRS.contains(&"node_modules"));
        assert!(DEPENDENCY_DIRS.contains(&"target"));
        assert!(DEPENDENCY_DIRS.contains(&".venv"));
        assert!(DEPENDENCY_DIRS.contains(&"vendor"));
    }

    #[test]
    fn test_toml_partial_config() {
        let toml_str = r#"
[indexing]
batch_size = 64
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.indexing.batch_size, 64);
        assert_eq!(config.indexing.max_file_size_bytes, default_max_file_size());
    }
}
