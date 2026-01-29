// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;

pub async fn run(project: Option<&str>, languages: &[String], session: Option<&str>) -> Result<()> {
    crate::query::run_chat_repl(project, languages, session).await
}
