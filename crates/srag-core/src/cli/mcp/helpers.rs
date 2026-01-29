// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use crate::index::store::Store;
use rmcp::ErrorData as McpError;

pub fn resolve_project(store: &Store, project: Option<&str>) -> Result<(i64, String), McpError> {
    if let Some(name) = project {
        let id = store
            .get_project_id(name)
            .map_err(|_| McpError::invalid_params(format!("Project '{}' not found", name), None))?;
        return Ok((id, name.to_string()));
    }

    let cwd = std::env::current_dir().map_err(|e| McpError::internal_error(e.to_string(), None))?;
    let cwd_str = cwd.to_string_lossy();

    if let Ok(Some(proj)) = store.find_project_by_path(&cwd_str) {
        return Ok((proj.id.unwrap(), proj.name));
    }

    let projects = store
        .list_projects()
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    if projects.len() == 1 {
        return Ok((projects[0].id.unwrap(), projects[0].name.clone()));
    }

    Err(McpError::invalid_params(
        "could not determine project - specify project name or run from project directory",
        None,
    ))
}

pub fn format_chunk(chunk: &srag_common::types::Chunk, file_path: &str) -> String {
    let header = if let Some(ref symbol) = chunk.symbol {
        format!(
            "--- {} ({}, lines {}-{}) ---",
            file_path, symbol, chunk.start_line, chunk.end_line
        )
    } else {
        format!(
            "--- {} (lines {}-{}) ---",
            file_path, chunk.start_line, chunk.end_line
        )
    };
    format!("{}\n{}\n", header, chunk.content)
}
