// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod context;
mod prompt;
pub mod retriever;

use anyhow::Result;
use rustyline::DefaultEditor;

use crate::config::Config;
use crate::index::hnsw::{rebuild_hnsw_from_db, VectorIndex};
use crate::index::store::Store;
use crate::ipc::client::MlClient;
use crate::ipc::lifecycle;

use srag_common::types::{Chunk, Project, QueryResult, SourceReference};

/// search vector index and optionally merge with FTS results.
/// sync function to avoid holding &Store across await points.
fn search_and_merge(
    query: &str,
    query_vec: &[f32],
    vector_index: &VectorIndex,
    store: &Store,
    config: &Config,
) -> Result<Vec<(Chunk, String)>> {
    let search_k = if config.query.rerank {
        config.query.broad_k
    } else {
        config.query.top_k
    };

    let vector_results = vector_index.search(query_vec, search_k, config.query.ef_search)?;

    if config.query.hybrid_search {
        let fts_results = store.search_fts(query, search_k).unwrap_or_default();
        retriever::reciprocal_rank_fusion(&vector_results, &fts_results, store, search_k)
            .map_err(Into::into)
    } else {
        retriever::resolve_results(store, &vector_results).map_err(Into::into)
    }
}

/// optionally re-rank retrieved chunks using the cross-encoder.
async fn maybe_rerank(
    query: &str,
    context_chunks: Vec<(Chunk, String)>,
    client: &MlClient,
    config: &Config,
) -> Result<Vec<(Chunk, String)>> {
    if config.query.rerank && context_chunks.len() > 1 {
        let documents: Vec<String> = context_chunks
            .iter()
            .map(|(chunk, _)| chunk.content.clone())
            .collect();

        match client.rerank(query, &documents, config.query.top_k).await {
            Ok(ranked) => Ok(ranked
                .into_iter()
                .filter_map(|(idx, _score)| context_chunks.get(idx).cloned())
                .collect()),
            Err(e) => {
                tracing::warn!("reranking failed, using original order: {}", e);
                Ok(context_chunks
                    .into_iter()
                    .take(config.query.top_k)
                    .collect())
            }
        }
    } else {
        Ok(context_chunks)
    }
}

pub async fn query_once(project: &str, query: &str, config: &Config) -> Result<QueryResult> {
    let db_path = config.db_path();
    if !db_path.exists() {
        anyhow::bail!("no index found. run 'srag index <path>' first.");
    }

    let store = Store::open(&db_path)?;

    let _project_id = store
        .get_project_id(project)
        .map_err(|_| anyhow::anyhow!("project '{}' not found", project))?;

    lifecycle::ensure_ml_service_running(config)?;
    let addr = crate::ipc::client::read_service_addr(&Config::port_file_path())?;
    let client = MlClient::connect(addr).await?;

    let mut vector_index =
        VectorIndex::open(&config.vectors_dir(), crate::config::EMBEDDING_DIMENSION)?;
    rebuild_hnsw_from_db(&store, &mut vector_index)?;

    let query_vectors = client.embed(&[query.to_string()]).await?;
    let query_vec = query_vectors
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no embedding returned for query"))?;

    let context_chunks = search_and_merge(query, &query_vec, &vector_index, &store, config)?;
    let context_chunks = maybe_rerank(query, context_chunks, &client, config).await?;

    let context_text = context::assemble_context(&context_chunks, config.query.context_tokens);

    let built = prompt::build_prompt(query, &context_text, &[]);

    let response = client
        .generate(
            &built.text,
            config.query.max_tokens,
            config.query.temperature,
        )
        .await?;

    if prompt::check_canary(&response, &built.canary) {
        tracing::warn!("canary token detected in LLM response — possible prompt injection");
    }

    let sources: Vec<SourceReference> = context_chunks
        .iter()
        .map(|(chunk, file_path)| SourceReference {
            file_path: file_path.clone(),
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            symbol: chunk.symbol.clone(),
            content: chunk.content.clone(),
        })
        .collect();

    Ok(QueryResult {
        answer: response,
        sources,
    })
}

fn build_scope_description(
    project: Option<&str>,
    languages: &[String],
    projects: &[Project],
) -> String {
    let mut parts = Vec::new();

    match project {
        Some(name) => parts.push(format!("project: {}", name)),
        None if projects.len() == 1 => parts.push(format!("project: {}", projects[0].name)),
        None => parts.push(format!("{} projects", projects.len())),
    }

    if !languages.is_empty() {
        parts.push(format!("languages: {}", languages.join(", ")));
    }

    format!("{}, ", parts.join(", "))
}

pub async fn run_chat_repl(
    project: Option<&str>,
    languages: &[String],
    session_id: Option<&str>,
) -> Result<()> {
    let config = Config::load()?;
    config.ensure_dirs()?;

    let db_path = config.db_path();
    if !db_path.exists() {
        anyhow::bail!("no index found. run 'srag index <path>' first.");
    }

    let store = Store::open(&db_path)?;

    let projects = store.list_projects()?;
    if projects.is_empty() {
        anyhow::bail!("no projects indexed. run 'srag index <path>' first.");
    }

    // resolve project filter
    let project_ids: Option<Vec<i64>> = match project {
        Some(name) => {
            let id = store
                .get_project_id(name)
                .map_err(|_| anyhow::anyhow!("project '{}' not found", name))?;
            Some(vec![id])
        }
        None => None, // all projects
    };

    // resolve language filter
    let language_filter: Vec<String> = if languages.is_empty() {
        Vec::new()
    } else {
        // normalize language names (lowercase)
        languages.iter().map(|l| l.to_lowercase()).collect()
    };

    lifecycle::ensure_ml_service_running(&config)?;
    let addr = crate::ipc::client::read_service_addr(&Config::port_file_path())?;
    let client = MlClient::connect(addr).await?;

    let mut vector_index =
        VectorIndex::open(&config.vectors_dir(), crate::config::EMBEDDING_DIMENSION)?;
    rebuild_hnsw_from_db(&store, &mut vector_index)?;

    let session = match session_id {
        Some(id) => id.to_string(),
        None => uuid::Uuid::new_v4().to_string(),
    };

    let session_label = project.unwrap_or("all");
    let _ = store.create_session(&session, Some(session_label));

    // build file path set for project filtering
    let allowed_files: Option<std::collections::HashSet<String>> =
        if let Some(ref pids) = project_ids {
            let mut files = std::collections::HashSet::new();
            for pid in pids {
                if let Ok(project_files) = store.list_project_files(*pid) {
                    for f in project_files {
                        files.insert(f.path);
                    }
                }
            }
            Some(files)
        } else {
            None
        };

    // build scope description
    let scope_desc = build_scope_description(project, &language_filter, &projects);
    println!("srag chat ({}session: {})", scope_desc, &session[..8]);

    // show available languages if no filter applied
    if language_filter.is_empty() {
        if let Ok(langs) = store.list_indexed_languages() {
            if !langs.is_empty() {
                let lang_list: Vec<String> = langs
                    .iter()
                    .take(10)
                    .map(|l| format!("{} ({})", l.language, l.chunk_count))
                    .collect();
                println!("available languages: {}", lang_list.join(", "));
                println!("use -l/--language to filter");
            }
        }
    }

    println!("type 'quit' or ctrl-d to exit\n");

    let mut editor = DefaultEditor::new()?;

    loop {
        let line = match editor.readline("you> ") {
            Ok(line) => line,
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(rustyline::error::ReadlineError::Interrupted) => break,
            Err(e) => return Err(e.into()),
        };

        let query = line.trim();
        if query.is_empty() {
            continue;
        }
        if query == "quit" || query == "exit" {
            break;
        }

        editor.add_history_entry(query)?;

        let query_vectors = client.embed(&[query.to_string()]).await?;
        let query_vec = query_vectors
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no embedding returned for query"))?;

        let context_chunks = search_and_merge(query, &query_vec, &vector_index, &store, &config)?;

        // filter by project and language
        let context_chunks: Vec<(Chunk, String)> = context_chunks
            .into_iter()
            .filter(|(chunk, file_path)| {
                // language filter
                if !language_filter.is_empty() {
                    let chunk_lang = chunk.language.as_str().to_lowercase();
                    if !language_filter.iter().any(|l| l == &chunk_lang) {
                        return false;
                    }
                }
                // project filter (check if file belongs to allowed projects)
                if let Some(ref allowed) = allowed_files {
                    return allowed.contains(file_path);
                }
                true
            })
            .collect();

        let context_chunks = maybe_rerank(query, context_chunks, &client, &config).await?;

        let context_text = context::assemble_context(&context_chunks, config.query.context_tokens);

        let history = store.get_recent_turns(&session, config.query.history_turns)?;

        let built = prompt::build_prompt(query, &context_text, &history);

        let response = client
            .generate(
                &built.text,
                config.query.max_tokens,
                config.query.temperature,
            )
            .await?;

        if prompt::check_canary(&response, &built.canary) {
            tracing::warn!("canary token detected in LLM response — possible prompt injection");
            println!("[warning: response may be influenced by injected content in source files]");
        }

        println!("\nsrag> {}", response);

        if !context_chunks.is_empty() {
            println!("\nsources:");
            let mut seen = std::collections::HashSet::new();
            for (chunk, file_path) in &context_chunks {
                let source = format!("  {}:{}-{}", file_path, chunk.start_line, chunk.end_line);
                if seen.insert(source.clone()) {
                    println!("{}", source);
                }
            }
        }
        println!();

        let user_turn = srag_common::types::ConversationTurn {
            id: None,
            session_id: session.clone(),
            role: "user".into(),
            content: query.to_string(),
            sources: None,
            created_at: String::new(),
        };
        store.add_turn(&user_turn)?;

        let sources_json = serde_json::to_string(
            &context_chunks
                .iter()
                .map(|(c, p)| format!("{}:{}-{}", p, c.start_line, c.end_line))
                .collect::<Vec<_>>(),
        )
        .ok();

        let assistant_turn = srag_common::types::ConversationTurn {
            id: None,
            session_id: session.clone(),
            role: "assistant".into(),
            content: response,
            sources: sources_json,
            created_at: String::new(),
        };
        store.add_turn(&assistant_turn)?;
    }

    vector_index.save(&config.vectors_dir())?;
    println!("session saved.");

    Ok(())
}
