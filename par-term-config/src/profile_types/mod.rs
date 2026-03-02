//! Profile types and manager for terminal session configurations.
//!
//! This module provides profile types that can be used by the settings UI
//! and other configuration-dependent components.
//!
//! ## Sub-modules
//!
//! - [`dynamic`]: Runtime `ProfileSource` enum — tracks where a profile was loaded from
//! - [`profile`]: Core `Profile` struct and its builder/impl methods
//! - [`matchers`]: `ProfileManager` — collection management and glob-pattern matching

pub mod dynamic;
pub mod matchers;
pub mod profile;

// Re-export everything that was previously public from the flat profile_types.rs
// so that all external call sites continue to compile without any changes.
pub use dynamic::ProfileSource;
pub use matchers::ProfileManager;
pub use profile::{Profile, ProfileId};
