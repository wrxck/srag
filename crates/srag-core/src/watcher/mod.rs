// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::path::PathBuf;

use anyhow::Result;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tokio::sync::mpsc;

use crate::cli::index_cmd::enrich_chunk_text;
use crate::config::Config;
use crate::index::hnsw::{rebuild_hnsw_from_db, VectorIndex};
use crate::index::store::Store;
use crate::ipc::client::MlClient;
use crate::ipc::lifecycle;

pub fn stop_watcher() -> Result<()> {
    let pid_path = Config::watcher_pid_path();
    if !pid_path.exists() {
        println!("No watcher running");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str.trim().parse()?;

    #[cfg(unix)]
    {
        // validate the pid belongs to an srag process before sending signal
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        if let Ok(cmdline) = std::fs::read_to_string(&cmdline_path) {
            if !cmdline.contains("srag") {
                std::fs::remove_file(&pid_path)?;
                anyhow::bail!("Stale pid file (pid {} is not an srag process)", pid);
            }
        }

        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid), Signal::SIGTERM)?;
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
    }

    std::fs::remove_file(&pid_path)?;
    println!("Watcher stopped (pid {})", pid);
    Ok(())
}

pub async fn run_foreground() -> Result<()> {
    let config = Config::load()?;
    config.ensure_dirs()?;

    let db_path = config.db_path();
    if !db_path.exists() {
        anyhow::bail!("No index found. Run 'srag index <path>' first.");
    }

    let store = Store::open(&db_path)?;
    let projects = store.list_projects()?;
    if projects.is_empty() {
        anyhow::bail!("No projects indexed.");
    }

    // start ML service and connect for embedding
    lifecycle::ensure_ml_service_running(&config)?;
    let addr = crate::ipc::client::read_service_addr(&Config::port_file_path())?;
    let client = MlClient::connect(addr).await?;

    // open HNSW index and rebuild from DB
    let mut vector_index =
        VectorIndex::open(&config.vectors_dir(), crate::config::EMBEDDING_DIMENSION)?;
    rebuild_hnsw_from_db(&store, &mut vector_index)?;

    let debounce_duration = std::time::Duration::from_millis(config.watcher.debounce_ms);
    let (tx, mut rx) = mpsc::channel::<Vec<PathBuf>>(256);

    let mut debouncer = new_debouncer(
        debounce_duration,
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                let paths: Vec<PathBuf> = events.into_iter().flat_map(|e| e.event.paths).collect();
                if !paths.is_empty() && tx.try_send(paths).is_err() {
                    tracing::warn!("Watcher event queue full, dropping events");
                }
            }
        },
    )?;

    for project in &projects {
        let path = PathBuf::from(&project.path);
        if path.exists() {
            println!("Watching: {} ({})", project.name, project.path);
            debouncer.watch(&path, notify::RecursiveMode::Recursive)?;
        }
    }

    // write PID file
    let pid_path = Config::watcher_pid_path();
    std::fs::write(&pid_path, std::process::id().to_string())?;

    println!(
        "Watcher running (pid {}). Press CTRL-C to stop.",
        std::process::id()
    );

    // handle signals for graceful shutdown
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            Some(paths) = rx.recv() => {
                handle_changed_paths(&store, &projects, &paths, &config, &client, &mut vector_index).await?;
            }
            _ = &mut shutdown => {
                println!("\nShutting down watcher...");
                break;
            }
        }
    }

    // save HNSW on shutdown
    vector_index.save(&config.vectors_dir())?;

    let _ = std::fs::remove_file(&pid_path);
    Ok(())
}

pub fn run_daemon() -> Result<()> {
    let exe = std::env::current_exe()?;
    let mut child = std::process::Command::new(exe)
        .arg("watch")
        .arg("--foreground")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let pid = child.id();

    // brief wait to catch immediate startup failures
    std::thread::sleep(std::time::Duration::from_millis(500));
    match child.try_wait()? {
        Some(status) => {
            anyhow::bail!("Watcher daemon exited immediately with {}", status);
        }
        None => {
            println!("Watcher daemon started (pid {})", pid);
            Ok(())
        }
    }
}

async fn handle_changed_paths(
    store: &Store,
    projects: &[srag_common::types::Project],
    paths: &[PathBuf],
    config: &Config,
    client: &MlClient,
    vector_index: &mut VectorIndex,
) -> Result<()> {
    for path in paths {
        for project in projects {
            let project_dir = PathBuf::from(&project.path);
            if path.starts_with(&project_dir) {
                let abs_file_path = path
                    .strip_prefix(&project_dir)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                let pid = project.id.unwrap();

                let event_type = if path.exists() { "modify" } else { "delete" };
                store.enqueue_reindex(pid, &abs_file_path, event_type)?;

                tracing::info!(
                    "Queued reindex: {} ({}: {})",
                    abs_file_path,
                    event_type,
                    project.name
                );

                if let Some((_id, queued_path, evt)) = store.dequeue_reindex(pid)? {
                    if evt == "delete" {
                        tracing::info!("File deleted: {}", queued_path);
                    } else {
                        let full_path = project_dir.join(&queued_path);
                        if full_path.exists() {
                            if let Err(e) = reindex_file(
                                store,
                                pid,
                                &project_dir,
                                &full_path,
                                config,
                                client,
                                vector_index,
                            )
                            .await
                            {
                                tracing::warn!("Reindex failed for {}: {}", queued_path, e);
                                // re-enqueue so it can be retried
                                let _ = store.enqueue_reindex(pid, &queued_path, &evt);
                            }
                        }
                    }
                }

                break;
            }
        }
    }
    Ok(())
}

async fn reindex_file(
    store: &Store,
    project_id: i64,
    _project_dir: &PathBuf,
    file_path: &PathBuf,
    config: &Config,
    client: &MlClient,
    vector_index: &mut VectorIndex,
) -> Result<()> {
    let content = std::fs::read(file_path)?;
    let abs_file_path = file_path.to_string_lossy().to_string();

    if content.len() as u64 > config.indexing.max_file_size_bytes {
        return Ok(());
    }

    let hash = blake3::hash(&content).to_hex().to_string();

    // skip if unchanged
    if let Ok(Some(existing_hash)) = store.get_file_hash(project_id, &abs_file_path) {
        if existing_hash == hash {
            return Ok(());
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

    let mut pending_texts: Vec<(i64, String)> = Vec::new();

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

            store.insert_chunk_fts(chunk_id, &c.content, &abs_file_path, c.symbol.as_deref())?;

            let enriched = enrich_chunk_text(&abs_file_path, &c);
            pending_texts.push((chunk_id, enriched));
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

    // embed all chunks for this file
    if !pending_texts.is_empty() {
        let texts: Vec<String> = pending_texts.iter().map(|(_, t)| t.clone()).collect();
        let vectors = client.embed(&texts).await?;

        for (i, (chunk_id, _)) in pending_texts.iter().enumerate() {
            if let Some(vector) = vectors.get(i) {
                let embedding_id = store.insert_embedding(*chunk_id, vector)?;
                store.update_chunk_embedding_id(*chunk_id, embedding_id)?;
                vector_index.insert(embedding_id as usize, vector)?;
            }
        }
    }

    tracing::info!("Reindexed: {}", file_path.display());
    Ok(())
}
