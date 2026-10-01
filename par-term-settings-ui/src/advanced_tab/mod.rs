//! Advanced, tmux, logging, update, file-transfer, and security sections.
//! Placement is set in [`crate::layout`].
//!
//! | File | Contents |
//! |------|----------|
//! | `import_export.rs` | Import/Export section + `merge_config` helper |
//! | `tmux.rs` | tmux Integration section (Sessions › tmux) |
//! | `logging.rs` | Output Recording section |
//! | `system.rs` | Screenshots, Updates, File Transfers, Debug Logging, Security sections |

pub(crate) mod import_export;
pub(crate) mod logging;
mod state;
pub(crate) mod system;
pub(crate) mod tmux;

pub use state::AdvancedTabState;

// Re-export merge_config — it is part of the public API of advanced_tab.
pub use import_export::merge_config;
