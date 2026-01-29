// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::path::{Path, PathBuf};

use anyhow::Result;
use dialoguer::{Input, MultiSelect};

use crate::cli::index_cmd;

const PROJECT_MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "Makefile",
    "CMakeLists.txt",
    ".git",
];

const MAX_SCAN_DEPTH: usize = 3;

pub async fn run(all: bool) -> Result<()> {
    let root: String = Input::new()
        .with_prompt("root directory to scan for projects")
        .default(
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        )
        .interact_text()?;

    let root_path = PathBuf::from(&root);
    if !root_path.is_dir() {
        anyhow::bail!("{} is not a directory", root);
    }

    let root_path = std::fs::canonicalize(&root_path)?;
    println!("scanning {}...", root_path.display());

    let mut projects = Vec::new();
    scan_for_projects(&root_path, 0, &mut projects)?;

    if projects.is_empty() {
        println!("no projects found under {}", root_path.display());
        return Ok(());
    }

    let labels: Vec<String> = projects
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    let defaults: Vec<bool> = vec![true; labels.len()];

    let selections = MultiSelect::new()
        .with_prompt("select projects to index")
        .items(&labels)
        .defaults(&defaults)
        .interact()?;

    if selections.is_empty() {
        println!("no projects selected.");
        return Ok(());
    }

    for &idx in &selections {
        let project_path = &projects[idx];
        let name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed");

        println!("\nindexing {} ...", project_path.display());
        index_cmd::run_opts(
            &project_path.to_string_lossy(),
            Some(name),
            false,
            false,
            all,
        )
        .await?;
    }

    println!("\nsetup complete. {} projects indexed.", selections.len());
    Ok(())
}

fn scan_for_projects(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) -> Result<()> {
    if depth > MAX_SCAN_DEPTH {
        return Ok(());
    }

    for marker in PROJECT_MARKERS {
        if dir.join(marker).exists() {
            found.push(dir.to_path_buf());
            return Ok(());
        }
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
            continue;
        }
        scan_for_projects(&path, depth + 1, found)?;
    }

    Ok(())
}
