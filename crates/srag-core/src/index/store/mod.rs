// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod store_chunks;
mod store_embeddings;
mod store_session;

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use srag_common::types::{FileRecord, Project};
use srag_common::{Error, Result};

fn escape_like_pattern(pattern: &str) -> String {
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

    // -- project operations --

    pub fn upsert_project(&self, name: &str, path: &str) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO projects (name, path) VALUES (?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET path = ?2",
                params![name, path],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let id = self.conn.last_insert_rowid();
        if id == 0 {
            return self.get_project_id(name);
        }
        Ok(id)
    }

    pub fn get_project_id(&self, name: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT id FROM projects WHERE name = ?1",
                params![name],
                |row| row.get(0),
            )
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn delete_project(&self, project_id: i64) -> Result<()> {
        self.delete_project_chunks_fts(project_id)?;
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![project_id])
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn update_project_indexed_at(&self, project_id: i64) -> Result<()> {
        self.conn
            .execute(
                "UPDATE projects SET last_indexed_at = datetime('now') WHERE id = ?1",
                params![project_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, path, created_at, last_indexed_at FROM projects ORDER BY name",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                    last_indexed_at: row.get(4)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
        }
        Ok(projects)
    }

    // -- file operations --

    pub fn get_file_hash(&self, project_id: i64, path: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT blake3_hash FROM files WHERE project_id = ?1 AND path = ?2",
                params![project_id, path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn upsert_file(&self, record: &FileRecord) -> Result<i64> {
        let lang = serde_json::to_value(record.language)
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        self.conn
            .execute(
                "INSERT INTO files (project_id, path, blake3_hash, language, size_bytes, chunk_count, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
                 ON CONFLICT(project_id, path) DO UPDATE SET
                    blake3_hash = ?3, language = ?4, size_bytes = ?5,
                    chunk_count = ?6, indexed_at = datetime('now')",
                params![
                    record.project_id,
                    record.path,
                    record.blake3_hash,
                    lang,
                    record.size_bytes,
                    record.chunk_count,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let file_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM files WHERE project_id = ?1 AND path = ?2",
                params![record.project_id, record.path],
                |row| row.get(0),
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(file_id)
    }

    pub fn delete_project_files(&self, project_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM files WHERE project_id = ?1",
                params![project_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    // -- statistics --

    pub fn file_count(&self, project_id: Option<i64>) -> Result<u64> {
        let count: i64 = if let Some(pid) = project_id {
            self.conn
                .query_row(
                    "SELECT COUNT(*) FROM files WHERE project_id = ?1",
                    params![pid],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Sqlite(e.to_string()))?
        } else {
            self.conn
                .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
                .map_err(|e| Error::Sqlite(e.to_string()))?
        };
        Ok(count as u64)
    }

    pub fn chunk_count(&self, project_id: Option<i64>) -> Result<u64> {
        let count: i64 = if let Some(pid) = project_id {
            self.conn
                .query_row(
                    "SELECT COUNT(*) FROM chunks c
                     JOIN files f ON c.file_id = f.id
                     WHERE f.project_id = ?1",
                    params![pid],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Sqlite(e.to_string()))?
        } else {
            self.conn
                .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
                .map_err(|e| Error::Sqlite(e.to_string()))?
        };
        Ok(count as u64)
    }

    pub fn total_size_bytes(&self, project_id: Option<i64>) -> Result<u64> {
        let size: i64 = if let Some(pid) = project_id {
            self.conn
                .query_row(
                    "SELECT COALESCE(SUM(size_bytes), 0) FROM files WHERE project_id = ?1",
                    params![pid],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Sqlite(e.to_string()))?
        } else {
            self.conn
                .query_row(
                    "SELECT COALESCE(SUM(size_bytes), 0) FROM files",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Sqlite(e.to_string()))?
        };
        Ok(size as u64)
    }

    pub fn find_project_by_path(&self, dir_path: &str) -> Result<Option<Project>> {
        let escaped = escape_like_pattern(dir_path);
        self.conn
            .query_row(
                "SELECT id, name, path, created_at, last_indexed_at FROM projects
                 WHERE path = ?1 OR ?2 LIKE path || '%' ESCAPE '\\'
                 ORDER BY LENGTH(path) DESC LIMIT 1",
                params![dir_path, escaped],
                |row| {
                    Ok(Project {
                        id: Some(row.get(0)?),
                        name: row.get(1)?,
                        path: row.get(2)?,
                        created_at: row.get(3)?,
                        last_indexed_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn search_symbols(
        &self,
        pattern: &str,
        project_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<(srag_common::types::Chunk, String)>> {
        let escaped = escape_like_pattern(pattern);
        let like_pattern = format!("%{}%", escaped);
        let query = if project_id.is_some() {
            "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                    c.start_line, c.end_line, c.language, f.path, c.suspicious
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE c.symbol LIKE ?1 ESCAPE '\\' AND f.project_id = ?2
             ORDER BY c.symbol LIMIT ?3"
        } else {
            "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                    c.start_line, c.end_line, c.language, f.path, c.suspicious
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE c.symbol LIKE ?1 ESCAPE '\\'
             ORDER BY c.symbol LIMIT ?2"
        };

        let mut stmt = self
            .conn
            .prepare(query)
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = if let Some(pid) = project_id {
            stmt.query_map(
                params![like_pattern, pid, limit as i64],
                Self::map_chunk_row,
            )
        } else {
            stmt.query_map(params![like_pattern, limit as i64], Self::map_chunk_row)
        }
        .map_err(|e| Error::Sqlite(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
        }
        Ok(results)
    }

    pub fn get_file_chunks(
        &self,
        project_id: i64,
        file_path: &str,
    ) -> Result<Vec<srag_common::types::Chunk>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                    c.start_line, c.end_line, c.language, c.suspicious
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE f.project_id = ?1 AND f.path = ?2
             ORDER BY c.start_line",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = stmt
            .query_map(params![project_id, file_path], |row| {
                let lang_str: String = row.get(7)?;
                let language: srag_common::types::Language =
                    serde_json::from_value(serde_json::Value::String(lang_str))
                        .unwrap_or(srag_common::types::Language::Unknown);
                let suspicious: i32 = row.get::<_, Option<i32>>(8)?.unwrap_or(0);
                Ok(srag_common::types::Chunk {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    content: row.get(2)?,
                    symbol: row.get(3)?,
                    symbol_kind: row.get(4)?,
                    start_line: row.get(5)?,
                    end_line: row.get(6)?,
                    language,
                    suspicious: suspicious != 0,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let mut chunks = Vec::new();
        for row in rows {
            chunks.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
        }
        Ok(chunks)
    }

    pub fn list_project_files(&self, project_id: i64) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, path, blake3_hash, language, size_bytes, chunk_count, indexed_at
             FROM files WHERE project_id = ?1 ORDER BY path"
        ).map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = stmt
            .query_map(params![project_id], |row| {
                let lang_str: String = row.get(4)?;
                let language: srag_common::types::Language =
                    serde_json::from_value(serde_json::Value::String(lang_str))
                        .unwrap_or(srag_common::types::Language::Unknown);
                Ok(FileRecord {
                    id: Some(row.get(0)?),
                    project_id: row.get(1)?,
                    path: row.get(2)?,
                    blake3_hash: row.get(3)?,
                    language,
                    size_bytes: row.get(5)?,
                    chunk_count: row.get(6)?,
                    indexed_at: row.get(7)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
        }
        Ok(files)
    }

    pub fn get_project_patterns(&self, project_id: i64) -> Result<ProjectPatterns> {
        let mut lang_stmt = self.conn.prepare(
            "SELECT language, COUNT(*) as cnt FROM files WHERE project_id = ?1 GROUP BY language ORDER BY cnt DESC"
        ).map_err(|e| Error::Sqlite(e.to_string()))?;
        let languages: Vec<(String, u64)> = lang_stmt
            .query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        let mut kind_stmt = self
            .conn
            .prepare(
                "SELECT c.symbol_kind, COUNT(*) as cnt FROM chunks c
             JOIN files f ON c.file_id = f.id
             WHERE f.project_id = ?1 AND c.symbol_kind IS NOT NULL
             GROUP BY c.symbol_kind ORDER BY cnt DESC LIMIT 20",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let symbol_kinds: Vec<(String, u64)> = kind_stmt
            .query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        let mut prefix_stmt = self.conn.prepare(
            "SELECT SUBSTR(c.symbol, 1, INSTR(c.symbol || '_', '_') - 1) as prefix, COUNT(*) as cnt
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE f.project_id = ?1 AND c.symbol IS NOT NULL AND LENGTH(c.symbol) > 0
             GROUP BY prefix HAVING cnt > 2 ORDER BY cnt DESC LIMIT 30"
        ).map_err(|e| Error::Sqlite(e.to_string()))?;
        let common_prefixes: Vec<(String, u64)> = prefix_stmt
            .query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter(|(p, _)| !p.is_empty())
            .collect();

        let mut dir_stmt = self
            .conn
            .prepare(
                "SELECT SUBSTR(path, 1, INSTR(path || '/', '/') - 1) as dir, COUNT(*) as cnt
             FROM files WHERE project_id = ?1
             GROUP BY dir ORDER BY cnt DESC LIMIT 20",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let directories: Vec<(String, u64)> = dir_stmt
            .query_map(params![project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .filter_map(|r| r.ok())
            .filter(|(d, _)| !d.is_empty())
            .collect();

        Ok(ProjectPatterns {
            languages,
            symbol_kinds,
            common_prefixes,
            directories,
        })
    }

    pub fn list_indexed_languages(&self) -> Result<Vec<LanguageStats>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.language, COUNT(DISTINCT f.id) as file_cnt, COUNT(c.id) as chunk_cnt
             FROM files f
             LEFT JOIN chunks c ON c.file_id = f.id
             GROUP BY f.language
             ORDER BY chunk_cnt DESC",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(LanguageStats {
                    language: row.get(0)?,
                    file_count: row.get::<_, i64>(1)? as u64,
                    chunk_count: row.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let mut languages = Vec::new();
        for row in rows {
            let stats = row.map_err(|e| Error::Sqlite(e.to_string()))?;
            if stats.language != "unknown" {
                languages.push(stats);
            }
        }
        Ok(languages)
    }

    fn map_chunk_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<(srag_common::types::Chunk, String)> {
        let lang_str: String = row.get(7)?;
        let language: srag_common::types::Language =
            serde_json::from_value(serde_json::Value::String(lang_str))
                .unwrap_or(srag_common::types::Language::Unknown);
        let suspicious: i32 = row.get::<_, Option<i32>>(9)?.unwrap_or(0);
        Ok((
            srag_common::types::Chunk {
                id: Some(row.get(0)?),
                file_id: row.get(1)?,
                content: row.get(2)?,
                symbol: row.get(3)?,
                symbol_kind: row.get(4)?,
                start_line: row.get(5)?,
                end_line: row.get(6)?,
                language,
                suspicious: suspicious != 0,
            },
            row.get::<_, String>(8)?,
        ))
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
mod tests {
    use super::*;
    use srag_common::types::Language;
    use tempfile::tempdir;

    fn test_store() -> Store {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        Store::open(&db_path).unwrap()
    }

    #[test]
    fn test_store_open_and_init() {
        let _store = test_store();
    }

    #[test]
    fn test_project_crud() {
        let store = test_store();
        let id = store.upsert_project("test-proj", "/tmp/test").unwrap();
        assert!(id > 0);

        let fetched_id = store.get_project_id("test-proj").unwrap();
        assert_eq!(id, fetched_id);

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-proj");
    }

    #[test]
    fn test_project_upsert_updates_path() {
        let store = test_store();
        store.upsert_project("proj", "/old/path").unwrap();
        store.upsert_project("proj", "/new/path").unwrap();

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, "/new/path");
    }

    #[test]
    fn test_project_delete_cascades() {
        let store = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();
        let record = FileRecord {
            id: None,
            project_id: pid,
            path: "test.rs".to_string(),
            blake3_hash: "abc".to_string(),
            language: Language::Rust,
            size_bytes: 100,
            chunk_count: 1,
            indexed_at: "2024-01-01".to_string(),
        };
        store.upsert_file(&record).unwrap();
        assert_eq!(store.file_count(Some(pid)).unwrap(), 1);

        store.delete_project(pid).unwrap();
        assert_eq!(store.file_count(Some(pid)).unwrap(), 0);
    }

    #[test]
    fn test_file_operations() {
        let store = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();

        let record = FileRecord {
            id: None,
            project_id: pid,
            path: "src/main.rs".to_string(),
            blake3_hash: "hash123".to_string(),
            language: Language::Rust,
            size_bytes: 1024,
            chunk_count: 5,
            indexed_at: "2024-01-01".to_string(),
        };
        let fid = store.upsert_file(&record).unwrap();
        assert!(fid > 0);

        let hash = store.get_file_hash(pid, "src/main.rs").unwrap();
        assert_eq!(hash, Some("hash123".to_string()));

        let files = store.list_project_files(pid).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].language, Language::Rust);
    }

    #[test]
    fn test_file_hash_not_found() {
        let store = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();
        let hash = store.get_file_hash(pid, "nonexistent.rs").unwrap();
        assert!(hash.is_none());
    }

    #[test]
    fn test_transaction_commit() {
        let store = test_store();
        store.begin_transaction().unwrap();
        store.upsert_project("proj", "/tmp").unwrap();
        store.commit().unwrap();

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let store = test_store();
        store.upsert_project("existing", "/tmp").unwrap();

        store.begin_transaction().unwrap();
        store.upsert_project("new", "/tmp2").unwrap();
        store.rollback().unwrap();

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "existing");
    }

    #[test]
    fn test_find_project_by_path_exact() {
        let store = test_store();
        store
            .upsert_project("myproj", "/home/user/project")
            .unwrap();

        let found = store.find_project_by_path("/home/user/project").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "myproj");
    }

    #[test]
    fn test_find_project_by_path_prefix() {
        let store = test_store();
        store
            .upsert_project("myproj", "/home/user/project")
            .unwrap();

        let found = store
            .find_project_by_path("/home/user/project/src")
            .unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_find_project_by_path_not_found() {
        let store = test_store();
        store
            .upsert_project("myproj", "/home/user/project")
            .unwrap();

        let found = store.find_project_by_path("/other/path").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_escape_like_pattern_special_chars() {
        assert_eq!(escape_like_pattern("test%pattern"), "test\\%pattern");
        assert_eq!(escape_like_pattern("test_pattern"), "test\\_pattern");
        assert_eq!(escape_like_pattern("test\\pattern"), "test\\\\pattern");
        assert_eq!(escape_like_pattern("normal"), "normal");
    }

    #[test]
    fn test_sql_injection_in_search_pattern() {
        let store = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();

        let results = store.search_symbols("'; DROP TABLE chunks; --", Some(pid), 10);
        assert!(results.is_ok());

        let results = store.search_symbols("%", Some(pid), 10);
        assert!(results.is_ok());

        let results = store.search_symbols("_", Some(pid), 10);
        assert!(results.is_ok());
    }

    #[test]
    fn test_statistics() {
        let store = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();

        assert_eq!(store.file_count(Some(pid)).unwrap(), 0);
        assert_eq!(store.chunk_count(Some(pid)).unwrap(), 0);
        assert_eq!(store.total_size_bytes(Some(pid)).unwrap(), 0);

        let record = FileRecord {
            id: None,
            project_id: pid,
            path: "test.rs".to_string(),
            blake3_hash: "abc".to_string(),
            language: Language::Rust,
            size_bytes: 500,
            chunk_count: 3,
            indexed_at: "2024-01-01".to_string(),
        };
        store.upsert_file(&record).unwrap();

        assert_eq!(store.file_count(Some(pid)).unwrap(), 1);
        assert_eq!(store.total_size_bytes(Some(pid)).unwrap(), 500);
    }

    #[test]
    fn test_update_project_indexed_at() {
        let store = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();
        store.update_project_indexed_at(pid).unwrap();

        let projects = store.list_projects().unwrap();
        assert!(projects[0].last_indexed_at.is_some());
    }
}
