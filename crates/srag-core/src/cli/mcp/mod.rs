// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod helpers;
mod params;

use anyhow::Result;
use rmcp::{
    model::*, tool, tool_handler, tool_router, transport::stdio, ErrorData as McpError,
    ServerHandler, ServiceExt,
};

use crate::config::Config;
use crate::index::store::Store;
use helpers::{format_chunk, resolve_project};
use params::*;

#[derive(Clone)]
pub struct SragMcpServer {
    tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
}

#[tool_router]
impl SragMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "list all indexed projects with their paths")]
    async fn list_projects(&self) -> Result<CallToolResult, McpError> {
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let db_path = config.db_path();

        if !db_path.exists() {
            return Ok(CallToolResult::success(vec![Content::text(
                "no projects indexed yet",
            )]));
        }

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
        for p in &projects {
            let files = store.file_count(p.id).unwrap_or(0);
            let chunks = store.chunk_count(p.id).unwrap_or(0);
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
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        config
            .ensure_dirs()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let db_path = config.db_path();
        if !db_path.exists() {
            return Err(McpError::internal_error("No index found", None));
        }

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

        let mut vector_index = crate::index::hnsw::VectorIndex::open(
            &config.vectors_dir(),
            crate::config::EMBEDDING_DIMENSION,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        crate::index::hnsw::rebuild_hnsw_from_db(&store, &mut vector_index)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let query_vectors = client
            .embed(std::slice::from_ref(&params.query))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let query_vec = query_vectors
            .into_iter()
            .next()
            .ok_or_else(|| McpError::internal_error("No embedding returned", None))?;

        let search_k = if config.query.rerank {
            config.query.broad_k
        } else {
            params.top_k
        };

        let results = vector_index
            .search(&query_vec, search_k, config.query.ef_search)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let all_chunks = crate::query::retriever::resolve_results(&store, &results)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let project_files: std::collections::HashSet<String> = store
            .list_project_files(store.get_project_id(&project_name).unwrap_or(0))
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.path)
            .collect();

        let context_chunks: Vec<_> = all_chunks
            .into_iter()
            .filter(|(_, path)| project_files.contains(path))
            .collect();

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
                Err(_) => context_chunks.into_iter().take(params.top_k).collect(),
            }
        } else {
            context_chunks
                .into_iter()
                .take(params.top_k)
                .collect::<Vec<_>>()
        };

        let mut text = format!("search results from project '{}':\n\n", project_name);
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
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        config
            .ensure_dirs()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let db_path = config.db_path();
        if !db_path.exists() {
            return Err(McpError::internal_error("No index found", None));
        }

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

        let mut vector_index = crate::index::hnsw::VectorIndex::open(
            &config.vectors_dir(),
            crate::config::EMBEDDING_DIMENSION,
        )
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        crate::index::hnsw::rebuild_hnsw_from_db(&store, &mut vector_index)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let snippet_vectors = client
            .embed(std::slice::from_ref(&params.code_snippet))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let snippet_vec = snippet_vectors
            .into_iter()
            .next()
            .ok_or_else(|| McpError::internal_error("No embedding returned", None))?;

        let results = vector_index
            .search(&snippet_vec, params.top_k * 4, config.query.ef_search)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let all_chunks = crate::query::retriever::resolve_results(&store, &results)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let project_files: std::collections::HashSet<String> = store
            .list_project_files(store.get_project_id(&project_name).unwrap_or(0))
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.path)
            .collect();

        let context_chunks: Vec<_> = all_chunks
            .into_iter()
            .filter(|(_, path)| project_files.contains(path))
            .collect();

        let mut text = format!("similar code found in project '{}':\n\n", project_name);
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
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let db_path = config.db_path();

        if !db_path.exists() {
            return Err(McpError::internal_error("No index found", None));
        }

        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let project_id = if let Some(ref name) = params.project {
            Some(store.get_project_id(name).map_err(|_| {
                McpError::invalid_params(format!("Project '{}' not found", name), None)
            })?)
        } else {
            None
        };

        let results = store
            .search_symbols(&params.pattern, project_id, params.limit)
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
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let db_path = config.db_path();

        if !db_path.exists() {
            return Err(McpError::internal_error("No index found", None));
        }

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
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let db_path = config.db_path();

        if !db_path.exists() {
            return Err(McpError::internal_error("No index found", None));
        }

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
        let config = Config::load().map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let db_path = config.db_path();

        if !db_path.exists() {
            return Err(McpError::internal_error("No index found", None));
        }

        let store =
            Store::open(&db_path).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let (project_id, project_name) = resolve_project(&store, params.project.as_deref())?;

        let results = store
            .search_fts_project(&params.query, Some(project_id), params.limit)
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
            if let Ok(Some((chunk, file_path))) = store.get_chunk_by_id(*chunk_id) {
                text.push_str(&format_chunk(&chunk, &file_path));
                text.push('\n');
            }
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
