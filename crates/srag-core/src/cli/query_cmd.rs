// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;

use crate::config::Config;

pub async fn run(project: &str, query: &str, json_output: bool) -> Result<()> {
    let config = Config::load()?;
    config.ensure_dirs()?;

    let result = crate::query::query_once(project, query, &config).await?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("{}", result.answer);

        if !result.sources.is_empty() {
            println!("\nSources:");
            let mut seen = std::collections::HashSet::new();
            for src in &result.sources {
                let label = format!("  {}:{}-{}", src.file_path, src.start_line, src.end_line);
                if seen.insert(label.clone()) {
                    println!("{}", label);
                }
            }
        }
    }

    Ok(())
}
