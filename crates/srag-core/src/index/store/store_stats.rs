// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::params;
use srag_common::{Error, Result};

use super::Store;

impl Store {
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
}

#[cfg(test)]
mod tests {
    use crate::index::store::tests::test_store;
    use srag_common::types::{FileRecord, Language};

    #[test]
    fn test_statistics() {
        let (store, _dir) = test_store();
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
}
