// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rusqlite::params;
use srag_common::types::Project;
use srag_common::{Error, Result};

use super::{escape_like_pattern, Store};

impl Store {
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

    pub fn find_project_by_path(&self, dir_path: &str) -> Result<Option<Project>> {
        use rusqlite::OptionalExtension;
        let escaped = escape_like_pattern(dir_path);
        self.conn
            .query_row(
                "SELECT id, name, path, created_at, last_indexed_at FROM projects
                 WHERE path = ?1 OR ?2 LIKE path || '/%' ESCAPE '\\'
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
}

#[cfg(test)]
mod tests {
    use crate::index::store::tests::test_store;
    use srag_common::types::Language;

    #[test]
    fn test_project_crud() {
        let (store, _dir) = test_store();
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
        let (store, _dir) = test_store();
        store.upsert_project("proj", "/old/path").unwrap();
        store.upsert_project("proj", "/new/path").unwrap();

        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, "/new/path");
    }

    #[test]
    fn test_project_delete_cascades() {
        let (store, _dir) = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();
        let record = srag_common::types::FileRecord {
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
    fn test_find_project_by_path_exact() {
        let (store, _dir) = test_store();
        store
            .upsert_project("myproj", "/home/user/project")
            .unwrap();

        let found = store.find_project_by_path("/home/user/project").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "myproj");
    }

    #[test]
    fn test_find_project_by_path_prefix() {
        let (store, _dir) = test_store();
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
        let (store, _dir) = test_store();
        store
            .upsert_project("myproj", "/home/user/project")
            .unwrap();

        let found = store.find_project_by_path("/other/path").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn test_update_project_indexed_at() {
        let (store, _dir) = test_store();
        let pid = store.upsert_project("proj", "/tmp").unwrap();
        store.update_project_indexed_at(pid).unwrap();

        let projects = store.list_projects().unwrap();
        assert!(projects[0].last_indexed_at.is_some());
    }
}
