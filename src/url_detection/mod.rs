// URL and file path detection and handling utilities
//
// # Error Handling Convention
//
// Public functions in this module return `Result<(), String>` (simple string
// errors for UI display) rather than `anyhow::Error`. New helper functions
// added to this module should follow the same `Result<T, String>` pattern so
// callers can surface the error message directly to the user without conversion.

/// Core data types for detected items and position queries.
pub mod state;

/// Regex-based URL/path detection and OSC 8 hyperlink extraction.
pub mod detector;

/// URL and file opening/action utilities.
pub mod render;

// Re-export the public API so call-sites are unchanged.
pub use detector::{detect_file_paths_in_line, detect_osc8_hyperlinks, detect_urls_in_line};
pub use render::{ensure_url_scheme, expand_link_handler, open_file_in_editor, open_url};
pub use state::{DetectedItemType, DetectedUrl, find_url_at_position};
// shell_escape is pub(crate) for test access via `use super::*`
#[allow(unused_imports)]
pub(crate) use render::shell_escape;

#[cfg(test)]
mod tests;
