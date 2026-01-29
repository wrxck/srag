// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;
use std::io::Write;

use crate::config::Config;
use crate::discovery;
use crate::index::hnsw::{rebuild_hnsw_from_db, VectorIndex};
use crate::index::store::Store;
use crate::ipc::client::MlClient;
use crate::ipc::lifecycle;
use crate::resource;
use srag_common::types::Chunk;

const LOAD_SAMPLE_INTERVAL: u64 = 20;
const PROGRESS_WIDTH: usize = 60;

pub async fn run(path: &str, name: Option<&str>, force: bool, dry_run: bool) -> Result<()> {
    run_opts(path, name, force, dry_run, false).await
}

pub async fn run_opts(
    path: &str,
    name: Option<&str>,
    force: bool,
    dry_run: bool,
    all: bool,
) -> Result<()> {
    let abs_path = std::fs::canonicalize(path)?;
    if !abs_path.is_dir() {
        anyhow::bail!("{} is not a directory", abs_path.display());
    }

    let project_name = name.unwrap_or_else(|| {
        abs_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
    });

    let config = Config::load()?;
    config.ensure_dirs()?;

    let files = discovery::walk_directory_opts(&abs_path, &config, all)?;

    if dry_run {
        println!(
            "Dry run: would index {} files from {}",
            files.len(),
            abs_path.display()
        );
        for f in &files {
            println!("  {}", f.display());
        }
        return Ok(());
    }

    let _ = resource::apply_nice_level(config.resource.nice_level);

    lifecycle::ensure_ml_service_running(&config)?;
    let addr = crate::ipc::client::read_service_addr(&Config::port_file_path())?;
    let client = MlClient::connect(addr).await?;

    let store = Store::open(&config.db_path())?;
    let project_id = store.upsert_project(project_name, &abs_path.to_string_lossy())?;

    let mut vector_index =
        VectorIndex::open(&config.vectors_dir(), crate::config::EMBEDDING_DIMENSION)?;
    rebuild_hnsw_from_db(&store, &mut vector_index)?;

    if force {
        // clean up fts rows before cascade-deleting files/chunks/embeddings
        store.delete_project_chunks_fts(project_id)?;
        store.delete_project_files(project_id)?;
    }

    let total_files = files.len();

    let mut indexed = 0u64;
    let mut processed = 0u64;
    let mut skipped = 0u64;
    let mut embedded_count = 0u64;
    let throttle = std::time::Duration::from_millis(config.indexing.throttle_ms);
    let batch_size = config.indexing.batch_size;
    let mut cached_load = 0.0f64;

    let mut pending: Vec<(i64, String)> = Vec::new();

    for file_path in &files {
        let abs_file_path = file_path.to_string_lossy().to_string();

        processed += 1;
        print_progress(project_name, processed, total_files, &abs_file_path);

        let content = match std::fs::read(file_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Skipping {}: {}", abs_file_path, e);
                skipped += 1;
                continue;
            }
        };

        let hash = blake3::hash(&content).to_hex().to_string();

        if !force {
            if let Ok(Some(existing_hash)) = store.get_file_hash(project_id, &abs_file_path) {
                if existing_hash == hash {
                    skipped += 1;
                    continue;
                }
            }
        }

        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let language = srag_common::types::Language::from_extension(ext);
        let language = if language == srag_common::types::Language::Unknown {
            let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            srag_common::types::Language::from_filename(file_name).unwrap_or(language)
        } else {
            language
        };

        let chunks = crate::chunking::chunk_file(&content, language)?;

        let file_record = srag_common::types::FileRecord {
            id: None,
            project_id,
            path: abs_file_path.clone(),
            blake3_hash: hash,
            language,
            size_bytes: content.len() as u64,
            chunk_count: chunks.len() as u32,
            indexed_at: String::new(),
        };

        store.begin_transaction()?;
        let txn_result: anyhow::Result<()> = (|| {
            let file_id = store.upsert_file(&file_record)?;

            store.delete_file_chunks_fts(file_id)?;
            store.delete_file_embeddings(file_id)?;
            store.delete_file_chunks(file_id)?;

            for chunk in &chunks {
                let mut c = chunk.clone();
                c.file_id = file_id;
                c.suspicious = crate::chunking::injection_scanner::is_suspicious(&c.content);
                let chunk_id = store.insert_chunk(&c, None)?;

                store.insert_chunk_fts(
                    chunk_id,
                    &c.content,
                    &abs_file_path,
                    c.symbol.as_deref(),
                )?;

                let enriched = enrich_chunk_text(&abs_file_path, &c);
                pending.push((chunk_id, enriched));
            }
            Ok(())
        })();

        match txn_result {
            Ok(()) => store.commit()?,
            Err(e) => {
                let _ = store.rollback();
                return Err(e);
            }
        }

        if pending.len() >= batch_size {
            let count =
                flush_embedding_batch(&client, &store, &mut vector_index, &mut pending).await?;
            embedded_count += count;
        }

        indexed += 1;

        if !throttle.is_zero() {
            // sample system load periodically instead of every file
            if indexed % LOAD_SAMPLE_INTERVAL == 0 {
                cached_load = resource::get_system_load().unwrap_or(0.0);
            }
            let multiplier = if cached_load > 4.0 {
                3
            } else if cached_load > 2.0 {
                2
            } else {
                1
            };
            std::thread::sleep(throttle * multiplier);
        }
    }

    if !pending.is_empty() {
        let count = flush_embedding_batch(&client, &store, &mut vector_index, &mut pending).await?;
        embedded_count += count;
    }

    vector_index.save(&config.vectors_dir())?;
    store.update_project_indexed_at(project_id)?;
    store.wal_checkpoint()?;

    clear_progress();
    println!(
        "done: {} files indexed, {} chunks embedded, {} skipped (unchanged)",
        indexed, embedded_count, skipped
    );

    Ok(())
}

pub fn enrich_chunk_text(file_path: &str, chunk: &Chunk) -> String {
    let mut enriched = String::new();
    enriched.push_str("File: ");
    enriched.push_str(file_path);
    enriched.push('\n');
    if let Some(ref kind) = chunk.symbol_kind {
        if let Some(ref name) = chunk.symbol {
            enriched.push_str(kind);
            enriched.push_str(": ");
            enriched.push_str(name);
            enriched.push('\n');
        }
    }
    let lang = chunk.language.as_str();
    if lang != "unknown" {
        enriched.push_str("Language: ");
        enriched.push_str(lang);
        enriched.push('\n');
    }
    enriched.push('\n');
    enriched.push_str(&chunk.content);
    enriched
}

const ML_EMBED_LIMIT: usize = 64;

async fn flush_embedding_batch(
    client: &MlClient,
    store: &Store,
    vector_index: &mut VectorIndex,
    pending: &mut Vec<(i64, String)>,
) -> Result<u64> {
    if pending.is_empty() {
        return Ok(0);
    }

    let mut count = 0u64;

    for batch in pending.chunks(ML_EMBED_LIMIT) {
        let texts: Vec<String> = batch.iter().map(|(_, text)| text.clone()).collect();
        let vectors = client.embed(&texts).await?;

        for (i, (chunk_id, _)) in batch.iter().enumerate() {
            if let Some(vector) = vectors.get(i) {
                let embedding_id = store.insert_embedding(*chunk_id, vector)?;
                store.update_chunk_embedding_id(*chunk_id, embedding_id)?;
                vector_index.insert(embedding_id as usize, vector)?;
                count += 1;
            }
        }
    }

    pending.clear();
    Ok(count)
}

fn print_progress(project: &str, current: u64, total: usize, path: &str) {
    let pct = if total > 0 {
        current as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    let bar_width = 20;
    let filled = (pct / 100.0 * bar_width as f64) as usize;
    let bar: String = "=".repeat(filled) + &"-".repeat(bar_width - filled);

    let prefix = format!(
        "\r{} [{}] {:>3.0}% [{}/{}] ",
        project, bar, pct, current, total
    );
    let max_path_len = PROGRESS_WIDTH.saturating_sub(prefix.len());

    let display_path = if path.len() > max_path_len && max_path_len > 3 {
        format!("...{}", &path[path.len() - (max_path_len - 3)..])
    } else {
        path.to_string()
    };

    eprint!("{}{: <width$}", prefix, display_path, width = max_path_len);
    let _ = std::io::stderr().flush();
}

fn clear_progress() {
    eprint!("\r{: <width$}\r", "", width = PROGRESS_WIDTH + 40);
    let _ = std::io::stderr().flush();
}
