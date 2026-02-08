// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::params;
use srag_common::{Error, Result};

use super::{escape_like_pattern, LanguageStats, ProjectPatterns, Store};

impl Store {
    pub fn search_symbols(
        &self,
        pattern: &str,
        project_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<(srag_common::types::Chunk, String)>> {
        self.search_symbols_paginated(pattern, project_id, limit, 0)
    }

    pub fn search_symbols_paginated(
        &self,
        pattern: &str,
        project_id: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(srag_common::types::Chunk, String)>> {
        let escaped = escape_like_pattern(pattern);
        let like_pattern = format!("%{}%", escaped);
        let query = if project_id.is_some() {
            "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                    c.start_line, c.end_line, c.language, f.path, c.suspicious
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE c.symbol LIKE ?1 ESCAPE '\\' AND f.project_id = ?2
             ORDER BY c.symbol LIMIT ?3 OFFSET ?4"
        } else {
            "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                    c.start_line, c.end_line, c.language, f.path, c.suspicious
             FROM chunks c JOIN files f ON c.file_id = f.id
             WHERE c.symbol LIKE ?1 ESCAPE '\\'
             ORDER BY c.symbol LIMIT ?2 OFFSET ?3"
        };

        let mut stmt = self
            .conn
            .prepare(query)
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = if let Some(pid) = project_id {
            stmt.query_map(
                params![like_pattern, pid, limit as i64, offset as i64],
                Self::map_chunk_row,
            )
        } else {
            stmt.query_map(
                params![like_pattern, limit as i64, offset as i64],
                Self::map_chunk_row,
            )
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

    pub(crate) fn map_chunk_row(
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

#[cfg(test)]
mod tests {
    use crate::index::store::tests::test_store;

    #[test]
    fn test_sql_injection_in_search_pattern() {
        let (store, _dir) = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();

        let results = store.search_symbols("'; DROP TABLE chunks; --", Some(pid), 10);
        assert!(results.is_ok());

        let results = store.search_symbols("%", Some(pid), 10);
        assert!(results.is_ok());

        let results = store.search_symbols("_", Some(pid), 10);
        assert!(results.is_ok());
    }
}
