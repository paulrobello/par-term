//! tmux notification handling and gateway session management.
//!
//! ## Sub-modules
//!
//! - `notifications`: `check_tmux_notifications` and all notification event handlers
//!   (session-changed, window-add/close/rename, layout-change, output, pane-focus,
//!   session-ended, pause/continue, sync-action dispatch).
//! - `gateway`: Gateway session lifecycle (initiate, attach, disconnect), input routing
//!   (send_input_via_tmux, paste_via_tmux, prefix key), pane operations
//!   (split/close), clipboard/resize sync, and profile auto-application.

mod gateway;
mod gateway_input;
mod notifications;
