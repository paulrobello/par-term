//! Automation sections: triggers, coprocesses, and plugins. Observer
//! scripts are in `scripts_tab`, snippets in `snippets_tab`, custom actions
//! in `actions_tab`. Placement is set in [`crate::layout`].
//!
//! | File | Contents |
//! |------|----------|
//! | `triggers_section.rs` | Trigger list, edit form, action field rendering |
//! | `coprocesses_section.rs` | Coprocess list, edit form, output viewer |
//! | `plugins_section.rs` | Plugin list, trust surface, enable toggle, schema editor |
//! | `plugin_git_ui.rs` | Plugin git distribution: add from URL, update review, remove |

pub(crate) mod coprocesses_section;
mod plugin_git_ui;
pub(crate) mod plugins_section;
mod state;
pub(crate) mod triggers_section;

pub use state::AutomationTabState;
