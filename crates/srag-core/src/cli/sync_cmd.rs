// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;

use crate::config::Config;
use crate::index::store::Store;

pub async fn run() -> Result<()> {
    let config = Config::load()?;
    let db_path = config.db_path();

    if !db_path.exists() {
        println!("no index found. run 'srag index <path>' first.");
        return Ok(());
    }

    let store = Store::open(&db_path)?;
    let projects = store.list_projects()?;
    drop(store);

    if projects.is_empty() {
        println!("no projects registered. run 'srag index <path>' first.");
        return Ok(());
    }

    println!("syncing {} project(s)...\n", projects.len());

    let mut errors = Vec::new();

    for project in &projects {
        let path = std::path::Path::new(&project.path);
        if !path.is_dir() {
            println!(
                "skipping {}: directory {} no longer exists\n",
                project.name, project.path
            );
            errors.push(project.name.clone());
            continue;
        }

        println!("--- {} ---", project.name);
        if let Err(e) =
            super::index_cmd::run_opts(&project.path, Some(&project.name), false, false, false)
                .await
        {
            println!("error syncing {}: {}\n", project.name, e);
            errors.push(project.name.clone());
        } else {
            println!();
        }
    }

    if errors.is_empty() {
        println!("all projects synced.");
    } else {
        println!("synced with errors in: {}", errors.join(", "));
    }

    Ok(())
}
