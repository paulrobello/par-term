//! Log output renderer with level coloring, timestamp dimming, error
//! highlighting, stack trace folding, and JSON-in-log expansion.
//!
//! Parses each line of log output to extract timestamps, log levels,
//! source info, message text, and embedded JSON payloads, then renders
//! with appropriate styling:
//!
//! - **Log level coloring**: TRACE/DEBUG dim, INFO green, WARN yellow, ERROR/FATAL red
//! - **Timestamp dimming**: timestamps rendered but visually de-emphasized
//! - **Error highlighting**: ERROR/FATAL lines bold/bright
//! - **Stack trace folding**: consecutive indented lines after ERROR collapsed
//! - **JSON-in-log expansion**: embedded JSON payloads detected and highlighted

mod level_parser;
#[cfg(test)]
mod tests;

pub use level_parser::{LogLevel, LogRenderer, LogRendererConfig, register_log_renderer};
