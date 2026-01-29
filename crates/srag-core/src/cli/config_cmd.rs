// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;
use std::io::{self, Write};

use crate::config::{ApiProvider, Config};

pub async fn show() -> Result<()> {
    let config = Config::load()?;
    let content = toml::to_string_pretty(&config)?;
    println!("{}", content);
    Ok(())
}

pub async fn set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;

    match key {
        "indexing.max_file_size_bytes" => {
            config.indexing.max_file_size_bytes = value.parse()?;
        }
        "indexing.batch_size" => {
            config.indexing.batch_size = value.parse()?;
        }
        "indexing.throttle_ms" => {
            config.indexing.throttle_ms = value.parse()?;
        }
        "query.top_k" => {
            config.query.top_k = value.parse()?;
        }
        "query.ef_search" => {
            config.query.ef_search = value.parse()?;
        }
        "query.context_tokens" => {
            config.query.context_tokens = value.parse()?;
        }
        "query.history_turns" => {
            config.query.history_turns = value.parse()?;
        }
        "query.temperature" => {
            config.query.temperature = value.parse()?;
        }
        "query.max_tokens" => {
            config.query.max_tokens = value.parse()?;
        }
        "query.rerank" => {
            config.query.rerank = value.parse()?;
        }
        "query.broad_k" => {
            config.query.broad_k = value.parse()?;
        }
        "query.hybrid_search" => {
            config.query.hybrid_search = value.parse()?;
        }
        "watcher.debounce_ms" => {
            config.watcher.debounce_ms = value.parse()?;
        }
        "resource.nice_level" => {
            config.resource.nice_level = value.parse()?;
        }
        "resource.llm_idle_timeout_secs" => {
            config.resource.llm_idle_timeout_secs = value.parse()?;
        }
        "resource.memory_budget_mb" => {
            config.resource.memory_budget_mb = value.parse()?;
        }
        "llm.model_filename" => {
            config.llm.model_filename = value.to_string();
        }
        "llm.model_url" => {
            config.llm.model_url = value.to_string();
        }
        "llm.threads" => {
            config.llm.threads = value.parse()?;
        }
        "llm.context_size" => {
            config.llm.context_size = value.parse()?;
        }
        "api.provider" => {
            config.api.provider = match value.to_lowercase().as_str() {
                "local" => crate::config::ApiProvider::Local,
                "anthropic" | "claude" => crate::config::ApiProvider::Anthropic,
                "openai" | "gpt" => crate::config::ApiProvider::OpenAI,
                _ => anyhow::bail!(
                    "Invalid provider: {}. Use 'local', 'anthropic', or 'openai'",
                    value
                ),
            };
        }
        "api.model" => {
            config.api.model = value.to_string();
        }
        "api.max_tokens" => {
            config.api.max_tokens = value.parse()?;
        }
        "api.redact_secrets" => {
            config.api.redact_secrets = value.parse()?;
        }
        "api.log_redactions" => {
            config.api.log_redactions = value.parse()?;
        }
        _ => {
            anyhow::bail!("Unknown config key: {}", key);
        }
    }

    config.save()?;
    println!("Set {} = {}", key, value);
    Ok(())
}

pub async fn reset() -> Result<()> {
    let config = Config::default();
    config.save()?;
    println!("Configuration reset to defaults");
    Ok(())
}

pub async fn set_api_key(key: Option<&str>) -> Result<()> {
    let config = Config::load()?;
    let key_path = config.api_key_path();

    if let Some(parent) = key_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let api_key = match key {
        Some(k) => k.to_string(),
        None => {
            eprint!("Enter API key (input hidden): ");
            io::stderr().flush()?;
            rpassword::read_password()?
        }
    };

    if api_key.trim().is_empty() {
        // remove the key file
        if key_path.exists() {
            std::fs::remove_file(&key_path)?;
            println!("API key removed");
        } else {
            println!("No API key configured");
        }
        return Ok(());
    }

    // write with restrictive permissions
    {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&key_path)?;
            f.write_all(api_key.trim().as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&key_path, api_key.trim())?;
        }
    }

    println!("API key saved to {}", key_path.display());
    println!("Permissions: 600 (owner read/write only)");
    Ok(())
}

pub fn read_api_key() -> Result<Option<String>> {
    let config = Config::load()?;
    let key_path = config.api_key_path();

    if !key_path.exists() {
        return Ok(None);
    }

    let key = std::fs::read_to_string(&key_path)?;
    Ok(Some(key.trim().to_string()))
}

pub async fn check_api_safety() -> Result<()> {
    let config = Config::load()?;

    if config.api.provider == ApiProvider::Local {
        println!("Using local LLM - no data sent to external APIs");
        return Ok(());
    }

    let db_path = config.db_path();
    if !db_path.exists() {
        println!("No existing index - safe to use external API");
        return Ok(());
    }

    let store = crate::index::store::Store::open(&db_path)?;
    let file_count = store.file_count(None)?;
    let chunk_count = store.chunk_count(None)?;

    if file_count == 0 {
        println!("No existing index data - safe to use external API");
        return Ok(());
    }

    eprintln!("WARNING: External API mode with existing index data!");
    eprintln!();
    eprintln!(
        "Your index contains {} files and {} chunks that were indexed",
        file_count, chunk_count
    );
    eprintln!("when local mode was enabled. These may contain sensitive data");
    eprintln!("that could be sent to the external API during queries.");
    eprintln!();
    eprintln!("Recommendations:");
    eprintln!("  1. Re-index with 'srag sync' to refresh all chunks");
    eprintln!("  2. Or reset with 'srag config reset-index' to clear all data");
    eprintln!();
    eprintln!(
        "Secret redaction is {} for queries.",
        if config.api.redact_secrets {
            "ENABLED"
        } else {
            "DISABLED"
        }
    );

    Ok(())
}

pub async fn edit() -> Result<()> {
    let path = Config::config_path();
    if !path.exists() {
        let config = Config::default();
        config.save()?;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let status = std::process::Command::new(&editor).arg(&path).status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    // validate the edited config
    Config::load()?;
    println!("Configuration saved");
    Ok(())
}
