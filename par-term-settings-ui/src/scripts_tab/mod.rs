//! Observer scripts (Automation › Observer Scripts): external scripts that
//! receive terminal events via JSON protocol and can send commands back.
//! Placement is set in [`crate::layout`].
//!
//! | File | Contents |
//! |------|----------|
//! | `list.rs` | Script list section (status, controls, output viewer, panel viewer) |
//! | `editor.rs` | Script edit form (name, path, permissions, save/cancel) |

mod editor;
pub(crate) mod list;
mod state;

pub use state::ScriptsTabState;
