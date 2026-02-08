// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchCodeParams {
    #[serde(default)]
    pub project: Option<String>,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindSimilarParams {
    #[serde(default)]
    pub project: Option<String>,
    pub code_snippet: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchSymbolsParams {
    #[serde(default)]
    pub project: Option<String>,
    pub pattern: String,
    #[serde(default = "default_symbol_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileParams {
    #[serde(default)]
    pub project: Option<String>,
    pub file_path: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPatternsParams {
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FtsSearchParams {
    #[serde(default)]
    pub project: Option<String>,
    pub query: String,
    #[serde(default = "default_top_k")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCallersParams {
    #[serde(default)]
    pub project: Option<String>,
    pub function_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindCalleesParams {
    #[serde(default)]
    pub project: Option<String>,
    pub function_name: String,
}

fn default_top_k() -> usize {
    10
}
fn default_symbol_limit() -> usize {
    20
}
