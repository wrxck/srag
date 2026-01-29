// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::{params, OptionalExtension};
use srag_common::types::{Chunk, Language};
use srag_common::{Error, Result};

use super::Store;

fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(vector.len() * 4);
    for &v in vector {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    buf
}

fn decode_vector(bytes: &[u8], dim: usize) -> Result<Vec<f32>> {
    let expected = dim * 4;
    if bytes.len() < expected {
        return Err(Error::Sqlite(format!(
            "Embedding blob too short: expected {} bytes, got {}",
            expected,
            bytes.len()
        )));
    }
    let mut vec = Vec::with_capacity(dim);
    for i in 0..dim {
        let offset = i * 4;
        let b = [
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ];
        vec.push(f32::from_le_bytes(b));
    }
    Ok(vec)
}

/// escape a user query for safe use in fts5 MATCH.
/// wraps each whitespace-delimited term in double quotes to disable
/// fts5 operators (AND, OR, NOT, NEAR, column filters).
fn escape_fts5_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Store {
    pub fn insert_embedding(&self, chunk_id: i64, vector: &[f32]) -> Result<i64> {
        let blob = encode_vector(vector);
        self.conn
            .execute(
                "INSERT INTO embeddings (chunk_id, vector) VALUES (?1, ?2)
                 ON CONFLICT(chunk_id) DO UPDATE SET vector = ?2",
                params![chunk_id, blob],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM embeddings WHERE chunk_id = ?1",
                params![chunk_id],
                |row| row.get(0),
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(id)
    }

    pub fn update_chunk_embedding_id(&self, chunk_id: i64, embedding_id: i64) -> Result<()> {
        self.conn
            .execute(
                "UPDATE chunks SET embedding_id = ?1 WHERE id = ?2",
                params![embedding_id, chunk_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    /// stream embeddings from db one row at a time, calling `f` for each.
    /// avoids materialising all embeddings in memory at once.
    pub fn for_each_embedding(
        &self,
        dim: usize,
        mut f: impl FnMut(i64, Vec<f32>) -> Result<()>,
    ) -> Result<u64> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, vector FROM embeddings")
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((id, blob))
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let mut count = 0u64;
        for row in rows {
            let (id, blob) = row.map_err(|e| Error::Sqlite(e.to_string()))?;
            let vector = decode_vector(&blob, dim)?;
            f(id, vector)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn embedding_count(&self) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(count as u64)
    }

    pub fn get_chunk_by_id(&self, chunk_id: i64) -> Result<Option<(Chunk, String)>> {
        self.conn
            .query_row(
                "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                        c.start_line, c.end_line, c.language, f.path, c.suspicious
                 FROM chunks c JOIN files f ON c.file_id = f.id
                 WHERE c.id = ?1",
                params![chunk_id],
                |row| {
                    let lang_str: String = row.get(7)?;
                    let language: Language =
                        serde_json::from_value(serde_json::Value::String(lang_str))
                            .unwrap_or(Language::Unknown);
                    let suspicious: i32 = row.get::<_, Option<i32>>(9)?.unwrap_or(0);
                    Ok((
                        Chunk {
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
                },
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn get_chunk_id_by_embedding_id(&self, embedding_id: i64) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT chunk_id FROM embeddings WHERE id = ?1",
                params![embedding_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn embedded_chunk_count(&self, project_id: Option<i64>) -> Result<u64> {
        let count: i64 = if let Some(pid) = project_id {
            self.conn
                .query_row(
                    "SELECT COUNT(*) FROM embeddings e
                     JOIN chunks c ON e.chunk_id = c.id
                     JOIN files f ON c.file_id = f.id
                     WHERE f.project_id = ?1",
                    params![pid],
                    |row| row.get(0),
                )
                .map_err(|e| Error::Sqlite(e.to_string()))?
        } else {
            self.conn
                .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
                .map_err(|e| Error::Sqlite(e.to_string()))?
        };
        Ok(count as u64)
    }

    pub fn delete_file_embeddings(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM embeddings WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)",
                params![file_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    // -- FTS operations --

    pub fn insert_chunk_fts(
        &self,
        chunk_id: i64,
        content: &str,
        file_path: &str,
        symbol: Option<&str>,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO chunks_fts (chunk_id, content, file_path, symbol) VALUES (?1, ?2, ?3, ?4)",
                params![chunk_id, content, file_path, symbol.unwrap_or("")],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn delete_file_chunks_fts(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM chunks_fts WHERE chunk_id IN (SELECT id FROM chunks WHERE file_id = ?1)",
                params![file_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    /// delete all fts entries for chunks belonging to a project.
    /// must be called before delete_project_files to avoid orphaned fts rows.
    pub fn delete_project_chunks_fts(&self, project_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM chunks_fts WHERE chunk_id IN (
                     SELECT c.id FROM chunks c
                     JOIN files f ON c.file_id = f.id
                     WHERE f.project_id = ?1
                 )",
                params![project_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<(i64, f64)>> {
        self.search_fts_project(query, None, limit)
    }

    pub fn search_fts_project(
        &self,
        query: &str,
        project_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<(i64, f64)>> {
        let escaped = escape_fts5_query(query);
        if escaped.is_empty() {
            return Ok(Vec::new());
        }

        let (sql, use_project) = if project_id.is_some() {
            (
                "SELECT fts.chunk_id, fts.rank FROM chunks_fts fts
                 JOIN chunks c ON fts.chunk_id = c.id
                 JOIN files f ON c.file_id = f.id
                 WHERE fts.content MATCH ?1 AND f.project_id = ?2
                 ORDER BY fts.rank LIMIT ?3",
                true,
            )
        } else {
            (
                "SELECT chunk_id, rank FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY rank LIMIT ?2",
                false
            )
        };

        let mut stmt = self
            .conn
            .prepare(sql)
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let mut results: Vec<(i64, f64)> = Vec::new();
        let mapper = |row: &rusqlite::Row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?));

        if use_project {
            let rows = stmt
                .query_map(params![escaped, project_id.unwrap(), limit as i64], mapper)
                .map_err(|e| Error::Sqlite(e.to_string()))?;
            for row in rows {
                results.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
            }
        } else {
            let rows = stmt
                .query_map(params![escaped, limit as i64], mapper)
                .map_err(|e| Error::Sqlite(e.to_string()))?;
            for row in rows {
                results.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
            }
        }
        Ok(results)
    }
}
