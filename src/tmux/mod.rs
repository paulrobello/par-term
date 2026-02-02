//! tmux Control Mode Integration
//!
//! This module provides integration with tmux's control mode (-CC flag),
//! allowing par-term to:
//! - Attach to existing tmux sessions
//! - Display tmux panes natively using the split pane system
//! - Send input through tmux control protocol
//! - Receive output and notifications
//!
//! The integration uses tmux control mode which provides a machine-readable
//! interface for controlling tmux sessions.
//!
//! ## Architecture
//!
//! - `session.rs`: TmuxSession lifecycle and state management
//! - `commands.rs`: Command builders for tmux control protocol
//! - `sync.rs`: Bidirectional state synchronization
//! - `types.rs`: Core data types (TmuxWindow, TmuxPane, etc.)
//!
//! ## Control Mode Protocol
//!
//! tmux control mode uses line-based commands:
//! - Commands start with the command name
//! - Notifications from tmux start with `%`
//! - Output blocks are delimited by `%begin` and `%end`
//!
//! The core library (par-term-emu-core-rust) provides the control mode parser.

mod commands;
pub mod parser_bridge;
pub mod prefix;
mod session;
mod sync;
mod types;

pub use commands::TmuxCommand;
pub use parser_bridge::ParserBridge;
pub use prefix::{PrefixKey, PrefixState, translate_command_key};
pub use session::{
    GatewayState, SessionState, TmuxNotification, TmuxSession, escape_keys_for_tmux,
};
pub use sync::{SyncAction, TmuxSync};
pub use types::{
    LayoutNode, TmuxLayout, TmuxPane, TmuxPaneId, TmuxSessionInfo, TmuxWindow, TmuxWindowId,
};
