//! JSON-RPC 2.0 wire types and response helpers.
//!
//! This module contains the minimal set of types needed to implement a
//! JSON-RPC 2.0 server over stdio: incoming message deserialization,
//! outgoing response serialization, and the standard error constructors.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// An incoming JSON-RPC 2.0 message from the client.
#[derive(Debug, Deserialize)]
pub struct IncomingMessage {
    #[allow(dead_code)] // Deserialized from JSON-RPC protocol; required by spec
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
}

/// An outgoing JSON-RPC 2.0 response.
#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: Value,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ---------------------------------------------------------------------------
// Response constructors
// ---------------------------------------------------------------------------

/// Build a success response.
pub fn success_response(id: Value, result: Value) -> Response {
    Response {
        jsonrpc: "2.0",
        result: Some(result),
        error: None,
        id,
    }
}

/// Build a method-not-found error response.
pub fn method_not_found(id: Value, method: &str) -> Response {
    Response {
        jsonrpc: "2.0",
        result: None,
        error: Some(RpcError {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }),
        id,
    }
}

/// Build a parse error response.
pub fn parse_error() -> Response {
    Response {
        jsonrpc: "2.0",
        result: None,
        error: Some(RpcError {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }),
        id: Value::Null,
    }
}

// ---------------------------------------------------------------------------
// I/O helper
// ---------------------------------------------------------------------------

/// Send a JSON-RPC response to a writer as a single newline-terminated line.
pub fn send_response(stdout: &mut impl Write, response: &Response) {
    match serde_json::to_string(response) {
        Ok(json) => {
            if let Err(e) = writeln!(stdout, "{json}") {
                eprintln!("[mcp-server] Failed to write response: {e}");
            }
            if let Err(e) = stdout.flush() {
                eprintln!("[mcp-server] Failed to flush stdout: {e}");
            }
        }
        Err(e) => {
            eprintln!("[mcp-server] Failed to serialize response: {e}");
        }
    }
}
