// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

mod chat_cmd;
mod config_cmd;
pub(crate) mod index_cmd;
mod mcp;
mod query_cmd;
mod remove_cmd;
mod setup_cmd;
mod status_cmd;
mod sync_cmd;
mod watch_cmd;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "srag",
    about = "System RAG - local code repository search and chat"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// index a code directory
    Index {
        /// path to the directory to index
        path: String,
        /// project name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
        /// force full re-index, ignoring cache
        #[arg(long)]
        force: bool,
        /// dry run: show what would be indexed without indexing
        #[arg(long)]
        dry_run: bool,
        /// index all files: include hidden files, .env, configs, and ignore .gitignore
        #[arg(long)]
        all: bool,
    },
    /// start file watcher daemon for auto-reindexing
    Watch {
        /// run in foreground instead of daemonising
        #[arg(long)]
        foreground: bool,
        /// stop a running watcher
        #[arg(long)]
        stop: bool,
    },
    /// interactive chat REPL with indexed code
    Chat {
        /// project to query (defaults to all projects)
        #[arg(long)]
        project: Option<String>,
        /// filter by language(s) - can specify multiple
        #[arg(long, short = 'l')]
        language: Vec<String>,
        /// resume a previous session
        #[arg(long)]
        session: Option<String>,
    },
    /// non-interactive query against indexed code
    Query {
        /// project to query
        #[arg(short, long)]
        project: String,
        /// the question to ask
        #[arg(short, long)]
        query: String,
        /// output as JSON
        #[arg(long)]
        json: bool,
    },
    /// interactive setup wizard: scan and index projects
    Setup {
        /// index all files: include hidden files, .env, configs, and ignore .gitignore
        #[arg(long)]
        all: bool,
    },
    /// show index statistics
    Status {
        /// show detailed per-project stats
        #[arg(long)]
        detailed: bool,
    },
    /// manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// re-index all registered projects (incremental, skips unchanged files)
    Sync,
    /// start MCP server (stdio transport) for agent integration
    Mcp,
    /// remove a project from the index
    Remove {
        /// project name to remove
        project: String,
        /// skip confirmation prompt
        #[arg(long, short = 'y')]
        force: bool,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// show current configuration
    Show,
    /// set a configuration value
    Set { key: String, value: String },
    /// reset configuration to defaults
    Reset,
    /// open config file in $EDITOR
    Edit,
    /// set API key for external providers (anthropic/openai)
    ApiKey {
        /// API key value (omit to enter interactively)
        key: Option<String>,
    },
    /// check safety of current API configuration
    ApiCheck,
}

impl Cli {
    pub async fn run(self) -> anyhow::Result<()> {
        match self.command {
            Commands::Index {
                path,
                name,
                force,
                dry_run,
                all,
            } => index_cmd::run_opts(&path, name.as_deref(), force, dry_run, all).await,
            Commands::Watch { foreground, stop } => watch_cmd::run(foreground, stop).await,
            Commands::Chat {
                project,
                language,
                session,
            } => chat_cmd::run(project.as_deref(), &language, session.as_deref()).await,
            Commands::Query {
                project,
                query,
                json,
            } => {
                //TODO: add a way to specify the project and the query from the command line
                query_cmd::run(&project, &query, json).await
            }
            Commands::Setup { all } => setup_cmd::run(all).await,
            Commands::Status { detailed } => status_cmd::run(detailed).await,
            Commands::Config { action } => match action {
                ConfigAction::Show => config_cmd::show().await,
                ConfigAction::Set { key, value } => config_cmd::set(&key, &value).await,
                ConfigAction::Reset => config_cmd::reset().await,
                ConfigAction::Edit => config_cmd::edit().await,
                ConfigAction::ApiKey { key } => config_cmd::set_api_key(key.as_deref()).await,
                ConfigAction::ApiCheck => config_cmd::check_api_safety().await,
            },
            Commands::Sync => sync_cmd::run().await,
            Commands::Mcp => mcp::run().await,
            Commands::Remove { project, force } => remove_cmd::run(&project, force).await,
        }
    }
}
