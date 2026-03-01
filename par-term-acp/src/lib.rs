//! par-term-acp: Agent Communication Protocol (ACP) implementation.
//!
//! This crate provides the core ACP protocol implementation for communicating
//! with AI coding agents (Claude Code, Codex CLI, Gemini CLI, etc.) via JSON-RPC.
//!
//! # Architecture
//!
//! The crate is organized into several modules:
//!
//! - [`agent`] - Agent lifecycle management (spawn, handshake, message routing dispatch)
//! - [`agents`] - Agent discovery and configuration loading
//! - [`message_handler`] - Background async task that routes incoming JSON-RPC messages to the UI
//! - [`protocol`] - ACP message types (initialize, session, permission, etc.)
//! - [`jsonrpc`] - JSON-RPC 2.0 client implementation
//! - [`fs_ops`] - Low-level filesystem operations (read, write, list, find)
//! - [`fs_tools`] - RPC handler functions for `fs/*` tool calls from the agent
//! - [`permissions`] - Permission request dispatch, auto-approval logic, `SafePaths`, and `is_safe_write_path`
//! - [`session`] - Session-new parameter builders (MCP server descriptor, Claude wrapper metadata)
//!
//! # Example
//!
//! ```ignore
//! use par_term_acp::{Agent, AgentConfig, AgentMessage, SafePaths, discover_agents};
//! use tokio::sync::mpsc;
//!
//! // Discover available agents
//! let agents = discover_agents(&config_dir);
//! let config = agents.into_iter().next().unwrap();
//!
//! // Create agent manager
//! let (tx, mut rx) = mpsc::unbounded_channel();
//! let safe_paths = SafePaths {
//!     config_dir: PathBuf::from("/path/to/config"),
//!     shaders_dir: PathBuf::from("/path/to/shaders"),
//! };
//! let mut agent = Agent::new(config, tx, safe_paths, PathBuf::from("par-term"));
//!
//! // Connect and handle messages
//! agent.connect("/working/dir", capabilities).await?;
//! while let Some(msg) = rx.recv().await {
//!     match msg {
//!         AgentMessage::SessionUpdate(update) => { /* handle */ }
//!         AgentMessage::PermissionRequest { .. } => { /* handle */ }
//!         _ => {}
//!     }
//! }
//! ```

pub mod agent;
pub mod agents;
pub mod fs_ops;
pub mod fs_tools;
pub mod harness;
pub mod jsonrpc;
pub mod message_handler;
pub mod permissions;
pub mod protocol;
pub mod session;

// Re-export the main public types at the crate root for convenience
pub use agent::{Agent, AgentMessage, AgentStatus};
pub use agents::{AgentConfig, discover_agents};
pub use jsonrpc::{IncomingMessage, JsonRpcClient, Request, Response, RpcError};
pub use permissions::SafePaths;
pub use protocol::{
    ClientCapabilities, ClientInfo, ContentBlock, FsCapabilities, FsFindParams,
    FsListDirectoryParams, FsReadParams, FsWriteParams, InitializeParams, InitializeResult,
    PermissionOption, PermissionOutcome, RequestPermissionParams, RequestPermissionResponse,
    SessionNewParams, SessionPromptParams, SessionResult, SessionUpdate, SessionUpdateParams,
    ToolCallInfo, ToolCallUpdateInfo,
};
