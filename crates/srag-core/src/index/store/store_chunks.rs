// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::{params, OptionalExtension};
use srag_common::types::{Chunk, Language};
use srag_common::{Error, Result};

use super::Store;

impl Store {
    pub fn delete_file_chunks(&self, file_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT embedding_id FROM chunks WHERE file_id = ?1 AND embedding_id IS NOT NULL",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let ids: Vec<i64> = stmt
            .query_map(params![file_id], |row| row.get(0))
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        self.conn
            .execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(ids)
    }

    pub fn insert_chunk(&self, chunk: &Chunk, embedding_id: Option<i64>) -> Result<i64> {
        let lang = serde_json::to_value(chunk.language)
            .map_err(|e| Error::Sqlite(e.to_string()))?
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        self.conn
            .execute(
                "INSERT INTO chunks (file_id, content, symbol, symbol_kind, start_line, end_line, language, embedding_id, suspicious)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    chunk.file_id,
                    chunk.content,
                    chunk.symbol,
                    chunk.symbol_kind,
                    chunk.start_line,
                    chunk.end_line,
                    lang,
                    embedding_id,
                    chunk.suspicious as i32,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_chunk_by_embedding_id(&self, embedding_id: i64) -> Result<Option<(Chunk, String)>> {
        self.conn
            .query_row(
                "SELECT c.id, c.file_id, c.content, c.symbol, c.symbol_kind,
                        c.start_line, c.end_line, c.language, f.path, c.suspicious
                 FROM chunks c JOIN files f ON c.file_id = f.id
                 WHERE c.embedding_id = ?1",
                params![embedding_id],
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
}
