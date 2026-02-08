// SPDX-Licence-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use crate::config::Config;
use crate::index::store::Store;
use rmcp::ErrorData as McpError;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

const PROJECT_MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Gemfile",
    "composer.json",
    "*.csproj",
    "*.sln",
    "mix.exs",
    ".git",
];

pub fn is_project_directory(path: &Path) -> bool {
    for marker in PROJECT_MARKERS {
        if marker.contains('*') {
            if let Ok(entries) = std::fs::read_dir(path) {
                let pattern = marker.trim_start_matches('*');
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().ends_with(pattern) {
                        return true;
                    }
                }
            }
        } else if path.join(marker).exists() {
            return true;
        }
    }
    false
}

pub struct AutoIndexResult {
    pub project_name: String,
    pub summary: String,
}

pub async fn auto_index_cwd(_config: &Config) -> Result<AutoIndexResult, McpError> {
    let cwd = std::env::current_dir().map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let cwd = cwd
        .canonicalize()
        .map_err(|e| McpError::internal_error(format!("Failed to resolve path: {}", e), None))?;

    if !cwd.is_dir() {
        return Err(McpError::internal_error(
            "Current directory is not accessible",
            None,
        ));
    }

    if !is_project_directory(&cwd) {
        return Err(McpError::internal_error(
            "Current directory does not appear to be a project (no Cargo.toml, package.json, etc.)",
            None,
        ));
    }

    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed");

    if project_name.is_empty() || project_name.contains('\0') {
        return Err(McpError::internal_error(
            "Invalid project name derived from directory",
            None,
        ));
    }

    let project_name = project_name.to_string();

    let exe = std::env::current_exe().map_err(|e| {
        McpError::internal_error(format!("Failed to get executable path: {}", e), None)
    })?;

    let output = Command::new(&exe)
        .args(["index", &cwd.to_string_lossy(), "--name", &project_name])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            McpError::internal_error(format!("Failed to spawn indexing process: {}", e), None)
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(McpError::internal_error(
            format!("Auto-indexing failed: {}", stderr.trim()),
            None,
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let summary = stderr
        .lines()
        .last()
        .unwrap_or("indexing completed")
        .to_string();

    Ok(AutoIndexResult {
        project_name,
        summary,
    })
}

pub async fn ensure_index_exists(config: &Config) -> Result<Option<AutoIndexResult>, McpError> {
    let db_path = config.db_path();

    if db_path.exists() {
        return Ok(None);
    }

    if !config.mcp.auto_index_cwd {
        return Err(McpError::internal_error(
            "No index found. Run 'srag index /path/to/project' to create one.",
            None,
        ));
    }

    let result = auto_index_cwd(config).await?;

    if !db_path.exists() {
        return Err(McpError::internal_error(
            format!(
                "Auto-indexing of '{}' completed but index not found",
                result.project_name
            ),
            None,
        ));
    }

    Ok(Some(result))
}

pub fn resolve_project(store: &Store, project: Option<&str>) -> Result<(i64, String), McpError> {
    if let Some(name) = project {
        let id = store
            .get_project_id(name)
            .map_err(|_| McpError::invalid_params(format!("Project '{}' not found", name), None))?;
        return Ok((id, name.to_string()));
    }

    let cwd = std::env::current_dir().map_err(|e| McpError::internal_error(e.to_string(), None))?;
    let cwd_str = cwd.to_string_lossy();

    match store.find_project_by_path(&cwd_str) {
        Ok(Some(proj)) => {
            let id = proj
                .id
                .ok_or_else(|| McpError::internal_error("Project found but has no ID", None))?;
            return Ok((id, proj.name));
        }
        Ok(None) => {
            // project not found by path, fall through to check single project case
        }
        Err(e) => {
            tracing::warn!("Failed to find project by path '{}': {}", cwd_str, e);
            // fall through to check single project case
        }
    }

    let projects = store
        .list_projects()
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    if projects.len() == 1 {
        let id = projects[0]
            .id
            .ok_or_else(|| McpError::internal_error("Project found but has no ID", None))?;
        return Ok((id, projects[0].name.clone()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_auto_index_result_struct() {
        let result = AutoIndexResult {
            project_name: "test_project".to_string(),
            summary: "done: 10 files indexed".to_string(),
        };
        assert_eq!(result.project_name, "test_project");
        assert!(result.summary.contains("indexed"));
    }

    #[test]
    fn test_is_project_directory_with_cargo_toml() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_with_package_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_with_git() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_with_csproj() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("MyApp.csproj"), "<Project>").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_empty() {
        let dir = TempDir::new().unwrap();
        assert!(!is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_with_random_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("random.txt"), "content").unwrap();
        std::fs::write(dir.path().join("other.rs"), "fn main() {}").unwrap();
        assert!(!is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_nonexistent() {
        let path = Path::new("/nonexistent/path/that/does/not/exist");
        assert!(!is_project_directory(path));
    }

    #[test]
    fn test_is_project_directory_multiple_markers() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(dir.path().join("package.json"), "{}").unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_pyproject() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "[tool.poetry]").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_go_mod() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("go.mod"), "module example").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_project_markers_no_false_positives() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml.bak"), "[package]").unwrap();
        std::fs::write(dir.path().join("package.json.old"), "{}").unwrap();
        assert!(!is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_symlink_to_marker() {
        let dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();
        std::fs::write(target_dir.path().join("Cargo.toml"), "[package]").unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                target_dir.path().join("Cargo.toml"),
                dir.path().join("Cargo.toml"),
            )
            .unwrap();
            assert!(is_project_directory(dir.path()));
        }
    }

    #[test]
    fn test_is_project_directory_mix_exs() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("mix.exs"), "defmodule MyApp").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_gemfile() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("Gemfile"), "source 'https://rubygems.org'").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_composer_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("composer.json"), "{}").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_build_gradle() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("build.gradle"), "plugins {}").unwrap();
        assert!(is_project_directory(dir.path()));
    }

    #[test]
    fn test_is_project_directory_sln_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("MySolution.sln"),
            "Microsoft Visual Studio Solution",
        )
        .unwrap();
        assert!(is_project_directory(dir.path()));
    }
}
