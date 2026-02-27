//! Profile management modal UI using egui
//!
//! Provides a modal dialog for creating, editing, and managing profiles.

mod dialogs;
mod edit_view;
mod list_view;
mod state;

// Re-export the public API that external code expects
pub use state::{ProfileModalAction, ProfileModalUI};
