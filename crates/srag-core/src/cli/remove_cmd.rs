// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;
use std::io::{self, Write};

use crate::config::Config;
use crate::index::store::Store;

pub async fn run(project: &str, force: bool) -> Result<()> {
    let config = Config::load()?;
    let db_path = config.db_path();

    if !db_path.exists() {
        anyhow::bail!("no index found - nothing to remove");
    }

    let store = Store::open(&db_path)?;

    let project_id = match store.get_project_id(project) {
        Ok(id) => id,
        Err(_) => {
            let projects = store.list_projects()?;
            if projects.is_empty() {
                anyhow::bail!("no projects indexed");
            }
            eprintln!("project '{}' not found. available projects:", project);
            for p in &projects {
                eprintln!("  - {}", p.name);
            }
            anyhow::bail!("project not found");
        }
    };

    let file_count = store.file_count(Some(project_id))?;
    let chunk_count = store.chunk_count(Some(project_id))?;

    if !force {
        eprintln!("this will remove project '{}' from the index:", project);
        eprintln!("  {} files, {} chunks", file_count, chunk_count);
        eprintln!();
        eprint!("are you sure? [y/N] ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("cancelled");
            return Ok(());
        }
    }

    store.delete_project(project_id)?;
    store.wal_checkpoint()?;

    println!(
        "removed project '{}' ({} files, {} chunks)",
        project, file_count, chunk_count
    );
    Ok(())
}
