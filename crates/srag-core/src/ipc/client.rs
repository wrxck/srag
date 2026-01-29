// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use srag_common::types::{JsonRpcRequest, JsonRpcResponse};
use srag_common::{Error, Result};

pub struct MlClient {
    stream: tokio::sync::Mutex<TcpStream>,
    next_id: AtomicU64,
    auth_token: Option<String>,
}

/// read the port file written by the python ML service and return the address.
pub fn read_service_addr(port_file: &Path) -> Result<SocketAddr> {
    let content = std::fs::read_to_string(port_file).map_err(|e| {
        Error::Ipc(format!(
            "failed to read port file {}: {}",
            port_file.display(),
            e
        ))
    })?;
    let port: u16 = content
        .trim()
        .parse()
        .map_err(|e| Error::Ipc(format!("Invalid port in {}: {}", port_file.display(), e)))?;
    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}

impl MlClient {
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let stream =
            tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(addr))
                .await
                .map_err(|_| Error::Ipc(format!("Connection to {} timed out", addr)))?
                .map_err(|e| Error::Ipc(format!("Failed to connect to {}: {}", addr, e)))?;
        let auth_token = crate::ipc::lifecycle::read_auth_token().ok();
        Ok(Self {
            stream: tokio::sync::Mutex::new(stream),
            next_id: AtomicU64::new(1),
            auth_token,
        })
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn send(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let json = if let Some(ref token) = self.auth_token {
            let mut val = serde_json::to_value(request)?;
            if let Some(obj) = val.as_object_mut() {
                obj.insert("_auth".into(), serde_json::Value::String(token.clone()));
            }
            serde_json::to_vec(&val)?
        } else {
            serde_json::to_vec(request)?
        };
        let len = json.len() as u32;

        let mut stream = self.stream.lock().await;
        stream
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| Error::Ipc(e.to_string()))?;
        stream
            .write_all(&json)
            .await
            .map_err(|e| Error::Ipc(e.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|e| Error::Ipc(e.to_string()))?;

        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| Error::Ipc(e.to_string()))?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        if resp_len > 10 * 1024 * 1024 {
            return Err(Error::Ipc(format!("Response too large: {}", resp_len)));
        }

        let mut resp_buf = vec![0u8; resp_len];
        stream
            .read_exact(&mut resp_buf)
            .await
            .map_err(|e| Error::Ipc(e.to_string()))?;

        let response: JsonRpcResponse = serde_json::from_slice(&resp_buf)?;
        Ok(response)
    }

    pub async fn ping(&self) -> Result<bool> {
        let req = JsonRpcRequest::new("ping", serde_json::json!({}), self.next_id());
        let resp = self.send(&req).await?;
        Ok(resp.result.is_some())
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let req = JsonRpcRequest::new("embed", serde_json::json!({"texts": texts}), self.next_id());
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(Error::Ipc(err.message));
        }

        let result = resp
            .result
            .ok_or_else(|| Error::Ipc("No result in response".into()))?;
        let vectors: Vec<Vec<f32>> =
            serde_json::from_value(result.get("vectors").cloned().unwrap_or_default())?;
        Ok(vectors)
    }

    pub async fn generate(
        &self,
        prompt: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String> {
        let req = JsonRpcRequest::new(
            "generate",
            serde_json::json!({
                "prompt": prompt,
                "max_tokens": max_tokens,
                "temperature": temperature,
            }),
            self.next_id(),
        );
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(Error::Ipc(err.message));
        }

        let result = resp
            .result
            .ok_or_else(|| Error::Ipc("No result in response".into()))?;
        let text = result
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(text)
    }

    pub async fn rerank(
        &self,
        query: &str,
        documents: &[String],
        top_k: usize,
    ) -> Result<Vec<(usize, f32)>> {
        let req = JsonRpcRequest::new(
            "rerank",
            serde_json::json!({
                "query": query,
                "documents": documents,
                "top_k": top_k,
            }),
            self.next_id(),
        );
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(Error::Ipc(err.message));
        }

        let result = resp
            .result
            .ok_or_else(|| Error::Ipc("No result in response".into()))?;
        let ranked: Vec<(usize, f32)> =
            serde_json::from_value(result.get("results").cloned().unwrap_or_default())?;
        Ok(ranked)
    }

    pub async fn model_status(&self) -> Result<srag_common::types::ModelStatus> {
        let req = JsonRpcRequest::new("model_status", serde_json::json!({}), self.next_id());
        let resp = self.send(&req).await?;

        if let Some(err) = resp.error {
            return Err(Error::Ipc(err.message));
        }

        let result = resp.result.ok_or_else(|| Error::Ipc("No result".into()))?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn shutdown_service(&self) -> Result<()> {
        let req = JsonRpcRequest::new("shutdown", serde_json::json!({}), self.next_id());
        let _ = self.send(&req).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_read_service_addr_valid() {
        let dir = tempdir().unwrap();
        let port_file = dir.path().join("port");
        std::fs::write(&port_file, "12345").unwrap();

        let addr = read_service_addr(&port_file).unwrap();
        assert_eq!(addr.port(), 12345);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn test_read_service_addr_with_whitespace() {
        let dir = tempdir().unwrap();
        let port_file = dir.path().join("port");
        std::fs::write(&port_file, "  8080\n").unwrap();

        let addr = read_service_addr(&port_file).unwrap();
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_read_service_addr_missing_file() {
        let dir = tempdir().unwrap();
        let port_file = dir.path().join("nonexistent");

        let result = read_service_addr(&port_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_service_addr_invalid_port() {
        let dir = tempdir().unwrap();
        let port_file = dir.path().join("port");
        std::fs::write(&port_file, "not_a_number").unwrap();

        let result = read_service_addr(&port_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_service_addr_port_out_of_range() {
        let dir = tempdir().unwrap();
        let port_file = dir.path().join("port");
        std::fs::write(&port_file, "99999").unwrap();

        let result = read_service_addr(&port_file);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_service_addr_empty_file() {
        let dir = tempdir().unwrap();
        let port_file = dir.path().join("port");
        std::fs::write(&port_file, "").unwrap();

        let result = read_service_addr(&port_file);
        assert!(result.is_err());
    }
}
