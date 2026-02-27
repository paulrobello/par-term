//! tmux notification handling — control-mode event routing.
//!
//! Handles all events received from the tmux control-mode parser:
//! session-changed, window-add/close/rename, layout-change, output,
//! pane-focus, session-ended, pause/continue, and sync-action dispatch.
//!
//! ## Gateway Mode
//!
//! Gateway mode writes `tmux -CC` commands to the existing terminal's PTY
//! instead of spawning a separate process. This is the iTerm2 approach and
//! provides reliable tmux integration.
//!
//! The flow is:
//! 1. User selects "Create Session" in picker
//! 2. We write `tmux -CC new-session -s name\n` to the active tab's PTY
//! 3. Enable tmux control mode parsing in the terminal
//! 4. Receive notifications via `%session-changed`, `%output`, etc.
//! 5. Route input via `send-keys` commands back to the same PTY
//!
//! ## Sub-modules
//!
//! - `polling`:        `check_tmux_notifications` — drains, converts and dispatches all events.
//! - `session`:        Session lifecycle (started, renamed, ended) and window-title sync.
//! - `window`:         Window add, close, rename handlers.
//! - `layout`:         Layout-change handler with incremental pane-tree reconciliation.
//! - `layout_new_tab`: Helper that creates a new tab when a layout arrives with no mapping.
//! - `output`:         Output routing to native pane terminals.
//! - `flow_control`:   Pane focus, error, pause/continue, and sync-action dispatch.

mod flow_control;
mod layout;
mod layout_new_tab;
mod output;
mod polling;
mod session;
mod window;
