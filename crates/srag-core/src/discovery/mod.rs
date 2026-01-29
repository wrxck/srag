// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use srag_common::Result;

use crate::config::{Config, DEPENDENCY_DIRS};

pub fn walk_directory(root: &Path, config: &Config) -> Result<Vec<PathBuf>> {
    walk_directory_opts(root, config, false)
}

pub fn walk_directory_opts(root: &Path, config: &Config, all: bool) -> Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(root);
    builder.follow_links(false);

    if all {
        builder
            .hidden(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false);
    } else {
        builder
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true);

        // add custom ignore patterns
        let mut overrides = ignore::overrides::OverrideBuilder::new(root);
        for pattern in &config.ignore_patterns {
            let neg = format!("!{}", pattern);
            overrides
                .add(&neg)
                .map_err(|e| srag_common::Error::Discovery(e.to_string()))?;
        }

        // exclude dependency directories unless configured to include them
        if !config.indexing.include_dependencies {
            for dir in DEPENDENCY_DIRS {
                let neg = format!("!{}", dir);
                overrides
                    .add(&neg)
                    .map_err(|e| srag_common::Error::Discovery(e.to_string()))?;
            }
        }

        let overrides = overrides
            .build()
            .map_err(|e| srag_common::Error::Discovery(e.to_string()))?;
        builder.overrides(overrides);
    }

    // check for .sragignore
    let sragignore = root.join(".sragignore");
    if sragignore.exists() {
        builder.add_ignore(&sragignore);
    }

    let mut files = Vec::new();

    for entry in builder.build() {
        let entry = entry.map_err(|e| srag_common::Error::Discovery(e.to_string()))?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Ok(metadata) = path.metadata() {
            if metadata.len() > config.indexing.max_file_size_bytes {
                continue;
            }
        }

        if is_likely_binary(path) {
            continue;
        }

        files.push(path.to_path_buf());
    }

    Ok(files)
}

fn is_likely_binary(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    use std::io::Read;
    let mut buf = [0u8; 512];
    let Ok(n) = file.read(&mut buf) else {
        return true;
    };
    if n == 0 {
        return false;
    }
    let null_count = buf[..n].iter().filter(|&&b| b == 0).count();
    null_count > n / 10
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn test_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_walk_empty_directory() {
        let dir = tempdir().unwrap();
        let files = walk_directory(dir.path(), &test_config()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_walk_finds_text_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub mod foo;").unwrap();

        let files = walk_directory(dir.path(), &test_config()).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_walk_skips_hidden_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("visible.rs"), "code").unwrap();
        std::fs::write(dir.path().join(".hidden"), "hidden").unwrap();

        let files = walk_directory(dir.path(), &test_config()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap() == "visible.rs");
    }

    #[test]
    fn test_walk_skips_binary_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("text.txt"), "hello world").unwrap();
        std::fs::write(dir.path().join("binary.bin"), vec![0u8; 100]).unwrap();

        let files = walk_directory(dir.path(), &test_config()).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_walk_skips_large_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("small.txt"), "small").unwrap();

        let mut config = test_config();
        config.indexing.max_file_size_bytes = 10;
        std::fs::write(dir.path().join("large.txt"), "this is too large").unwrap();

        let files = walk_directory(dir.path(), &config).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_walk_respects_sragignore() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("keep.rs"), "code").unwrap();
        std::fs::write(dir.path().join("ignore.rs"), "ignored").unwrap();
        std::fs::write(dir.path().join(".sragignore"), "ignore.rs").unwrap();

        let files = walk_directory(dir.path(), &test_config()).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap() == "keep.rs");
    }

    #[test]
    fn test_walk_all_includes_hidden() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".hidden"), "hidden").unwrap();

        let files = walk_directory_opts(dir.path(), &test_config(), true).unwrap();
        assert!(files.iter().any(|f| f.file_name().unwrap() == ".hidden"));
    }

    #[test]
    fn test_is_likely_binary_text() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("text.txt");
        std::fs::write(&path, "hello world, this is text").unwrap();
        assert!(!is_likely_binary(&path));
    }

    #[test]
    fn test_is_likely_binary_null_bytes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("binary.bin");
        std::fs::write(&path, vec![0u8; 100]).unwrap();
        assert!(is_likely_binary(&path));
    }

    #[test]
    fn test_is_likely_binary_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty");
        std::fs::write(&path, "").unwrap();
        assert!(!is_likely_binary(&path));
    }
}
