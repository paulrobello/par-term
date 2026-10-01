//! Terminal-behavior sections. Where each one is drawn is set in
//! [`crate::layout`].
//!
//! | File | Contents |
//! |------|----------|
//! | `behavior.rs` | Scrollback; Closing & Quitting (close confirmation, shell exit, reopen closed tab) |
//! | `unicode.rs` | Unicode section (version, ambiguous width, normalization, answerback) |
//! | `shell.rs` | Shell section (custom shell, args, login shell, startup directory) |
//! | `startup.rs` | Startup section (restore windows, initial text) |
//! | `search.rs` | Search, Command History, and Command Separator sections |
//! | `semantic_history.rs` | Semantic History section (link handler, file path detection, editor) |

pub(crate) mod behavior;
pub(crate) mod search;
pub(crate) mod semantic_history;
pub(crate) mod shell;
pub(crate) mod startup;
pub(crate) mod unicode;
