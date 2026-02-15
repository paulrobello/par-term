//! Agent lifecycle manager for ACP (Agent Communication Protocol).
//!
//! Manages spawning an agent subprocess, performing the ACP handshake,
//! routing incoming messages to the UI, and handling permission/file-read
//! requests from the agent.

use std::sync::Arc;

use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::agents::AgentConfig;
use super::jsonrpc::{JsonRpcClient, RpcError};
use super::protocol::{
    ClientCapabilities, ClientInfo, ContentBlock, FsCapabilities, FsReadParams,
    InitializeParams, PermissionOption, PermissionOutcome, RequestPermissionParams,
    RequestPermissionResponse, SessionNewParams, SessionPromptParams, SessionResult,
    SessionUpdate, SessionUpdateParams,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Current connection status of an agent.
#[derive(Debug, Clone)]
pub enum AgentStatus {
    /// Not connected to any agent process.
    Disconnected,
    /// Handshake in progress.
    Connecting,
    /// Successfully connected and session established.
    Connected,
    /// An error occurred during connection or communication.
    Error(String),
}

/// Messages sent from the agent manager to the UI layer.
#[derive(Debug)]
pub enum AgentMessage {
    /// The agent's connection status changed.
    StatusChanged(AgentStatus),
    /// A session update notification from the agent.
    SessionUpdate(SessionUpdate),
    /// The agent is requesting permission for a tool call.
    PermissionRequest {
        request_id: u64,
        tool_call: Value,
        options: Vec<PermissionOption>,
    },
    /// The agent is requesting to read a file from the host.
    FileReadRequest {
        request_id: u64,
        path: String,
        line: Option<u64>,
        limit: Option<u64>,
    },
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

/// Manages the lifecycle of an ACP agent subprocess.
pub struct Agent {
    /// The agent's configuration (from TOML discovery).
    pub config: AgentConfig,
    /// Current connection status.
    pub status: AgentStatus,
    /// The active session id, if connected.
    pub session_id: Option<String>,
    /// The spawned child process.
    child: Option<tokio::process::Child>,
    /// JSON-RPC client for communication with the agent.
    client: Option<Arc<JsonRpcClient>>,
    /// Channel to send messages to the UI.
    ui_tx: mpsc::UnboundedSender<AgentMessage>,
    /// Whether to automatically approve permission requests.
    pub auto_approve: bool,
}

impl Agent {
    /// Create a new agent manager in the [`AgentStatus::Disconnected`] state.
    pub fn new(config: AgentConfig, ui_tx: mpsc::UnboundedSender<AgentMessage>) -> Self {
        Self {
            config,
            status: AgentStatus::Disconnected,
            session_id: None,
            child: None,
            client: None,
            ui_tx,
            auto_approve: false,
        }
    }

    /// Spawn the agent subprocess, perform the ACP handshake, and establish a
    /// session.
    ///
    /// On success the agent transitions to [`AgentStatus::Connected`] and a
    /// background task is spawned to route incoming messages to the UI channel.
    pub async fn connect(
        &mut self,
        cwd: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Resolve the run command for the current platform.
        let run_command = self
            .config
            .run_command_for_platform()
            .ok_or("No run command for current platform")?
            .to_string();

        self.set_status(AgentStatus::Connecting);

        // Spawn the agent subprocess.
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&run_command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to capture agent stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture agent stdout")?;

        // Create the JSON-RPC client.
        let mut rpc_client = JsonRpcClient::new(stdin, stdout);
        let incoming_rx = rpc_client
            .take_incoming()
            .ok_or("Failed to take incoming channel")?;
        let client = Arc::new(rpc_client);

        // --- ACP Handshake ---

        // 1. Send `initialize` with par-term client info.
        let init_params = InitializeParams {
            protocol_version: 1,
            client_capabilities: ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: false,
                },
                terminal: false,
            },
            client_info: ClientInfo {
                name: "par-term".to_string(),
                title: "Par Term".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        let init_response = client
            .request("initialize", Some(serde_json::to_value(&init_params)?))
            .await?;
        if let Some(err) = init_response.error {
            let msg = format!("Initialize failed: {err}");
            self.set_status(AgentStatus::Error(msg.clone()));
            return Err(msg.into());
        }

        // 2. Send `session/new` to create a session.
        let session_params = SessionNewParams {
            cwd: cwd.to_string(),
            mcp_servers: None,
        };
        let session_response = client
            .request(
                "session/new",
                Some(serde_json::to_value(&session_params)?),
            )
            .await?;
        if let Some(err) = session_response.error {
            let msg = format!("Session creation failed: {err}");
            self.set_status(AgentStatus::Error(msg.clone()));
            return Err(msg.into());
        }

        let session_result: SessionResult = serde_json::from_value(
            session_response
                .result
                .ok_or("Missing result in session/new response")?,
        )?;

        // 3. Store state and transition to Connected.
        self.session_id = Some(session_result.session_id.clone());
        self.child = Some(child);
        self.client = Some(Arc::clone(&client));
        self.set_status(AgentStatus::Connected);

        // 4. Spawn the message handler task.
        let ui_tx = self.ui_tx.clone();
        let handler_client = Arc::clone(&client);
        let auto_approve = self.auto_approve;
        tokio::spawn(async move {
            handle_incoming_messages(incoming_rx, handler_client, ui_tx, auto_approve).await;
        });

        Ok(())
    }

    /// Disconnect from the agent, killing the subprocess and clearing state.
    pub async fn disconnect(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill().await;
        }
        self.child = None;
        self.client = None;
        self.session_id = None;
        self.set_status(AgentStatus::Disconnected);
    }

    /// Send a prompt to the agent's active session.
    pub async fn send_prompt(
        &self,
        content: Vec<ContentBlock>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        let params = SessionPromptParams {
            session_id: session_id.clone(),
            prompt: content,
        };
        let response = client
            .request("session/prompt", Some(serde_json::to_value(&params)?))
            .await?;
        if let Some(err) = response.error {
            return Err(format!("Prompt failed: {err}").into());
        }
        Ok(())
    }

    /// Cancel the current prompt execution.
    pub async fn cancel(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        client
            .notify(
                "session/cancel",
                Some(serde_json::json!({ "sessionId": session_id })),
            )
            .await?;
        Ok(())
    }

    /// Respond to a permission request from the agent.
    pub async fn respond_permission(
        &self,
        request_id: u64,
        option_id: &str,
        cancelled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        let outcome = if cancelled {
            PermissionOutcome {
                outcome: "cancelled".to_string(),
                option_id: None,
            }
        } else {
            PermissionOutcome {
                outcome: "allowed".to_string(),
                option_id: Some(option_id.to_string()),
            }
        };

        let result = RequestPermissionResponse { outcome };
        client
            .respond(
                request_id,
                Some(serde_json::to_value(&result)?),
                None,
            )
            .await?;
        Ok(())
    }

    /// Respond to a file read request from the agent.
    pub async fn respond_file_read(
        &self,
        request_id: u64,
        content: Result<String, String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        match content {
            Ok(text) => {
                client
                    .respond(
                        request_id,
                        Some(serde_json::json!({ "content": text })),
                        None,
                    )
                    .await?;
            }
            Err(err_msg) => {
                client
                    .respond(
                        request_id,
                        None,
                        Some(RpcError {
                            code: -32000,
                            message: err_msg,
                            data: None,
                        }),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// Update the agent's status and notify the UI.
    fn set_status(&mut self, status: AgentStatus) {
        self.status = status.clone();
        let _ = self.ui_tx.send(AgentMessage::StatusChanged(status));
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        // Best-effort kill of the child process.
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

// ---------------------------------------------------------------------------
// Message handler
// ---------------------------------------------------------------------------

/// Background task that reads incoming JSON-RPC messages from the agent and
/// routes them to the UI channel.
async fn handle_incoming_messages(
    mut incoming_rx: mpsc::UnboundedReceiver<super::jsonrpc::IncomingMessage>,
    client: Arc<JsonRpcClient>,
    ui_tx: mpsc::UnboundedSender<AgentMessage>,
    auto_approve: bool,
) {
    while let Some(msg) = incoming_rx.recv().await {
        let method = match msg.method.as_deref() {
            Some(m) => m,
            None => continue,
        };

        if msg.is_notification() {
            // Handle notifications.
            match method {
                "session/update" => {
                    if let Some(params) = &msg.params {
                        // Parse the SessionUpdateParams to extract the update field.
                        if let Ok(update_params) =
                            serde_json::from_value::<SessionUpdateParams>(params.clone())
                        {
                            let update = SessionUpdate::from_value(&update_params.update);
                            let _ = ui_tx.send(AgentMessage::SessionUpdate(update));
                        } else {
                            log::error!("Failed to parse session/update params");
                        }
                    }
                }
                _ => {
                    log::error!("Unknown notification method: {method}");
                }
            }
        } else if msg.is_rpc_call() {
            // Handle RPC calls from the agent.
            let request_id = match msg.id {
                Some(id) => id,
                None => continue,
            };

            match method {
                "session/request_permission" => {
                    if let Some(params) = &msg.params {
                        match serde_json::from_value::<RequestPermissionParams>(params.clone()) {
                            Ok(perm_params) => {
                                if auto_approve {
                                    // Auto-approve: pick the first "allow" option, or just
                                    // the first option available.
                                    let option_id = perm_params
                                        .options
                                        .iter()
                                        .find(|o| {
                                            o.kind.as_deref() == Some("allow")
                                        })
                                        .or_else(|| perm_params.options.first())
                                        .map(|o| o.option_id.clone());

                                    let outcome = RequestPermissionResponse {
                                        outcome: PermissionOutcome {
                                            outcome: "allowed".to_string(),
                                            option_id,
                                        },
                                    };
                                    if let Err(e) = client
                                        .respond(
                                            request_id,
                                            Some(
                                                serde_json::to_value(&outcome)
                                                    .unwrap_or_default(),
                                            ),
                                            None,
                                        )
                                        .await
                                    {
                                        log::error!("Failed to auto-approve permission: {e}");
                                    }
                                } else {
                                    let _ = ui_tx.send(AgentMessage::PermissionRequest {
                                        request_id,
                                        tool_call: perm_params.tool_call,
                                        options: perm_params.options,
                                    });
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse permission params: {e}");
                                let _ = client
                                    .respond(
                                        request_id,
                                        None,
                                        Some(RpcError {
                                            code: -32602,
                                            message: "Invalid params".to_string(),
                                            data: None,
                                        }),
                                    )
                                    .await;
                            }
                        }
                    }
                }
                "fs/read_text_file" | "fs/readTextFile" => {
                    if let Some(params) = &msg.params {
                        match serde_json::from_value::<FsReadParams>(params.clone()) {
                            Ok(fs_params) => {
                                let _ = ui_tx.send(AgentMessage::FileReadRequest {
                                    request_id,
                                    path: fs_params.path,
                                    line: fs_params.line,
                                    limit: fs_params.limit,
                                });
                            }
                            Err(e) => {
                                log::error!("Failed to parse fs/readTextFile params: {e}");
                                let _ = client
                                    .respond(
                                        request_id,
                                        None,
                                        Some(RpcError {
                                            code: -32602,
                                            message: "Invalid params".to_string(),
                                            data: None,
                                        }),
                                    )
                                    .await;
                            }
                        }
                    }
                }
                _ => {
                    log::error!("Unknown RPC call method: {method}");
                    let _ = client
                        .respond(
                            request_id,
                            None,
                            Some(RpcError {
                                code: -32601,
                                message: format!("Method not found: {method}"),
                                data: None,
                            }),
                        )
                        .await;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_config() -> AgentConfig {
        AgentConfig {
            identity: "test.agent".to_string(),
            name: "Test Agent".to_string(),
            short_name: "test".to_string(),
            protocol: "acp".to_string(),
            r#type: "coding".to_string(),
            active: Some(true),
            run_command: {
                let mut m = HashMap::new();
                m.insert("*".to_string(), "echo test".to_string());
                m
            },
            actions: HashMap::new(),
        }
    }

    #[test]
    fn test_agent_new_disconnected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(make_test_config(), tx);
        assert!(matches!(agent.status, AgentStatus::Disconnected));
        assert!(agent.session_id.is_none());
        assert!(agent.client.is_none());
        assert!(agent.child.is_none());
        assert!(!agent.auto_approve);
    }

    #[test]
    fn test_agent_status_variants() {
        let status = AgentStatus::Disconnected;
        assert!(matches!(status, AgentStatus::Disconnected));

        let status = AgentStatus::Connecting;
        assert!(matches!(status, AgentStatus::Connecting));

        let status = AgentStatus::Connected;
        assert!(matches!(status, AgentStatus::Connected));

        let status = AgentStatus::Error("test error".to_string());
        assert!(matches!(status, AgentStatus::Error(_)));
    }

    #[test]
    fn test_set_status_sends_message() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut agent = Agent::new(make_test_config(), tx);

        agent.set_status(AgentStatus::Connecting);
        assert!(matches!(agent.status, AgentStatus::Connecting));

        let msg = rx.try_recv().unwrap();
        assert!(matches!(msg, AgentMessage::StatusChanged(AgentStatus::Connecting)));
    }

    #[tokio::test]
    async fn test_disconnect_clears_state() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut agent = Agent::new(make_test_config(), tx);

        // Simulate some connected state.
        agent.session_id = Some("test-session".to_string());
        agent.status = AgentStatus::Connected;

        agent.disconnect().await;

        assert!(matches!(agent.status, AgentStatus::Disconnected));
        assert!(agent.session_id.is_none());
        assert!(agent.client.is_none());
        assert!(agent.child.is_none());
    }

    #[tokio::test]
    async fn test_send_prompt_not_connected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(make_test_config(), tx);

        let result = agent
            .send_prompt(vec![ContentBlock::Text {
                text: "Hello".to_string(),
            }])
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cancel_not_connected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(make_test_config(), tx);

        let result = agent.cancel().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_respond_permission_not_connected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(make_test_config(), tx);

        let result = agent.respond_permission(1, "allow", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_respond_file_read_not_connected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(make_test_config(), tx);

        let result = agent
            .respond_file_read(1, Ok("file content".to_string()))
            .await;
        assert!(result.is_err());
    }
}
