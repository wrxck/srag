// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use anyhow::Result;

pub async fn run(foreground: bool, stop: bool) -> Result<()> {
    if stop {
        crate::watcher::stop_watcher()?;
        return Ok(());
    }

    if foreground {
        crate::watcher::run_foreground().await
    } else {
        crate::watcher::run_daemon()
    }
}
