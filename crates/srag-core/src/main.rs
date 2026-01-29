// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

#![allow(dead_code)]

mod chunking;
mod cli;
mod config;
mod discovery;
mod index;
mod ipc;
mod query;
mod resource;
mod watcher;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    cli.run().await
}
