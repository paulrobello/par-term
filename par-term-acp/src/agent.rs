//! Agent lifecycle manager for ACP (Agent Communication Protocol).
//!
//! Manages spawning an agent subprocess, performing the ACP handshake,
//! and spawning the background message-routing task.  Incoming message routing
//! has been extracted to [`super::message_handler`] to keep this module focused
//! on lifecycle concerns only.
//!
//! # Module layout
//!
//! - [`message_handler`](super::message_handler) — Background task routing incoming
//!   JSON-RPC messages to the UI channel.
//! - [`fs_tools`](super::fs_tools) — `fs/read_text_file`, `fs/write_text_file`,
//!   `fs/list_directory`, `fs/find` RPC handlers.
//! - [`permissions`](super::permissions) — Permission request dispatch, `SafePaths`,
//!   and the `is_safe_write_path` helper.
//! - [`session`](super::session) — Session-new parameter building helpers.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::Value;
use tokio::process::Command;
use tokio::sync::mpsc;

use super::agents::{AgentConfig, resolve_binary_in_path, resolve_shell_path};
use super::jsonrpc::JsonRpcClient;
use super::message_handler::handle_incoming_messages;
use super::permissions::SafePaths;
use super::protocol::{
    ClientCapabilities, ClientInfo, ContentBlock, InitializeParams, PermissionOption,
    PermissionOutcome, RequestPermissionResponse, SessionNewParams, SessionPromptParams,
    SessionResult, SessionUpdate,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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
    /// The agent has started processing a prompt (lock acquired, about to send).
    PromptStarted,
    /// The agent wants to update config settings.
    ConfigUpdate {
        updates: std::collections::HashMap<String, serde_json::Value>,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// The ACP client is ready — carry the `Arc<JsonRpcClient>` so the UI
    /// can send responses without locking the agent mutex.
    ClientReady(Arc<JsonRpcClient>),
    /// A tool call was automatically approved (for UI feedback).
    AutoApproved(String),
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
    /// Whether to automatically approve permission requests (shared with message handler).
    pub auto_approve: Arc<AtomicBool>,
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
            auto_approve: Arc::new(AtomicBool::new(false)),
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
        let run_command_template = self
            .config
            .run_command_for_platform()
            .ok_or("No run command for current platform")?
            .to_string();

        self.set_status(AgentStatus::Connecting);

        // Resolve the full PATH from the user's interactive login shell.
        //
        // When par-term is launched as a macOS app bundle (Finder/Dock/
        // Spotlight) the process inherits a minimal environment — tools
        // installed via nvm, homebrew, etc. won't be in PATH.  We spawn a
        // quick `$SHELL -lic 'printf "%s" "$PATH"'` to discover the PATH
        // the user would have in an interactive terminal, then pass that to
        // the agent child process.  This also covers shebangs like
        // `#!/usr/bin/env node` that need the runtime binary in PATH.
        let shell_path = resolve_shell_path();
        let run_command = if resolve_binary_in_path(&run_command_template).is_none() {
            // Binary not in process PATH — try resolving with shell PATH.
            if let Some(ref sp) = shell_path {
                let mut tokens = run_command_template.split_whitespace();
                if let Some(binary) = tokens.next() {
                    if let Some(abs) = super::agents::resolve_binary_in_path_str(binary, sp) {
                        log::info!("ACP: resolved '{binary}' to '{}'", abs.display());
                        let rest: String = tokens.collect::<Vec<_>>().join(" ");
                        if rest.is_empty() {
                            abs.to_string_lossy().to_string()
                        } else {
                            format!("{} {rest}", abs.to_string_lossy())
                        }
                    } else {
                        run_command_template.clone()
                    }
                } else {
                    run_command_template.clone()
                }
            } else {
                run_command_template.clone()
            }
        } else {
            run_command_template.clone()
        };

        // Spawn via login shell.  We intentionally do NOT use interactive
        // mode (-i) because it causes the shell to emit terminal control
        // sequences (e.g. [?1034h) to stdout, which corrupts the JSON-RPC
        // stream.  Instead we pass the resolved shell PATH as an env var so
        // the child has access to nvm, homebrew, etc.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        log::info!(
            "ACP: spawning agent '{}' via {shell} -lc '{run_command}' in cwd={cwd}",
            self.config.identity,
        );
        let mut cmd = Command::new(&shell);
        cmd.arg("-lc")
            .arg(&run_command)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // If we resolved a richer PATH from the shell, inject it so that
        // shebangs (#!/usr/bin/env node) and other runtime deps are found.
        if let Some(ref sp) = shell_path {
            cmd.env("PATH", sp);
        }
        cmd.envs(&self.config.env);

        // Ensure the agent doesn't think it's running inside another Claude
        // Code session (which would block session creation).
        cmd.env_remove("CLAUDECODE");

        let mut child = match cmd.spawn() {
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
        // Use helpers from `session` module to build the MCP server descriptor
        // and optional Claude-wrapper metadata.
        let mcp_server = super::session::build_mcp_server_descriptor(
            &self.safe_paths.config_dir,
            &self.config,
            &self.mcp_server_bin,
        );
        let session_meta =
            super::session::build_claude_session_meta(&self.config, &run_command_template);

        let session_params = SessionNewParams {
            cwd: cwd.to_string(),
            mcp_servers: Some(vec![mcp_server]),
            meta: session_meta,
        };
        log::info!(
            "ACP: sending session/new (cwd={cwd}, mcp_server_bin={})",
            self.mcp_server_bin.display()
        );
        // Session creation can take a while — the agent may need to start MCP
        // servers, load CLAUDE.md, and initialize its workspace.
        const SESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
        let session_response = match tokio::time::timeout(
            SESSION_TIMEOUT,
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
        let auto_approve = Arc::clone(&self.auto_approve);
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::Ordering;
    use std::time::{SystemTime, UNIX_EPOCH};

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
            env: HashMap::new(),
            install_command: None,
            actions: HashMap::new(),
            connector_installed: false,
        }
    }

    fn make_safe_paths() -> SafePaths {
        let base = std::env::temp_dir().join(format!(
            "par-term-acp-agent-tests-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        let config_dir = base.join("config");
        let shaders_dir = base.join("shaders");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::create_dir_all(&shaders_dir).expect("create shaders dir");

        SafePaths {
            config_dir,
            shaders_dir,
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
        assert!(!agent.auto_approve.load(Ordering::Relaxed));
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
}
