// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use serde_json::json;
use srag_common::types::{JsonRpcRequest, JsonRpcResponse};

#[test]
fn test_jsonrpc_request_new() {
    let req = JsonRpcRequest::new("test_method", json!({"key": "value"}), 42);

    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.method, "test_method");
    assert_eq!(req.id, 42);
    assert_eq!(req.params["key"], "value");
}

#[test]
fn test_jsonrpc_request_serialization() {
    let req = JsonRpcRequest::new("embed", json!({"texts": ["hello", "world"]}), 1);

    let json_str = serde_json::to_string(&req).unwrap();
    assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
    assert!(json_str.contains("\"method\":\"embed\""));
    assert!(json_str.contains("\"id\":1"));

    let parsed: JsonRpcRequest = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.method, "embed");
}

#[test]
fn test_jsonrpc_request_empty_params() {
    let req = JsonRpcRequest::new("ping", json!({}), 0);
    assert_eq!(req.params, json!({}));
}

#[test]
fn test_jsonrpc_request_null_params() {
    let req = JsonRpcRequest::new("ping", json!(null), 0);
    assert!(req.params.is_null());
}

#[test]
fn test_jsonrpc_request_array_params() {
    let req = JsonRpcRequest::new("batch", json!([1, 2, 3]), 5);
    assert!(req.params.is_array());
    assert_eq!(req.params.as_array().unwrap().len(), 3);
}

#[test]
fn test_jsonrpc_response_with_result() {
    let json_str = r#"{
        "jsonrpc": "2.0",
        "result": {"vectors": [[0.1, 0.2, 0.3]]},
        "error": null,
        "id": 1
    }"#;

    let resp: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
    assert!(resp.result.is_some());
    assert!(resp.error.is_none());
    assert_eq!(resp.id, 1);
}

#[test]
fn test_jsonrpc_response_with_error() {
    let json_str = r#"{
        "jsonrpc": "2.0",
        "result": null,
        "error": {
            "code": -32600,
            "message": "Invalid Request",
            "data": null
        },
        "id": 2
    }"#;

    let resp: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
    assert!(resp.result.is_none());
    assert!(resp.error.is_some());

    let err = resp.error.unwrap();
    assert_eq!(err.code, -32600);
    assert_eq!(err.message, "Invalid Request");
}

#[test]
fn test_jsonrpc_error_with_data() {
    let json_str = r#"{
        "jsonrpc": "2.0",
        "result": null,
        "error": {
            "code": -32000,
            "message": "Server error",
            "data": {"details": "Connection failed"}
        },
        "id": 3
    }"#;

    let resp: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
    let err = resp.error.unwrap();
    assert!(err.data.is_some());
    assert_eq!(err.data.unwrap()["details"], "Connection failed");
}

#[test]
fn test_jsonrpc_standard_error_codes() {
    let parse_error = r#"{
        "jsonrpc": "2.0",
        "result": null,
        "error": {"code": -32700, "message": "Parse error", "data": null},
        "id": 1
    }"#;
    let resp: JsonRpcResponse = serde_json::from_str(parse_error).unwrap();
    assert_eq!(resp.error.unwrap().code, -32700);

    let invalid_request = r#"{
        "jsonrpc": "2.0",
        "result": null,
        "error": {"code": -32600, "message": "Invalid Request", "data": null},
        "id": 2
    }"#;
    let resp: JsonRpcResponse = serde_json::from_str(invalid_request).unwrap();
    assert_eq!(resp.error.unwrap().code, -32600);

    let method_not_found = r#"{
        "jsonrpc": "2.0",
        "result": null,
        "error": {"code": -32601, "message": "Method not found", "data": null},
        "id": 3
    }"#;
    let resp: JsonRpcResponse = serde_json::from_str(method_not_found).unwrap();
    assert_eq!(resp.error.unwrap().code, -32601);
}
