//! par-mux Control Mode Client
//!
//! Client for the core library's par-mux daemon, speaking genuine tmux
//! control mode over a local socket. Where `par-term-tmux` drives a real
//! `tmux -CC` gateway through a PTY, this crate connects to a daemon the
//! core owns, so there is no shell to launch, no gateway tab to hide, and
//! no binary to discover.
//!
//! The sync layer is NOT reimplemented here: `ParserBridge`, `TmuxSync`,
//! `SyncAction`, and the id types are imported from `par-term-tmux`
//! unmodified. That reuse is the compatibility claim — if par-mux emits
//! genuine control mode, the existing sync layer works against it as-is.
//!
//! ## Architecture
//!
//! - `client.rs`: `MuxSessionClient` — connection lifecycle, the
//!   notification → `ParserBridge` → `TmuxSync` pipeline, and the command
//!   forms par-term sends (key names from `escape_keys_for_tmux`, literal
//!   `-l`, hex `-H`, absolute `resize-pane -x/-y`, `refresh-client -C`)
//! - `resync.rs`: reattach queries over the command tier —
//!   list-sessions/windows/panes, create-or-attach, per-pane screen seeding
//! - `agents.rs`: the agent roster wire tier — the `list-agents` query and
//!   the `%agent-state-changed` push payload as one entry shape
//!
//! ## Feature gate
//!
//! The core's `mux` module exists only in core >= 0.50 (unpublished), so
//! everything here compiles only under this crate's `mux` feature. The
//! feature is declared empty in the committed manifest; local runs extend
//! it via `scripts/with-local-core.sh`, which forwards the core feature
//! and auto-reverts the manifest afterwards.

#![cfg(feature = "mux")]

mod agents;
mod client;
mod resync;

pub use agents::{AgentEntry, AgentSource};
pub use client::MuxSessionClient;
pub use resync::{AttachOutcome, SessionSummary, WindowSummary};
