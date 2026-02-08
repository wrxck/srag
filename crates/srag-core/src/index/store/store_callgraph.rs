// SPDX-License-Identifier: GPL-3.0

use rusqlite::params;
use srag_common::types::{CallGraphEntry, Definition, FunctionCall};
use srag_common::{Error, Result};

use super::Store;

impl Store {
    pub fn insert_definition(&self, def: &Definition) -> Result<i64> {
        let lang_str = def.language.as_str();

        self.conn
            .execute(
                "INSERT OR REPLACE INTO definitions
                 (chunk_id, file_id, name, kind, scope, language, start_line, end_line, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    def.chunk_id,
                    def.file_id,
                    def.name,
                    def.kind,
                    def.scope,
                    lang_str,
                    def.start_line,
                    def.end_line,
                    def.signature,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_function_call(&self, call: &FunctionCall) -> Result<i64> {
        let lang_str = call.language.as_str();

        self.conn
            .execute(
                "INSERT INTO function_calls
                 (chunk_id, file_id, caller_name, caller_scope, callee_name, line_number, language)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    call.chunk_id,
                    call.file_id,
                    call.caller_name,
                    call.caller_scope,
                    call.callee_name,
                    call.line_number,
                    lang_str,
                ],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_file_call_graph(&self, file_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM definitions WHERE file_id = ?1",
                params![file_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        self.conn
            .execute(
                "DELETE FROM function_calls WHERE file_id = ?1",
                params![file_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        Ok(())
    }

    pub fn resolve_calls_for_project(&self, project_id: i64) -> Result<u64> {
        let updated = self
            .conn
            .execute(
                "UPDATE function_calls
                 SET callee_definition_id = (
                     SELECT d.id FROM definitions d
                     JOIN files f ON d.file_id = f.id
                     WHERE f.project_id = ?1
                     AND d.name = function_calls.callee_name
                     ORDER BY (d.file_id = function_calls.file_id) DESC, d.id ASC
                     LIMIT 1
                 )
                 WHERE file_id IN (SELECT id FROM files WHERE project_id = ?1)
                 AND callee_definition_id IS NULL",
                params![project_id],
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        Ok(updated as u64)
    }

    pub fn find_callers(
        &self,
        project_id: i64,
        function_name: &str,
    ) -> Result<Vec<CallGraphEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT d.name, d.kind, f.path, d.start_line, d.end_line, d.scope
                 FROM function_calls fc
                 JOIN definitions d ON fc.caller_name = d.name AND fc.file_id = d.file_id
                 AND (fc.caller_scope IS NULL OR d.scope IS NULL OR fc.caller_scope = d.scope)
                 JOIN files f ON d.file_id = f.id
                 WHERE f.project_id = ?1 AND fc.callee_name = ?2
                 ORDER BY f.path, d.start_line",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = stmt
            .query_map(params![project_id, function_name], |row| {
                Ok(CallGraphEntry {
                    definition_name: row.get(0)?,
                    definition_kind: row.get(1)?,
                    file_path: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    scope: row.get(5)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn find_callees(
        &self,
        project_id: i64,
        function_name: &str,
    ) -> Result<Vec<CallGraphEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT DISTINCT d.name, d.kind, f.path, d.start_line, d.end_line, d.scope
                 FROM function_calls fc
                 JOIN definitions d ON fc.callee_name = d.name
                 JOIN files f ON d.file_id = f.id
                 WHERE f.project_id = ?1 AND fc.caller_name = ?2
                 AND fc.file_id IN (SELECT id FROM files WHERE project_id = ?1)
                 ORDER BY f.path, d.start_line",
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = stmt
            .query_map(params![project_id, function_name], |row| {
                Ok(CallGraphEntry {
                    definition_name: row.get(0)?,
                    definition_kind: row.get(1)?,
                    file_path: row.get(2)?,
                    start_line: row.get(3)?,
                    end_line: row.get(4)?,
                    scope: row.get(5)?,
                })
            })
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    pub fn get_call_graph_stats(&self, project_id: i64) -> Result<(u64, u64)> {
        let def_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM definitions d
                 JOIN files f ON d.file_id = f.id WHERE f.project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let call_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM function_calls fc
                 JOIN files f ON fc.file_id = f.id WHERE f.project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        Ok((def_count as u64, call_count as u64))
    }

    pub fn list_definitions(
        &self,
        project_id: i64,
        kind_filter: Option<&str>,
    ) -> Result<Vec<CallGraphEntry>> {
        let query = match kind_filter {
            Some(_) => {
                "SELECT d.name, d.kind, f.path, d.start_line, d.end_line, d.scope
                 FROM definitions d JOIN files f ON d.file_id = f.id
                 WHERE f.project_id = ?1 AND d.kind = ?2
                 ORDER BY f.path, d.start_line"
            }
            None => {
                "SELECT d.name, d.kind, f.path, d.start_line, d.end_line, d.scope
                 FROM definitions d JOIN files f ON d.file_id = f.id
                 WHERE f.project_id = ?1
                 ORDER BY f.path, d.start_line"
            }
        };

        let mut stmt = self
            .conn
            .prepare(query)
            .map_err(|e| Error::Sqlite(e.to_string()))?;

        let rows = if let Some(kind) = kind_filter {
            stmt.query_map(params![project_id, kind], Self::map_call_graph_entry)
        } else {
            stmt.query_map(params![project_id], Self::map_call_graph_entry)
        }
        .map_err(|e| Error::Sqlite(e.to_string()))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| Error::Sqlite(e.to_string()))
    }

    fn map_call_graph_entry(row: &rusqlite::Row) -> rusqlite::Result<CallGraphEntry> {
        Ok(CallGraphEntry {
            definition_name: row.get(0)?,
            definition_kind: row.get(1)?,
            file_path: row.get(2)?,
            start_line: row.get(3)?,
            end_line: row.get(4)?,
            scope: row.get(5)?,
        })
    }
}
