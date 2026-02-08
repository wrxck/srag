// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::params;
use srag_common::types::FileRecord;
use srag_common::{Error, Result};

use super::Store;

impl Store {
    pub fn get_file_hash(&self, project_id: i64, path: &str) -> Result<Option<String>> {
        use rusqlite::OptionalExtension;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::store::tests::test_store;
    use srag_common::types::Language;

    #[test]
    fn test_file_operations() {
        let (store, _dir) = test_store();
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
        let (store, _dir) = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();
        let hash = store.get_file_hash(pid, "nonexistent.rs").unwrap();
        assert!(hash.is_none());
    }
}
