//! Content boundary detection for the Content Prettifier framework.
//!
//! [`BoundaryDetector`] identifies where one content block ends and another begins
//! in the terminal output stream. It accumulates lines and emits [`ContentBlock`]
//! instances at natural boundaries such as OSC 133 command markers, blank-line
//! runs, or debounce timeouts.

mod detector;
#[cfg(test)]
mod tests;

pub use detector::{BoundaryConfig, BoundaryDetector, DetectionScope};
