//! ACP agent connection and runtime state for a window.
//!
//! Groups the fields that manage the ACP agent lifecycle: the async channel,
//! the agent handle, the JSON-RPC client, pending send queue, error recovery
//! counters, and the list of available agent configs.

use par_term_acp::{Agent, AgentConfig, AgentMessage};
use std::sync::Arc;
use tokio::sync::mpsc;

/// ACP agent connection and runtime state.
///
/// # Mutex Strategy
///
/// `agent` uses `tokio::sync::Mutex` because it is accessed from multiple spawned async
/// tasks (prompt sender, tool-call responder). All accesses must use `.lock().await`.
/// Do not attempt to lock `agent` from the sync winit event loop — use the mpsc channel
/// (`agent_tx`) to send messages instead.
pub(crate) struct AgentState {
    /// ACP agent message receiver
    pub(crate) agent_rx: Option<mpsc::UnboundedReceiver<AgentMessage>>,
    /// ACP agent message sender (kept to signal prompt completion)
    pub(crate) agent_tx: Option<mpsc::UnboundedSender<AgentMessage>>,
    /// ACP agent handle.
    ///
    /// Uses `tokio::sync::Mutex`; always access with `.lock().await` from async tasks.
    pub(crate) agent: Option<Arc<tokio::sync::Mutex<Agent>>>,
    /// ACP JSON-RPC client for sending responses without locking the agent.
    /// Stored separately to avoid deadlocks: `send_prompt` holds the agent lock
    /// while waiting for the prompt response, but the agent's tool calls
    /// need us to respond via this same client.
    pub(crate) agent_client: Option<Arc<par_term_acp::JsonRpcClient>>,
    /// Handles for queued send tasks (waiting on agent lock).
    /// Used to abort queued sends when the user cancels a pending message.
    pub(crate) pending_send_handles: std::collections::VecDeque<tokio::task::JoinHandle<()>>,
    /// Tracks whether the current prompt encountered a recoverable local
    /// backend tool failure or malformed inline XML-style tool markup.
    pub(crate) agent_skill_failure_detected: bool,
    /// Bounded automatic recovery retries after recoverable ACP tool failures.
    pub(crate) agent_skill_recovery_attempts: u8,
    /// One-shot transcript replay prompt injected into the next user prompt
    /// after reconnecting/switching agents.
    pub(crate) pending_agent_context_replay: Option<String>,
    /// Timestamp of the last command auto-context sent to the agent.
    pub(crate) last_auto_context_sent_at: Option<std::time::Instant>,
    /// Available agent configs
    pub(crate) available_agents: Vec<AgentConfig>,
}

impl AgentState {
    pub(crate) fn new(available_agents: Vec<AgentConfig>) -> Self {
        Self {
            agent_rx: None,
            agent_tx: None,
            agent: None,
            agent_client: None,
            pending_send_handles: std::collections::VecDeque::new(),
            agent_skill_failure_detected: false,
            agent_skill_recovery_attempts: 0,
            pending_agent_context_replay: None,
            last_auto_context_sent_at: None,
            available_agents,
        }
    }

    /// Drain all currently-available messages from `agent_rx` into a Vec.
    ///
    /// This avoids a double-borrow: callers can hold a `&mut self.agent_state`
    /// borrow only long enough to drain, then process the returned messages
    /// against the full `WindowState` without any borrow conflict.
    pub(crate) fn drain_messages(&mut self) -> Vec<AgentMessage> {
        let mut messages = Vec::new();
        if let Some(rx) = &mut self.agent_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }
        messages
    }
}
