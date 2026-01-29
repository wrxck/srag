// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use srag_common::types::JsonRpcRequest;
use srag_common::Result;

pub fn encode_message(request: &JsonRpcRequest) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(request)?;
    let len = json.len() as u32;
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(&json);
    Ok(buf)
}

pub fn decode_length(buf: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_message() {
        let req = JsonRpcRequest::new("test", json!({}), 1);
        let encoded = encode_message(&req).unwrap();

        assert!(encoded.len() > 4);
        let len = decode_length(&[encoded[0], encoded[1], encoded[2], encoded[3]]);
        assert_eq!(len as usize, encoded.len() - 4);
    }

    #[test]
    fn test_decode_length() {
        let buf = [0x00, 0x00, 0x00, 0x0A];
        assert_eq!(decode_length(&buf), 10);

        let buf = [0x00, 0x00, 0x01, 0x00];
        assert_eq!(decode_length(&buf), 256);

        let buf = [0x00, 0x01, 0x00, 0x00];
        assert_eq!(decode_length(&buf), 65536);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let req = JsonRpcRequest::new("embed", json!({"texts": ["hello"]}), 42);
        let encoded = encode_message(&req).unwrap();

        let len = decode_length(&[encoded[0], encoded[1], encoded[2], encoded[3]]);
        let json_bytes = &encoded[4..];
        assert_eq!(json_bytes.len(), len as usize);

        let decoded: JsonRpcRequest = serde_json::from_slice(json_bytes).unwrap();
        assert_eq!(decoded.method, "embed");
        assert_eq!(decoded.id, 42);
    }

    #[test]
    fn test_big_endian_encoding() {
        let req = JsonRpcRequest::new("x", json!(null), 1);
        let mut json = serde_json::to_vec(&req).unwrap();
        while json.len() < 300 {
            json.push(b' ');
        }

        let len = json.len() as u32;
        let bytes = len.to_be_bytes();
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[1], 0x00);
        assert_eq!(bytes[2], 0x01);
        assert_eq!(bytes[3], 0x2C);
    }
}
