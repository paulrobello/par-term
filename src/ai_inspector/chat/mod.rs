//! Chat sub-system for the AI Inspector panel.
//!
//! Sub-modules:
//! - [`state`]     — `ChatState` struct: conversation history and streaming buffer
//! - [`text_utils`] — Text parsing utilities: code-block extraction, segment parsing
//! - [`types`]     — `ChatMessage` enum and `AGENT_SYSTEM_GUIDANCE` constant

mod state;
pub mod text_utils;
mod types;

#[cfg(test)]
mod tests;

// Re-export the public API so callers can use `chat::ChatState` etc.
pub use state::ChatState;
pub use text_utils::{
    TextSegment, extract_inline_config_update, extract_inline_tool_function_name,
    parse_text_segments,
};
pub use types::{AGENT_SYSTEM_GUIDANCE, ChatMessage};
