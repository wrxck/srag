// SPDX-Licence-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod sections;

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use sections::{
    ApiConfig, ApiProvider, IndexingConfig, LlmConfig, McpConfig, QueryConfig, ResourceConfig,
    WatcherConfig,
};

pub const EMBEDDING_DIMENSION: usize = 384;

pub const DEPENDENCY_DIRS: &[&str] = &[
    "node_modules",
    "bower_components",
    "target",
    ".venv",
    "venv",
    "env",
    ".env",
    "__pycache__",
    ".eggs",
    "*.egg-info",
    ".tox",
    ".nox",
    "vendor",
    "vendor/bundle",
    ".gradle",
    ".m2",
    "build",
    "bin",
    "obj",
    "packages",
    "deps",
    "_build",
    ".stack-work",
    "dist-newstyle",
    "_opam",
    "Pods",
    ".build",
    "DerivedData",
    ".dart_tool",
    ".pub-cache",
    "zig-cache",
    "zig-out",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "Config::default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub source_dir: Option<String>,
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
    pub mcp: McpConfig,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: Self::default_data_dir(),
            source_dir: None,
            indexing: IndexingConfig::default(),
            query: QueryConfig::default(),
            watcher: WatcherConfig::default(),
            resource: ResourceConfig::default(),
            llm: LlmConfig::default(),
            api: ApiConfig::default(),
            mcp: McpConfig::default(),
            ignore_patterns: vec![
                "*.lock".into(),
                "*.min.js".into(),
                "*.min.css".into(),
                "*.map".into(),
                "*.wasm".into(),
                "*.pyc".into(),
                "*.o".into(),
                "*.so".into(),
                "*.dylib".into(),
                "*.dll".into(),
                "*.exe".into(),
                "*.bin".into(),
                "*.png".into(),
                "*.jpg".into(),
                "*.jpeg".into(),
                "*.gif".into(),
                "*.ico".into(),
                "*.svg".into(),
                "*.pdf".into(),
                "*.zip".into(),
                "*.tar".into(),
                "*.gz".into(),
                ".git".into(),
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
        if let Err(e) = self.llm.validate() {
            anyhow::bail!(e);
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

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
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

    /// returns the path for storing the API key.
    /// uses the config directory (not runtime dir) for security - runtime dirs may be world-readable.
    pub fn api_key_path(&self) -> PathBuf {
        Self::config_path()
            .parent()
            .unwrap_or(&PathBuf::from("."))
            .join("api.key")
    }

    /// write the API key to the key file with proper 0o600 permissions.
    pub fn write_api_key(&self, key: &str) -> Result<()> {
        let path = self.api_key_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(key.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&path, key)?;
        }
        Ok(())
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
        assert_eq!(config.indexing.max_file_size_bytes, 1_048_576);
    }
}
