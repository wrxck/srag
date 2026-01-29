// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::{params, OptionalExtension};
use srag_common::types::ConversationTurn;
use srag_common::{Error, Result};

use super::Store;

impl Store {
    pub fn create_session(&self, session_id: &str, project_name: Option<&str>) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO sessions (id, project_name) VALUES (?1, ?2)",
                params![session_id, project_name],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn add_turn(&self, turn: &ConversationTurn) -> Result<i64> {
        self.conn
            .execute(
                "INSERT INTO turns (session_id, role, content, sources)
                 VALUES (?1, ?2, ?3, ?4)",
                params![turn.session_id, turn.role, turn.content, turn.sources],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_recent_turns(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationTurn>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, session_id, role, content, sources, created_at
                 FROM turns WHERE session_id = ?1
                 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![session_id, limit], |row| {
                Ok(ConversationTurn {
                    id: Some(row.get(0)?),
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    sources: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        let mut turns = Vec::new();
        for row in rows {
            turns.push(row.map_err(|e| Error::Sqlite(e.to_string()))?);
        }
        turns.reverse();
        Ok(turns)
    }

    pub fn enqueue_reindex(
        &self,
        project_id: i64,
        file_path: &str,
        event_type: &str,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO reindex_queue (project_id, file_path, event_type)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(project_id, file_path) DO UPDATE SET
                    event_type = ?3, queued_at = datetime('now')",
                params![project_id, file_path, event_type],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(())
    }

    pub fn dequeue_reindex(&self, project_id: i64) -> Result<Option<(i64, String, String)>> {
        let result = self
            .conn
            .query_row(
                "SELECT id, file_path, event_type FROM reindex_queue
                 WHERE project_id = ?1 ORDER BY queued_at ASC LIMIT 1",
                params![project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        if let Some((id, path, event)) = result {
            self.conn
                .execute("DELETE FROM reindex_queue WHERE id = ?1", params![id])
                .map_err(|e| Error::Sqlite(e.to_string()))?;
            Ok(Some((id, path, event)))
        } else {
            Ok(None)
        }
    }

    pub fn reindex_queue_len(&self, project_id: i64) -> Result<u64> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM reindex_queue WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;
        Ok(count as u64)
    }
}
