// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod store_callgraph;
mod store_chunks;
mod store_embeddings;
mod store_file;
mod store_project;
mod store_query;
mod store_session;
mod store_stats;

use std::path::Path;

use rusqlite::Connection;
use srag_common::{Error, Result};

pub(crate) fn escape_like_pattern(pattern: &str) -> String {
    pattern
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| Error::Sqlite(e.to_string()))?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;

            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_indexed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                path TEXT NOT NULL,
                blake3_hash TEXT NOT NULL,
                language TEXT NOT NULL DEFAULT 'unknown',
                size_bytes INTEGER NOT NULL DEFAULT 0,
                chunk_count INTEGER NOT NULL DEFAULT 0,
                indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_id, path)
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                symbol TEXT,
                symbol_kind TEXT,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                language TEXT NOT NULL DEFAULT 'unknown',
                embedding_id INTEGER
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                project_name TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                sources TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS reindex_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                file_path TEXT NOT NULL,
                event_type TEXT NOT NULL,
                queued_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(project_id, file_path)
            );

            CREATE TABLE IF NOT EXISTS embeddings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id INTEGER NOT NULL UNIQUE REFERENCES chunks(id) ON DELETE CASCADE,
                vector BLOB NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                chunk_id UNINDEXED,
                content,
                file_path,
                symbol,
                tokenize='porter unicode61'
            );

            CREATE INDEX IF NOT EXISTS idx_files_project ON files(project_id);
            CREATE INDEX IF NOT EXISTS idx_files_hash ON files(blake3_hash);
            CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_embedding ON chunks(embedding_id);
            CREATE INDEX IF NOT EXISTS idx_embeddings_chunk ON embeddings(chunk_id);
            CREATE INDEX IF NOT EXISTS idx_turns_session ON turns(session_id);
            CREATE INDEX IF NOT EXISTS idx_reindex_queue_project ON reindex_queue(project_id);

            CREATE TABLE IF NOT EXISTS definitions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id INTEGER NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
                file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                scope TEXT,
                language TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                signature TEXT,
                UNIQUE(file_id, name, scope, start_line)
            );

            CREATE TABLE IF NOT EXISTS function_calls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id INTEGER NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
                file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
                caller_name TEXT,
                caller_scope TEXT,
                callee_name TEXT NOT NULL,
                line_number INTEGER NOT NULL,
                language TEXT NOT NULL,
                callee_definition_id INTEGER REFERENCES definitions(id) ON DELETE SET NULL
            );

            CREATE INDEX IF NOT EXISTS idx_definitions_file ON definitions(file_id);
            CREATE INDEX IF NOT EXISTS idx_definitions_name ON definitions(name);
            CREATE INDEX IF NOT EXISTS idx_definitions_chunk ON definitions(chunk_id);
            CREATE INDEX IF NOT EXISTS idx_calls_file ON function_calls(file_id);
            CREATE INDEX IF NOT EXISTS idx_calls_callee ON function_calls(callee_name);
            CREATE INDEX IF NOT EXISTS idx_calls_caller ON function_calls(caller_name);
            CREATE INDEX IF NOT EXISTS idx_calls_definition ON function_calls(callee_definition_id);
            ",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        // migration: add suspicious column to chunks (ignore error if already exists)
        let _ = self
            .conn
            .execute_batch("ALTER TABLE chunks ADD COLUMN suspicious INTEGER NOT NULL DEFAULT 0;");

        Ok(())
    }

    pub fn begin_transaction(&self) -> Result<()> {
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        self.conn
            .execute_batch("COMMIT")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn rollback(&self) -> Result<()> {
        self.conn
            .execute_batch("ROLLBACK")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn wal_checkpoint(&self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ProjectPatterns {
    pub languages: Vec<(String, u64)>,
    pub symbol_kinds: Vec<(String, u64)>,
    pub common_prefixes: Vec<(String, u64)>,
    pub directories: Vec<(String, u64)>,
}

#[derive(Debug, Clone)]
pub struct LanguageStats {
    pub language: String,
    pub file_count: u64,
    pub chunk_count: u64,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tempfile::tempdir;

    pub fn test_store() -> (Store, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        (store, dir)
    }

    #[test]
    fn test_store_open_and_init() {
        let (_store, _dir) = test_store();
    }

    #[test]
    fn test_transaction_commit() {
        let (store, _dir) = test_store();
        store.begin_transaction().unwrap();
        store.upsert_project("proj", "/tmp").unwrap();
        store.commit().unwrap();

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let (store, _dir) = test_store();
        store.upsert_project("existing", "/tmp").unwrap();

        store.begin_transaction().unwrap();
        store.upsert_project("new", "/tmp2").unwrap();
        store.rollback().unwrap();

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "existing");
    }

    #[test]
    fn test_escape_like_pattern_special_chars() {
        assert_eq!(escape_like_pattern("test%pattern"), "test\\%pattern");
        assert_eq!(escape_like_pattern("test_pattern"), "test\\_pattern");
        assert_eq!(escape_like_pattern("test\\pattern"), "test\\\\pattern");
        assert_eq!(escape_like_pattern("normal"), "normal");
    }
}
