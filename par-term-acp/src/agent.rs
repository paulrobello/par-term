//! Agent lifecycle manager for ACP (Agent Communication Protocol).
//!
//! Manages spawning an agent subprocess, performing the ACP handshake,
//! routing incoming messages to the UI, and handling permission/file-read
//! requests from the agent.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::agents::AgentConfig;
use super::jsonrpc::{JsonRpcClient, RpcError};
use super::protocol::{
    ClientCapabilities, ClientInfo, ConfigUpdateParams, ContentBlock, FsFindParams,
    FsListDirectoryParams, FsReadParams, FsWriteParams, InitializeParams, PermissionOption,
    PermissionOutcome, RequestPermissionParams, RequestPermissionResponse, SessionNewParams,
    SessionPromptParams, SessionResult, SessionUpdate, SessionUpdateParams,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Directories considered safe for agent writes (auto-approved).
#[derive(Debug, Clone)]
pub struct SafePaths {
    /// Directory for par-term configuration files.
    pub config_dir: PathBuf,
    /// Directory for user shader files.
    pub shaders_dir: PathBuf,
}

/// Current connection status of an agent.
#[derive(Debug, Clone, PartialEq)]
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
    /// The agent finished processing a prompt (flush pending text).
    PromptComplete,
    /// The agent wants to update config settings.
    ConfigUpdate {
        updates: std::collections::HashMap<String, serde_json::Value>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// The ACP client is ready — carry the `Arc<JsonRpcClient>` so the UI
    /// can send responses without locking the agent mutex.
    ClientReady(Arc<JsonRpcClient>),
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
    pub client: Option<Arc<JsonRpcClient>>,
    /// Channel to send messages to the UI.
    ui_tx: mpsc::UnboundedSender<AgentMessage>,
    /// Whether to automatically approve permission requests.
    pub auto_approve: bool,
    /// Paths considered safe for auto-approving writes.
    safe_paths: SafePaths,
    /// Path to the binary to use for MCP server (par-term executable).
    mcp_server_bin: PathBuf,
}

impl Agent {
    /// Create a new agent manager in the [`AgentStatus::Disconnected`] state.
    ///
    /// # Arguments
    /// * `config` - The agent configuration from TOML discovery.
    /// * `ui_tx` - Channel to send messages to the UI layer.
    /// * `safe_paths` - Directories considered safe for agent writes.
    /// * `mcp_server_bin` - Path to the par-term binary for MCP server.
    pub fn new(
        config: AgentConfig,
        ui_tx: mpsc::UnboundedSender<AgentMessage>,
        safe_paths: SafePaths,
        mcp_server_bin: PathBuf,
    ) -> Self {
        Self {
            config,
            status: AgentStatus::Disconnected,
            session_id: None,
            child: None,
            client: None,
            ui_tx,
            auto_approve: false,
            safe_paths,
            mcp_server_bin,
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
        capabilities: ClientCapabilities,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Resolve the run command for the current platform.
        let run_command = self
            .config
            .run_command_for_platform()
            .ok_or("No run command for current platform")?
            .to_string();

        self.set_status(AgentStatus::Connecting);

        // Spawn via login shell so the user's full PATH is available
        // (nvm, homebrew, etc. are often configured in .bash_profile /
        // .zprofile / .profile). We intentionally do NOT use interactive
        // mode (-i) because it causes the shell to emit terminal control
        // sequences (e.g. [?1034h) to stdout, which corrupts the JSON-RPC
        // stream and causes handshake timeouts.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        log::info!(
            "ACP: spawning agent '{}' via {shell} -lc '{run_command}' in cwd={cwd}",
            self.config.identity,
        );
        let mut child = match Command::new(&shell)
            .arg("-lc")
            .arg(&run_command)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                let msg = format!("Failed to spawn agent: {e}");
                self.set_status(AgentStatus::Error(msg.clone()));
                return Err(msg.into());
            }
        };

        let stdin = child.stdin.take().ok_or("Failed to capture agent stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture agent stdout")?;

        // Log stderr in the background (matches Zed's pattern).
        if let Some(stderr) = child.stderr.take() {
            let identity = self.config.identity.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut reader = tokio::io::BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                log::warn!("ACP agent [{identity}] stderr: {trimmed}");
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Create the JSON-RPC client.
        let mut rpc_client = JsonRpcClient::new(stdin, stdout);
        let incoming_rx = rpc_client
            .take_incoming()
            .ok_or("Failed to take incoming channel")?;
        let client = Arc::new(rpc_client);

        // --- ACP Handshake (with timeout) ---
        const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

        // 1. Send `initialize` with par-term client info.
        let init_params = InitializeParams {
            protocol_version: 1,
            client_capabilities: capabilities,
            client_info: ClientInfo {
                name: "par-term".to_string(),
                title: "Par Term".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        log::info!("ACP: sending initialize request");
        let init_response = match tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            client.request("initialize", Some(serde_json::to_value(&init_params)?)),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                let msg = format!("Initialize request failed: {e}");
                self.set_status(AgentStatus::Error(msg.clone()));
                let _ = child.kill().await;
                return Err(msg.into());
            }
            Err(_) => {
                let msg =
                    "Agent handshake timed out (initialize). Is the agent installed?".to_string();
                self.set_status(AgentStatus::Error(msg.clone()));
                let _ = child.kill().await;
                return Err(msg.into());
            }
        };
        if let Some(err) = init_response.error {
            let msg = format!("Initialize failed: {err}");
            self.set_status(AgentStatus::Error(msg.clone()));
            let _ = child.kill().await;
            return Err(msg.into());
        }
        log::info!("ACP: initialize succeeded");

        // 2. Send `session/new` to create a session.
        //
        // Include an MCP server that exposes par-term's `config_update` tool
        // so the agent can modify settings without editing config.yaml directly.
        let config_update_path = self.safe_paths.config_dir.join(".config-update.json");
        let mcp_server = serde_json::json!({
            "name": "par-term-config",
            "command": self.mcp_server_bin.to_string_lossy(),
            "args": ["mcp-server"],
            "env": [{
                "name": "PAR_TERM_CONFIG_UPDATE_PATH",
                "value": config_update_path.to_string_lossy(),
            }],
        });
        let session_params = SessionNewParams {
            cwd: cwd.to_string(),
            mcp_servers: Some(vec![mcp_server]),
        };
        let session_response = match tokio::time::timeout(
            HANDSHAKE_TIMEOUT,
            client.request("session/new", Some(serde_json::to_value(&session_params)?)),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                let msg = format!("Session creation request failed: {e}");
                self.set_status(AgentStatus::Error(msg.clone()));
                let _ = child.kill().await;
                return Err(msg.into());
            }
            Err(_) => {
                let msg = "Agent handshake timed out (session/new)".to_string();
                self.set_status(AgentStatus::Error(msg.clone()));
                let _ = child.kill().await;
                return Err(msg.into());
            }
        };
        if let Some(err) = session_response.error {
            let msg = format!("Session creation failed: {err}");
            self.set_status(AgentStatus::Error(msg.clone()));
            let _ = child.kill().await;
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
        log::info!("ACP: connected, session_id={}", session_result.session_id);

        // 4. Spawn the message handler task.
        let ui_tx = self.ui_tx.clone();
        let handler_client = Arc::clone(&client);
        let auto_approve = self.auto_approve;
        let safe_paths = self.safe_paths.clone();
        tokio::spawn(async move {
            handle_incoming_messages(incoming_rx, handler_client, ui_tx, auto_approve, safe_paths)
                .await;
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

    /// Set the agent's session interaction mode.
    ///
    /// Valid modes: `"default"`, `"acceptEdits"`, `"bypassPermissions"`,
    /// `"dontAsk"`, `"plan"`.
    pub async fn set_mode(
        &self,
        mode_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        let response = client
            .request(
                "session/setMode",
                Some(serde_json::json!({
                    "sessionId": session_id,
                    "modeId": mode_id,
                })),
            )
            .await?;
        if let Some(err) = response.error {
            return Err(format!("setMode failed: {err}").into());
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
                outcome: "selected".to_string(),
                option_id: Some(option_id.to_string()),
            }
        };

        let result = RequestPermissionResponse { outcome };
        client
            .respond(request_id, Some(serde_json::to_value(&result)?), None)
            .await?;
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
// Helpers
// ---------------------------------------------------------------------------

/// Extract the file path from a tool_call JSON and check if it's in a safe
/// directory that can be auto-approved for writes.
///
/// Safe directories include `/tmp`, the par-term shaders directory, and the
/// par-term config directory (for `.config-update.json`).
fn is_safe_write_path(tool_call: &serde_json::Value, safe_paths: &SafePaths) -> bool {
    // Try to extract the path from various locations in the tool_call JSON.
    // Claude Code puts it in rawInput.file_path, rawInput.path, or the title
    // field as "Write /path/to/file".
    let path_str = tool_call
        .get("rawInput")
        .and_then(|ri| {
            ri.get("file_path")
                .or_else(|| ri.get("filePath"))
                .or_else(|| ri.get("path"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            // Fall back to extracting path from title: "Write /path/to/file"
            tool_call
                .get("title")
                .and_then(|v| v.as_str())
                .and_then(|t| t.split_whitespace().nth(1))
        });

    let Some(path_str) = path_str else {
        return false;
    };

    let path = std::path::Path::new(path_str);

    // Allow writes to /tmp or platform temp dir
    if path_str.starts_with("/tmp") || path_str.starts_with("/var/folders") {
        return true;
    }
    if let Ok(temp_dir) = std::env::var("TMPDIR")
        && path_str.starts_with(&temp_dir)
    {
        return true;
    }

    // Allow writes to par-term's shaders directory
    if path.starts_with(&safe_paths.shaders_dir) {
        return true;
    }

    // Allow writes to par-term's config directory (for .config-update.json etc.)
    if path.starts_with(&safe_paths.config_dir) {
        return true;
    }

    false
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
    safe_paths: SafePaths,
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

            log::info!("ACP RPC call: method={method} id={request_id}");

            match method {
                "session/request_permission" => {
                    if let Some(params) = &msg.params {
                        match serde_json::from_value::<RequestPermissionParams>(params.clone()) {
                            Ok(perm_params) => {
                                // Identify the tool from the tool_call JSON.
                                // Claude Code ACP puts the tool name in the "title"
                                // field as "ToolName /path/..." rather than in a
                                // dedicated "tool" or "name" field.
                                let tool_name = perm_params
                                    .tool_call
                                    .get("tool")
                                    .and_then(|v| v.as_str())
                                    .or_else(|| {
                                        perm_params.tool_call.get("name").and_then(|v| v.as_str())
                                    })
                                    .or_else(|| {
                                        perm_params
                                            .tool_call
                                            .get("toolName")
                                            .and_then(|v| v.as_str())
                                    })
                                    .or_else(|| {
                                        // Extract first word from "title" field
                                        // e.g. "Write /path/to/file" → "Write"
                                        perm_params
                                            .tool_call
                                            .get("title")
                                            .and_then(|v| v.as_str())
                                            .and_then(|t| t.split_whitespace().next())
                                    })
                                    .unwrap_or("");

                                log::info!(
                                    "ACP permission request: id={request_id} tool={tool_name} \
                                     tool_call={}",
                                    perm_params.tool_call
                                );

                                // Auto-approve read-only tools and config updates.
                                // Write/edit tools require approval unless writing
                                // to a temp directory (shaders dir, /tmp, etc.).
                                let is_safe_fs_tool = {
                                    let lower = tool_name.to_lowercase();
                                    let is_read_only = matches!(
                                        lower.as_str(),
                                        "read"
                                            | "read_file"
                                            | "readfile"
                                            | "readtextfile"
                                            | "glob"
                                            | "grep"
                                            | "find"
                                            | "list_directory"
                                            | "listdirectory"
                                            | "toolsearch"
                                            | "tool_search"
                                            | "notebookedit"
                                            | "notebook_edit"
                                            | "config"
                                            | "config_update"
                                            | "configupdate"
                                    ) || lower.contains("par-term-config");

                                    let is_write_tool = matches!(
                                        lower.as_str(),
                                        "write"
                                            | "write_file"
                                            | "writefile"
                                            | "writetextfile"
                                            | "edit"
                                    );

                                    if is_read_only {
                                        true
                                    } else if is_write_tool {
                                        // Only auto-approve writes to safe directories
                                        is_safe_write_path(&perm_params.tool_call, &safe_paths)
                                    } else {
                                        false
                                    }
                                };

                                // Log all options for debugging.
                                for (i, opt) in perm_params.options.iter().enumerate() {
                                    log::info!(
                                        "ACP permission option[{i}]: id={} name={} kind={:?}",
                                        opt.option_id,
                                        opt.name,
                                        opt.kind
                                    );
                                }

                                if auto_approve || is_safe_fs_tool {
                                    // Auto-approve: pick the first "allow" option, or just
                                    // the first option available.
                                    let option_id = perm_params
                                        .options
                                        .iter()
                                        .find(|o| {
                                            o.kind.as_deref() == Some("allow")
                                                || o.kind.as_deref() == Some("allowOnce")
                                                || o.name.to_lowercase().contains("allow")
                                        })
                                        .or_else(|| perm_params.options.first())
                                        .map(|o| o.option_id.clone());

                                    log::info!(
                                        "ACP: auto-approving tool={tool_name} id={request_id} \
                                         chosen_option={option_id:?}"
                                    );

                                    let outcome = RequestPermissionResponse {
                                        outcome: PermissionOutcome {
                                            outcome: "selected".to_string(),
                                            option_id,
                                        },
                                    };
                                    let response_json =
                                        serde_json::to_value(&outcome).unwrap_or_default();
                                    log::info!("ACP: sending permission response: {response_json}");
                                    if let Err(e) =
                                        client.respond(request_id, Some(response_json), None).await
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
                    let c = Arc::clone(&client);
                    match msg
                        .params
                        .as_ref()
                        .and_then(|p| serde_json::from_value::<FsReadParams>(p.clone()).ok())
                    {
                        Some(fs_params) => {
                            log::info!("ACP RPC: {method} path={}", fs_params.path);
                            // Spawn independently so handler continues processing other messages.
                            tokio::spawn(async move {
                                let path = fs_params.path.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    super::fs_ops::read_file_with_range(
                                        &fs_params.path,
                                        fs_params.line,
                                        fs_params.limit,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                                let (res, err) = match result {
                                    Ok(text) => {
                                        log::info!(
                                            "ACP fs/read OK: {} ({} bytes)",
                                            path,
                                            text.len()
                                        );
                                        (Some(serde_json::json!({ "content": text })), None)
                                    }
                                    Err(e) => {
                                        log::warn!("ACP fs/read FAIL: {} — {}", path, e);
                                        (
                                            None,
                                            Some(RpcError {
                                                code: -32000,
                                                message: e,
                                                data: None,
                                            }),
                                        )
                                    }
                                };
                                let _ = c.respond(request_id, res, err).await;
                            });
                        }
                        None => {
                            log::error!("ACP: failed to parse {method} params: {:?}", msg.params);
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
                "fs/write_text_file" | "fs/writeTextFile" => {
                    let c = Arc::clone(&client);
                    match msg
                        .params
                        .as_ref()
                        .and_then(|p| serde_json::from_value::<FsWriteParams>(p.clone()).ok())
                    {
                        Some(fs_params) => {
                            log::info!(
                                "ACP RPC: {method} path={} ({} bytes)",
                                fs_params.path,
                                fs_params.content.len()
                            );
                            tokio::spawn(async move {
                                let path = fs_params.path.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    super::fs_ops::write_file_safe(
                                        &fs_params.path,
                                        &fs_params.content,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                                let (res, err) = match result {
                                    Ok(()) => {
                                        log::info!("ACP fs/write OK: {}", path);
                                        (Some(serde_json::json!(null)), None)
                                    }
                                    Err(e) => {
                                        log::warn!("ACP fs/write FAIL: {} — {}", path, e);
                                        (
                                            None,
                                            Some(RpcError {
                                                code: -32000,
                                                message: e,
                                                data: None,
                                            }),
                                        )
                                    }
                                };
                                let _ = c.respond(request_id, res, err).await;
                            });
                        }
                        None => {
                            log::error!("ACP: failed to parse {method} params: {:?}", msg.params);
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
                "fs/list_directory" | "fs/listDirectory" => {
                    let c = Arc::clone(&client);
                    match msg.params.as_ref().and_then(|p| {
                        serde_json::from_value::<FsListDirectoryParams>(p.clone()).ok()
                    }) {
                        Some(fs_params) => {
                            log::info!("ACP RPC: {method} path={}", fs_params.path);
                            let pattern = fs_params.pattern.clone();
                            tokio::spawn(async move {
                                let path = fs_params.path.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    super::fs_ops::list_directory_entries(
                                        &fs_params.path,
                                        pattern.as_deref(),
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                                let (res, err) = match result {
                                    Ok(entries) => {
                                        log::info!(
                                            "ACP fs/list OK: {} ({} entries)",
                                            path,
                                            entries.len()
                                        );
                                        (Some(serde_json::json!({ "entries": entries })), None)
                                    }
                                    Err(e) => {
                                        log::warn!("ACP fs/list FAIL: {} — {}", path, e);
                                        (
                                            None,
                                            Some(RpcError {
                                                code: -32000,
                                                message: e,
                                                data: None,
                                            }),
                                        )
                                    }
                                };
                                let _ = c.respond(request_id, res, err).await;
                            });
                        }
                        None => {
                            log::error!("ACP: failed to parse {method} params: {:?}", msg.params);
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
                "fs/find" | "fs/glob" => {
                    let c = Arc::clone(&client);
                    match msg
                        .params
                        .as_ref()
                        .and_then(|p| serde_json::from_value::<FsFindParams>(p.clone()).ok())
                    {
                        Some(fs_params) => {
                            log::info!("ACP RPC: {method} path={}", fs_params.path);
                            tokio::spawn(async move {
                                let path = fs_params.path.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    super::fs_ops::find_files_recursive(
                                        &fs_params.path,
                                        &fs_params.pattern,
                                    )
                                })
                                .await
                                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                                let (res, err) = match result {
                                    Ok(files) => {
                                        log::info!(
                                            "ACP fs/find OK: {} ({} files)",
                                            path,
                                            files.len()
                                        );
                                        (Some(serde_json::json!({ "files": files })), None)
                                    }
                                    Err(e) => {
                                        log::warn!("ACP fs/find FAIL: {} — {}", path, e);
                                        (
                                            None,
                                            Some(RpcError {
                                                code: -32000,
                                                message: e,
                                                data: None,
                                            }),
                                        )
                                    }
                                };
                                let _ = c.respond(request_id, res, err).await;
                            });
                        }
                        None => {
                            log::error!("ACP: failed to parse {method} params: {:?}", msg.params);
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
                "config/update" | "config/updateConfig" => {
                    match msg
                        .params
                        .as_ref()
                        .and_then(|p| serde_json::from_value::<ConfigUpdateParams>(p.clone()).ok())
                    {
                        Some(params) => {
                            log::info!(
                                "ACP RPC: config/update keys={:?}",
                                params.updates.keys().collect::<Vec<_>>()
                            );
                            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                            let _ = ui_tx.send(AgentMessage::ConfigUpdate {
                                updates: params.updates,
                                reply: reply_tx,
                            });
                            let c = Arc::clone(&client);
                            tokio::spawn(async move {
                                match reply_rx.await {
                                    Ok(Ok(())) => {
                                        log::info!("ACP config/update OK");
                                        let _ = c
                                            .respond(
                                                request_id,
                                                Some(serde_json::json!({"success": true})),
                                                None,
                                            )
                                            .await;
                                    }
                                    Ok(Err(e)) => {
                                        log::warn!("ACP config/update FAIL: {e}");
                                        let _ = c
                                            .respond(
                                                request_id,
                                                None,
                                                Some(RpcError {
                                                    code: -32000,
                                                    message: e,
                                                    data: None,
                                                }),
                                            )
                                            .await;
                                    }
                                    Err(_) => {
                                        let _ = c
                                            .respond(
                                                request_id,
                                                None,
                                                Some(RpcError {
                                                    code: -32003,
                                                    message: "Config update handler dropped"
                                                        .to_string(),
                                                    data: None,
                                                }),
                                            )
                                            .await;
                                    }
                                }
                            });
                        }
                        None => {
                            log::error!("ACP: failed to parse {method} params: {:?}", msg.params);
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
            install_command: None,
            actions: HashMap::new(),
            connector_installed: false,
        }
    }

    fn make_safe_paths() -> SafePaths {
        SafePaths {
            config_dir: std::path::PathBuf::from("/tmp/test-config"),
            shaders_dir: std::path::PathBuf::from("/tmp/test-shaders"),
        }
    }

    #[test]
    fn test_agent_new_disconnected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(
            make_test_config(),
            tx,
            make_safe_paths(),
            std::path::PathBuf::from("par-term"),
        );
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
        let mut agent = Agent::new(
            make_test_config(),
            tx,
            make_safe_paths(),
            std::path::PathBuf::from("par-term"),
        );

        agent.set_status(AgentStatus::Connecting);
        assert!(matches!(agent.status, AgentStatus::Connecting));

        let msg = rx.try_recv().unwrap();
        assert!(matches!(
            msg,
            AgentMessage::StatusChanged(AgentStatus::Connecting)
        ));
    }

    #[tokio::test]
    async fn test_disconnect_clears_state() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut agent = Agent::new(
            make_test_config(),
            tx,
            make_safe_paths(),
            std::path::PathBuf::from("par-term"),
        );

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
        let agent = Agent::new(
            make_test_config(),
            tx,
            make_safe_paths(),
            std::path::PathBuf::from("par-term"),
        );

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
        let agent = Agent::new(
            make_test_config(),
            tx,
            make_safe_paths(),
            std::path::PathBuf::from("par-term"),
        );

        let result = agent.cancel().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_respond_permission_not_connected() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let agent = Agent::new(
            make_test_config(),
            tx,
            make_safe_paths(),
            std::path::PathBuf::from("par-term"),
        );

        let result = agent.respond_permission(1, "allow", false).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_write_path_tmp() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/tmp/test.glsl"},
            "title": "Write /tmp/test.glsl"
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_shaders_dir() {
        let safe_paths = SafePaths {
            shaders_dir: std::path::PathBuf::from("/tmp/test-shaders"),
            config_dir: std::path::PathBuf::from("/tmp/test-config"),
        };
        let path = safe_paths.shaders_dir.join("crt.glsl");
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": path.to_string_lossy()},
            "title": format!("Write {}", path.display())
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_config_dir() {
        let safe_paths = SafePaths {
            config_dir: std::path::PathBuf::from("/tmp/test-config"),
            shaders_dir: std::path::PathBuf::from("/tmp/test-shaders"),
        };
        let path = safe_paths.config_dir.join(".config-update.json");
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": path.to_string_lossy()},
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_unsafe_write_path_home() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/Users/someone/.bashrc"},
            "title": "Write /Users/someone/.bashrc"
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_unsafe_write_path_system() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/etc/passwd"},
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_from_title_fallback() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "title": "Write /tmp/shader.glsl"
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_no_path() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "title": "Write"
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }
}
