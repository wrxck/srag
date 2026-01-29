// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Command;

use srag_common::{Error, Result};

use crate::config::Config;
use crate::ipc::client::read_service_addr;

pub fn python_executable(config: &Config) -> PathBuf {
    let venv_dir = config.data_dir.join("venv");
    #[cfg(unix)]
    let venv_python = venv_dir.join("bin").join("python3");
    #[cfg(windows)]
    let venv_python = venv_dir.join("Scripts").join("python.exe");

    if venv_python.exists() {
        venv_python
    } else {
        #[cfg(unix)]
        {
            PathBuf::from("python3")
        }
        #[cfg(windows)]
        {
            PathBuf::from("python")
        }
    }
}

fn probe_service(addr: SocketAddr) -> bool {
    TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2)).is_ok()
}

pub fn ensure_ml_service_running(config: &Config) -> Result<()> {
    let port_file = Config::port_file_path();
    if port_file.exists() {
        if let Ok(addr) = read_service_addr(&port_file) {
            if probe_service(addr) {
                return Ok(());
            }
        }
        // stale port file, remove it
        let _ = std::fs::remove_file(&port_file);
    }

    let runtime_dir = Config::runtime_dir();
    std::fs::create_dir_all(&runtime_dir)?;

    // generate auth token and write to file with restrictive permissions
    let token = generate_auth_token();
    let token_path = Config::token_file_path();
    {
        use std::io::Write;
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&token_path)?;
            f.write_all(token.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&token_path, &token)?;
        }
    }

    let python = python_executable(config);
    let python_pkg = find_python_package()?;

    let child = Command::new(&python)
        .arg("-m")
        .arg("srag_ml")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg("0")
        .arg("--port-file")
        .arg(&port_file)
        .arg("--models-dir")
        .arg(config.models_dir())
        .arg("--auth-token")
        .arg(&token)
        .arg("--model-filename")
        .arg(&config.llm.model_filename)
        .arg("--model-url")
        .arg(&config.llm.model_url)
        .arg("--llm-threads")
        .arg(config.llm.threads.to_string())
        .arg("--llm-context-size")
        .arg(config.llm.context_size.to_string())
        .arg("--api-provider")
        .arg(match config.api.provider {
            crate::config::ApiProvider::Local => "local",
            crate::config::ApiProvider::Anthropic => "anthropic",
            crate::config::ApiProvider::OpenAI => "openai",
        })
        .arg("--api-model")
        .arg(&config.api.model)
        .arg("--api-max-tokens")
        .arg(config.api.max_tokens.to_string())
        .arg("--redact-secrets")
        .arg(config.api.redact_secrets.to_string())
        .arg("--api-key-file")
        .arg(config.api_key_path())
        .current_dir(&python_pkg)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| Error::Ipc(format!("Failed to start Python ML service: {}", e)))?;

    tracing::info!("started ML service (pid {})", child.id());

    // wait for the port file to appear and service to respond
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    loop {
        if port_file.exists() {
            if let Ok(addr) = read_service_addr(&port_file) {
                if probe_service(addr) {
                    return Ok(());
                }
            }
        }

        if start.elapsed() > timeout {
            return Err(Error::Ipc(
                "timed out waiting for ML service to start".into(),
            ));
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn stop_ml_service() -> Result<()> {
    let port_file = Config::port_file_path();
    if port_file.exists() {
        if let Ok(addr) = read_service_addr(&port_file) {
            if let Ok(mut stream) =
                TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(2))
            {
                use std::io::Write;
                let mut req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "shutdown",
                    "params": {},
                    "id": 0
                });
                if let Ok(token) = read_auth_token() {
                    req.as_object_mut()
                        .unwrap()
                        .insert("_auth".into(), serde_json::Value::String(token));
                }
                let json = serde_json::to_vec(&req).unwrap();
                let len = json.len() as u32;
                let mut buf = Vec::with_capacity(4 + json.len());
                buf.extend_from_slice(&len.to_be_bytes());
                buf.extend_from_slice(&json);
                let _ = stream.write_all(&buf);
            }
        }
        let _ = std::fs::remove_file(&port_file);
    }
    let _ = std::fs::remove_file(Config::token_file_path());
    Ok(())
}

fn generate_auth_token() -> String {
    let mut bytes = [0u8; 32];
    #[cfg(unix)]
    {
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            let _ = f.read_exact(&mut bytes);
        }
    }
    #[cfg(not(unix))]
    {
        // fallback: hash multiple RandomState instances for some entropy
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        for i in 0..4 {
            let s = RandomState::new();
            let mut h = s.build_hasher();
            h.write_usize(i);
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
            );
            let val = h.finish();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&val.to_le_bytes());
        }
    }
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// read the auth token from the token file.
pub fn read_auth_token() -> Result<String> {
    let token_path = Config::token_file_path();
    let token = std::fs::read_to_string(&token_path)
        .map_err(|e| Error::Ipc(format!("Failed to read auth token: {}", e)))?;
    Ok(token.trim().to_string())
}

fn find_python_package() -> Result<PathBuf> {
    let exe = std::env::current_exe().map_err(|e| Error::Ipc(e.to_string()))?;
    let exe_dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));

    let candidates = [
        exe_dir.join("../../python"),
        exe_dir.join("../python"),
        exe_dir.join("python"),
        PathBuf::from("python"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python"),
    ];

    for candidate in &candidates {
        let resolved = if candidate.is_relative() {
            std::env::current_dir().unwrap_or_default().join(candidate)
        } else {
            candidate.clone()
        };
        if resolved.join("srag_ml").exists() {
            return Ok(resolved);
        }
    }

    Err(Error::Ipc("Could not find Python srag_ml package".into()))
}
