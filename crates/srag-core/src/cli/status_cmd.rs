// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;

use crate::config::Config;
use crate::index::store::Store;

pub async fn run(detailed: bool) -> Result<()> {
    let config = Config::load()?;
    let db_path = config.db_path();

    if !db_path.exists() {
        println!("no index found. run 'srag index <path>' first.");
        return Ok(());
    }

    let store = Store::open(&db_path)?;
    let projects = store.list_projects()?;
    let total_files = store.file_count(None)?;
    let total_chunks = store.chunk_count(None)?;
    let total_embedded = store.embedded_chunk_count(None)?;
    let total_bytes = store.total_size_bytes(None)?;

    println!("srag status");
    println!("  projects: {}", projects.len());
    println!("  files:    {}", total_files);
    println!("  chunks:   {} ({} embedded)", total_chunks, total_embedded);
    println!("  size:     {}", format_bytes(total_bytes));
    println!("  db:       {}", db_path.display());

    if detailed {
        println!();
        for p in &projects {
            let pid = p.id.unwrap();
            let files = store.file_count(Some(pid))?;
            let chunks = store.chunk_count(Some(pid))?;
            let embedded = store.embedded_chunk_count(Some(pid))?;
            let size = store.total_size_bytes(Some(pid))?;
            println!("  [{}]", p.name);
            println!("    path:         {}", p.path);
            println!("    files:        {}", files);
            println!("    chunks:       {} ({} embedded)", chunks, embedded);
            println!("    size:         {}", format_bytes(size));
            println!(
                "    last indexed: {}",
                p.last_indexed_at.as_deref().unwrap_or("never")
            );
        }
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
