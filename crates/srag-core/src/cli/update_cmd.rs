// SPDX-Licence-Identifier: GPL-3.0

use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_GITHUB_REPO: &str = "https://github.com/wrxck/srag";
const VERSION_FILE: &str = ".srag_version";

pub async fn run(force: bool) -> Result<()> {
    let source_dir = get_source_dir()?;

    println!("updating srag...");
    println!("source directory: {}", source_dir.display());

    if !source_dir.join(".git").exists() {
        bail!(
            "source directory is not a git repository. \
             please clone from {} first",
            DEFAULT_GITHUB_REPO
        );
    }

    let current = get_current_commit(&source_dir)?;
    println!("current commit: {}", &current[..8]);

    println!("fetching latest changes...");
    let fetch = Command::new("git")
        .args(["fetch", "origin", "main"])
        .current_dir(&source_dir)
        .output()?;

    if !fetch.status.success() {
        bail!(
            "failed to fetch: {}",
            String::from_utf8_lossy(&fetch.stderr)
        );
    }

    let remote = get_remote_commit(&source_dir)?;

    if current == remote && !force {
        println!("already up to date ({})", &current[..8]);
        return Ok(());
    }

    println!("new version available: {}", &remote[..8]);

    println!("pulling changes...");
    let pull = Command::new("git")
        .args(["pull", "origin", "main"])
        .current_dir(&source_dir)
        .output()?;

    if !pull.status.success() {
        bail!("failed to pull: {}", String::from_utf8_lossy(&pull.stderr));
    }

    println!("rebuilding...");
    let install_script = source_dir.join("install.sh");
    if !install_script.exists() {
        bail!("install.sh not found in source directory");
    }

    let install = Command::new("bash")
        .arg(&install_script)
        .current_dir(&source_dir)
        .status()?;

    if !install.success() {
        bail!("install.sh failed");
    }

    save_version(&remote)?;
    println!("updated to {}", &remote[..8]);

    Ok(())
}

pub async fn check() -> Result<()> {
    let source_dir = match get_source_dir() {
        Ok(d) => d,
        Err(_) => {
            return Ok(());
        }
    };

    if !source_dir.join(".git").exists() {
        return Ok(());
    }

    let current = match get_current_commit(&source_dir) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    let fetch = Command::new("git")
        .args(["fetch", "origin", "main", "--quiet"])
        .current_dir(&source_dir)
        .output();

    if fetch.is_err() {
        return Ok(());
    }

    let remote = match get_remote_commit(&source_dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };

    if current != remote {
        eprintln!(
            "\n\x1b[33msrag update available\x1b[0m: {} -> {}\n\
             run \x1b[1msrag update\x1b[0m to install\n",
            &current[..8],
            &remote[..8]
        );
    }

    Ok(())
}

pub fn print_shell_hook() {
    println!(
        r#"# srag update check - add to ~/.bashrc or ~/.zshrc
if command -v srag &> /dev/null; then
    srag check-update 2>/dev/null &
fi"#
    );
}

fn get_source_dir() -> Result<PathBuf> {
    let config = crate::config::Config::load()?;
    if let Some(ref dir) = config.source_dir {
        let path = PathBuf::from(dir);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Ok(dir) = std::env::var("SRAG_SOURCE_DIR") {
        let path = PathBuf::from(&dir);
        if path.exists() {
            return Ok(path);
        }
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot find home directory"))?;
    let default = home.join("system-rag");
    if default.exists() {
        return Ok(default);
    }

    bail!(
        "source directory not found. set source_dir in config.toml \
         or SRAG_SOURCE_DIR environment variable"
    );
}

fn get_current_commit(source_dir: &PathBuf) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(source_dir)
        .output()?;

    if !output.status.success() {
        bail!("failed to get current commit");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_remote_commit(source_dir: &PathBuf) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "origin/main"])
        .current_dir(source_dir)
        .output()?;

    if !output.status.success() {
        bail!("failed to get remote commit");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn save_version(commit: &str) -> Result<()> {
    let config = crate::config::Config::load()?;
    let version_file = config.data_dir().join(VERSION_FILE);
    std::fs::write(version_file, commit)?;
    Ok(())
}
