use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{Mutex, mpsc, oneshot};

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request (or notification when `id` is `None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    pub id: Option<u64>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// A raw incoming JSON-RPC message that can be classified as a response,
/// notification, or an RPC call from the remote side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl IncomingMessage {
    /// A response has no `method` and carries `result` or `error`.
    pub fn is_response(&self) -> bool {
        self.method.is_none() && (self.result.is_some() || self.error.is_some())
    }

    /// A notification has a `method` but no `id`.
    pub fn is_notification(&self) -> bool {
        self.method.is_some() && self.id.is_none()
    }

    /// An RPC call from the remote side has both `method` and `id`.
    pub fn is_rpc_call(&self) -> bool {
        self.method.is_some() && self.id.is_some()
    }

    /// Convert into a [`Response`] (only valid when [`is_response`] is true).
    pub fn into_response(self) -> Response {
        Response {
            jsonrpc: self.jsonrpc,
            result: self.result,
            error: self.error,
            id: self.id,
        }
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 client that communicates over line-delimited JSON on the
/// stdin/stdout of a child process.
pub struct JsonRpcClient {
    /// Writer half — protected by a mutex so multiple tasks can send.
    writer: Arc<Mutex<ChildStdin>>,
    /// Monotonically increasing request id counter.
    next_id: Arc<AtomicU64>,
    /// Pending requests awaiting a response, keyed by request id.
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>>,
    /// Receiver side — handed out exactly once via `take_incoming()`.
    incoming_rx: Option<mpsc::UnboundedReceiver<IncomingMessage>>,
}

impl JsonRpcClient {
    /// Create a new client.
    ///
    /// Spawns a background tokio task that reads line-delimited JSON from
    /// `stdout`, routing responses to their pending futures and everything
    /// else (notifications / incoming RPC calls) to an mpsc channel
    /// retrievable via [`take_incoming`].
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel::<IncomingMessage>();

        // Spawn the reader task.
        let reader_pending = Arc::clone(&pending);
        let reader_tx = incoming_tx;
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF — child process closed stdout.
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        let msg: IncomingMessage = match serde_json::from_str(trimmed) {
                            Ok(m) => m,
                            Err(e) => {
                                log::error!("Failed to parse JSON-RPC message: {e}");
                                continue;
                            }
                        };

                        if msg.is_response() {
                            // Route to the pending request future.
                            if let Some(id) = msg.id {
                                let mut map = reader_pending.lock().await;
                                if let Some(tx) = map.remove(&id) {
                                    let _ = tx.send(msg.into_response());
                                } else {
                                    log::error!("Received response for unknown request id {id}");
                                }
                            } else {
                                log::error!("Received response without id: {trimmed}");
                            }
                        } else {
                            // Notification or incoming RPC call.
                            if reader_tx.send(msg).is_err() {
                                // Receiver dropped — stop reading.
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error reading from child stdout: {e}");
                        break;
                    }
                }
            }
        });

        Self {
            writer: Arc::new(Mutex::new(stdin)),
            next_id: Arc::new(AtomicU64::new(1)),
            pending,
            incoming_rx: Some(incoming_rx),
        }
    }

    /// Take the receiver for incoming notifications and RPC calls.
    ///
    /// This can only be called once — subsequent calls return `None`.
    pub fn take_incoming(&mut self) -> Option<mpsc::UnboundedReceiver<IncomingMessage>> {
        self.incoming_rx.take()
    }

    /// Send a JSON-RPC request and wait for the matching response.
    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let req = Request {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: Some(id),
        };

        let (tx, rx) = oneshot::channel::<Response>();

        // Register the pending request before writing to avoid races.
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        // Serialize and send.
        let json = serde_json::to_string(&req)?;
        {
            let mut writer = self.writer.lock().await;
            writer.write_all(format!("{json}\n").as_bytes()).await?;
            writer.flush().await?;
        }

        // Wait for the response.
        let response = rx.await?;
        Ok(response)
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    pub async fn notify(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let req = Request {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: None,
        };

        let json = serde_json::to_string(&req)?;
        let mut writer = self.writer.lock().await;
        writer.write_all(format!("{json}\n").as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Send a JSON-RPC response to an incoming RPC call from the agent.
    pub async fn respond(
        &self,
        id: u64,
        result: Option<Value>,
        error: Option<RpcError>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let resp = Response {
            jsonrpc: "2.0".to_string(),
            result,
            error,
            id: Some(id),
        };

        let json = serde_json::to_string(&resp)?;
        let mut writer = self.writer.lock().await;
        writer.write_all(format!("{json}\n").as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incoming_message_classification() {
        let msg: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#).unwrap();
        assert!(msg.is_response());
        assert!(!msg.is_notification());
        assert!(!msg.is_rpc_call());

        let msg: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#)
                .unwrap();
        assert!(!msg.is_response());
        assert!(msg.is_notification());
        assert!(!msg.is_rpc_call());

        let msg: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":5,"method":"session/request_permission","params":{}}"#,
        )
        .unwrap();
        assert!(!msg.is_response());
        assert!(!msg.is_notification());
        assert!(msg.is_rpc_call());
    }

    #[test]
    fn test_request_serialization() {
        let req = Request {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({"protocolVersion": 1})),
            id: Some(1),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("initialize"));
        assert!(json.contains("protocolVersion"));
    }

    #[test]
    fn test_notification_has_no_id() {
        let req = Request {
            jsonrpc: "2.0".to_string(),
            method: "session/update".to_string(),
            params: Some(serde_json::json!({"status": "active"})),
            id: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn test_response_serialization() {
        let resp = Response {
            jsonrpc: "2.0".to_string(),
            result: Some(serde_json::json!({"capabilities": {}})),
            error: None,
            id: Some(1),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("capabilities"));
        assert!(!json.contains("error"));
    }

    #[test]
    fn test_rpc_error_display() {
        let err = RpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        };
        assert_eq!(format!("{err}"), "RPC error -32600: Invalid Request");
    }

    #[test]
    fn test_incoming_into_response() {
        let msg: IncomingMessage =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":42,"result":{"data":"hello"}}"#).unwrap();
        assert!(msg.is_response());

        let resp = msg.into_response();
        assert_eq!(resp.id, Some(42));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }
}
