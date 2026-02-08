// SPDX-Licence-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod helpers;
mod params;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use rmcp::{
    model::*, tool, tool_handler, tool_router, transport::stdio, ErrorData as McpError,
    ServerHandler, ServiceExt,
};

use crate::config::Config;
use crate::index::store::Store;
use helpers::{ensure_index_exists, format_chunk, resolve_project};
use params::*;

#[derive(Clone)]
struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
    capacity: u32,
    refill_rate: f64,
}

struct RateLimiterState {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(capacity: u32, refill_interval_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimiterState {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            })),
            capacity,
            refill_rate: capacity as f64 / refill_interval_secs as f64,
        }
    }

    fn try_acquire(&self) -> bool {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();

        state.tokens = (state.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        state.last_refill = now;

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct SragMcpServer {
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
    rate_limiter: RateLimiter,
}

#[tool_router]
impl SragMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            rate_limiter: RateLimiter::new(60, 60),
        }
    }

    fn check_rate_limit(&self) -> Result<(), McpError> {
        if !self.rate_limiter.try_acquire() {
            return Err(McpError::internal_error(
                "rate limit exceeded, please try again later",
                None,
            ));
        }
        Ok(())
    }

    #[tool(description = "list all indexed projects with their paths")]
    async fn list_projects(&self) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let db_path = config.db_path();

        let auto_indexed = if !db_path.exists() {
            if config.mcp.auto_index_cwd {
                Some(helpers::auto_index_cwd(&config).await?)
            } else {
                return Ok(CallToolResult::success(vec![Content::text(
                    "no projects indexed yet",
                )]));
            }
        } else {
            None
        };

        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let projects = store
            .list_projects()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if projects.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "no projects indexed yet",
            )]));
        }

        let mut text = String::new();

        if let Some(result) = auto_indexed {
            text.push_str(&format!(
                "[auto-indexed '{}': {}]\n\n",
                result.project_name, result.summary
            ));
        }

        for p in &projects {
            let files = store.file_count(p.id).map_err(|e| {
                McpError::internal_error(format!("Failed to get file count: {}", e), None)
            })?;
            let chunks = store.chunk_count(p.id).map_err(|e| {
                McpError::internal_error(format!("Failed to get chunk count: {}", e), None)
            })?;
            text.push_str(&format!(
                "{}: {} ({} files, {} chunks)\n",
                p.name, p.path, files, chunks
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "semantic search for code - finds relevant code chunks using vector similarity. use this to find implementations, patterns, or examples"
    )]
    async fn search_code(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<SearchCodeParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        config
            .ensure_dirs()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let auto_indexed = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, project_name) = resolve_project(&store, params.project.as_deref())?;

        crate::ipc::lifecycle::ensure_ml_service_running(&config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let addr = crate::ipc::client::read_service_addr(&Config::port_file_path())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let client = crate::ipc::client::MlClient::connect(addr)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let query_vectors = client
            .embed(std::slice::from_ref(&params.query))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let query_vec = query_vectors
            .into_iter()
            .next()
            .ok_or_else(|| McpError::internal_error("no embedding returned", None))?;

        let search_k = if config.query.rerank || config.query.hybrid_search {
            config.query.broad_k
        } else {
            params.top_k
        };

        let vector_results = crate::index::hnsw::search_cached(
            &config.vectors_dir(),
            crate::config::EMBEDDING_DIMENSION,
            &store,
            &query_vec,
            search_k,
            config.query.ef_search,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let project_files: std::collections::HashSet<String> = store
            .list_project_files(project_id)
            .map_err(|e| McpError::internal_error(format!("failed to list files: {}", e), None))?
            .into_iter()
            .map(|f| f.path)
            .collect();

        let context_chunks: Vec<_> = if config.query.hybrid_search {
            let fts_results = store
                .search_fts_project(&params.query, Some(project_id), search_k)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let fused = crate::query::retriever::reciprocal_rank_fusion(
                &vector_results,
                &fts_results,
                &store,
                search_k,
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            fused
                .into_iter()
                .filter(|(_, path)| project_files.contains(path))
                .collect()
        } else {
            let all_chunks = crate::query::retriever::resolve_results(&store, &vector_results)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            all_chunks
                .into_iter()
                .filter(|(_, path)| project_files.contains(path))
                .collect()
        };

        let context_chunks = if config.query.rerank && context_chunks.len() > 1 {
            let documents: Vec<String> = context_chunks
                .iter()
                .map(|(c, _)| c.content.clone())
                .collect();
            match client.rerank(&params.query, &documents, params.top_k).await {
                Ok(ranked) => ranked
                    .into_iter()
                    .filter_map(|(idx, _)| context_chunks.get(idx).cloned())
                    .collect(),
                Err(e) => {
                    tracing::warn!("reranking failed, falling back to original order: {}", e);
                    context_chunks.into_iter().take(params.top_k).collect()
                }
            }
        } else {
            context_chunks
                .into_iter()
                .take(params.top_k)
                .collect::<Vec<_>>()
        };

        let mut text = String::new();
        if let Some(result) = auto_indexed {
            text.push_str(&format!(
                "[auto-indexed '{}': {}]\n\n",
                result.project_name, result.summary
            ));
        }
        text.push_str(&format!(
            "search results from project '{}':\n\n",
            project_name
        ));
        for (chunk, file_path) in &context_chunks {
            text.push_str(&format_chunk(chunk, file_path));
            text.push('\n');
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "find code similar to a given snippet - useful for finding reusable patterns, duplicate code, or related implementations"
    )]
    async fn find_similar_code(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<FindSimilarParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        config
            .ensure_dirs()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let auto_indexed = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (_, project_name) = resolve_project(&store, params.project.as_deref())?;

        crate::ipc::lifecycle::ensure_ml_service_running(&config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let addr = crate::ipc::client::read_service_addr(&Config::port_file_path())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let client = crate::ipc::client::MlClient::connect(addr)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let snippet_vectors = client
            .embed(std::slice::from_ref(&params.code_snippet))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let snippet_vec = snippet_vectors
            .into_iter()
            .next()
            .ok_or_else(|| McpError::internal_error("no embedding returned", None))?;

        let results = crate::index::hnsw::search_cached(
            &config.vectors_dir(),
            crate::config::EMBEDDING_DIMENSION,
            &store,
            &snippet_vec,
            params.top_k * 4,
            config.query.ef_search,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let all_chunks = crate::query::retriever::resolve_results(&store, &results)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let project_id = store.get_project_id(&project_name).map_err(|e| {
            McpError::internal_error(format!("Failed to get project ID: {}", e), None)
        })?;
        let project_files: std::collections::HashSet<String> = store
            .list_project_files(project_id)
            .map_err(|e| McpError::internal_error(format!("Failed to list files: {}", e), None))?
            .into_iter()
            .map(|f| f.path)
            .collect();

        let context_chunks: Vec<_> = all_chunks
            .into_iter()
            .filter(|(_, path)| project_files.contains(path))
            .collect();

        let mut text = String::new();
        if let Some(result) = auto_indexed {
            text.push_str(&format!(
                "[auto-indexed '{}': {}]\n\n",
                result.project_name, result.summary
            ));
        }
        text.push_str(&format!(
            "similar code found in project '{}':\n\n",
            project_name
        ));
        for (i, (chunk, file_path)) in context_chunks.iter().take(params.top_k).enumerate() {
            text.push_str(&format!("{}. ", i + 1));
            text.push_str(&format_chunk(chunk, file_path));
            text.push('\n');
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "search for functions, classes, or other symbols by name pattern - useful for finding specific definitions"
    )]
    async fn search_symbols(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<SearchSymbolsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let _ = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let project_id = if let Some(ref name) = params.project {
            Some(store.get_project_id(name).map_err(|_| {
                McpError::invalid_params(format!("project '{}' not found", name), None)
            })?)
        } else {
            None
        };

        let results = store
            .search_symbols_paginated(&params.pattern, project_id, params.limit, params.offset)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if results.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "no symbols matching '{}' found",
                params.pattern
            ))]));
        }

        let mut text = format!("symbols matching '{}':\n\n", params.pattern);
        for (chunk, file_path) in &results {
            let kind = chunk.symbol_kind.as_deref().unwrap_or("symbol");
            let name = chunk.symbol.as_deref().unwrap_or("unknown");
            text.push_str(&format!(
                "{} {} in {} (lines {}-{})\n",
                kind, name, file_path, chunk.start_line, chunk.end_line
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "get the contents of a file or specific line range - useful for examining code in detail"
    )]
    async fn get_file(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<GetFileParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let _ = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, _) = resolve_project(&store, params.project.as_deref())?;

        let chunks = store
            .get_file_chunks(project_id, &params.file_path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if chunks.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "file '{}' not found in project",
                params.file_path
            ))]));
        }

        let mut content = String::new();
        for chunk in &chunks {
            if let (Some(start), Some(end)) = (params.start_line, params.end_line) {
                if chunk.end_line < start || chunk.start_line > end {
                    continue;
                }
            }
            content.push_str(&chunk.content);
            content.push('\n');
        }

        let header = if let (Some(start), Some(end)) = (params.start_line, params.end_line) {
            format!("--- {} (lines {}-{}) ---\n", params.file_path, start, end)
        } else {
            format!("--- {} ---\n", params.file_path)
        };

        Ok(CallToolResult::success(vec![Content::text(format!(
            "{}{}",
            header, content
        ))]))
    }

    #[tool(
        description = "analyse project patterns - returns common conventions like naming patterns, directory structure, languages used, and symbol types. use this to understand project standards before writing new code"
    )]
    async fn get_project_patterns(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<GetPatternsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let _ = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, project_name) = resolve_project(&store, params.project.as_deref())?;

        let patterns = store
            .get_project_patterns(project_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut text = format!("project '{}' patterns:\n\n", project_name);

        text.push_str("languages:\n");
        for (lang, count) in &patterns.languages {
            text.push_str(&format!("  {}: {} files\n", lang, count));
        }

        text.push_str("\ndirectory structure:\n");
        for (dir, count) in &patterns.directories {
            text.push_str(&format!("  {}/: {} files\n", dir, count));
        }

        text.push_str("\nsymbol types:\n");
        for (kind, count) in &patterns.symbol_kinds {
            text.push_str(&format!("  {}: {}\n", kind, count));
        }

        if !patterns.common_prefixes.is_empty() {
            text.push_str("\ncommon naming prefixes:\n");
            for (prefix, count) in patterns.common_prefixes.iter().take(15) {
                text.push_str(&format!("  {}: {} symbols\n", prefix, count));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "full-text keyword search - searches for exact terms in code. use when you know specific identifiers, strings, or keywords to find"
    )]
    async fn text_search(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<FtsSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let _ = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, project_name) = resolve_project(&store, params.project.as_deref())?;

        let results = store
            .search_fts_project_paginated(
                &params.query,
                Some(project_id),
                params.limit,
                params.offset,
            )
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if results.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "no results for '{}'",
                params.query
            ))]));
        }

        let mut text = format!(
            "text search results for '{}' in '{}':\n\n",
            params.query, project_name
        );
        for (chunk_id, _score) in &results {
            match store.get_chunk_by_id(*chunk_id) {
                Ok(Some((chunk, file_path))) => {
                    text.push_str(&format_chunk(&chunk, &file_path));
                    text.push('\n');
                }
                Ok(None) => {
                    tracing::warn!("chunk {} not found in database", chunk_id);
                }
                Err(e) => {
                    tracing::warn!("failed to retrieve chunk {}: {}", chunk_id, e);
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "find all functions that call a specific function - useful for understanding dependencies and impact of changes"
    )]
    async fn find_callers(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<FindCallersParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let _ = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, project_name) = resolve_project(&store, params.project.as_deref())?;

        let callers = store
            .find_callers(project_id, &params.function_name)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if callers.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "no callers found for '{}' in project '{}'",
                params.function_name, project_name
            ))]));
        }

        let mut text = format!(
            "functions that call '{}' in '{}':\n\n",
            params.function_name, project_name
        );
        for entry in &callers {
            let scope = entry
                .scope
                .as_ref()
                .map(|s| format!("{}::", s))
                .unwrap_or_default();
            text.push_str(&format!(
                "  {} {}{} in {}:{}-{}\n",
                entry.definition_kind,
                scope,
                entry.definition_name,
                entry.file_path,
                entry.start_line,
                entry.end_line
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        description = "find all functions called by a specific function - useful for understanding what a function depends on"
    )]
    async fn find_callees(
        &self,
        rmcp::handler::server::wrapper::Parameters(params): rmcp::handler::server::wrapper::Parameters<FindCalleesParams>,
    ) -> Result<CallToolResult, McpError> {
        self.check_rate_limit()?;
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let _ = ensure_index_exists(&config).await?;

        let db_path = config.db_path();
        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, project_name) = resolve_project(&store, params.project.as_deref())?;

        let callees = store
            .find_callees(project_id, &params.function_name)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        if callees.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "no callees found for '{}' in project '{}' (function may not exist or makes no calls)",
                params.function_name, project_name
            ))]));
        }

        let mut text = format!(
            "functions called by '{}' in '{}':\n\n",
            params.function_name, project_name
        );
        for entry in &callees {
            let scope = entry
                .scope
                .as_ref()
                .map(|s| format!("{}::", s))
                .unwrap_or_default();
            text.push_str(&format!(
                "  {} {}{} in {}:{}-{}\n",
                entry.definition_kind,
                scope,
                entry.definition_name,
                entry.file_path,
                entry.start_line,
                entry.end_line
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[tool_handler]
impl ServerHandler for SragMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "srag provides semantic code search across your indexed repositories. \
                use search_code for natural language queries, find_similar_code to discover \
                reusable patterns, search_symbols to find definitions, get_project_patterns \
                to understand conventions, and text_search for exact keyword matches. \
                projects are auto-detected from the current directory when not specified."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run() -> Result<()> {
    let server = SragMcpServer::new();
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
