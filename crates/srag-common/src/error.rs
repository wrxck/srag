// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Index error: {0}")]
    Index(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Chunking error: {0}")]
    Chunking(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Watcher error: {0}")]
    Watcher(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SQLite error: {0}")]
    Sqlite(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
