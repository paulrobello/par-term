//! Profile management for terminal session configurations
//!
//! This module provides an iTerm2-style profile system that allows users to save
//! terminal session configurations including:
//! - Working directory for the session
//! - Custom command with arguments
//! - Custom tab name
//!
//! Profiles are stored in `~/.config/par-term/profiles.yaml`.

pub mod dynamic;
pub mod storage;
pub mod types;

pub use dynamic::{
    CacheMeta, ConflictResolution, DynamicProfileSource, FetchResult, cache_dir, fetch_profiles,
    merge_dynamic_profiles, read_cache, url_to_cache_filename, write_cache,
};
pub use types::{Profile, ProfileId, ProfileManager, ProfileSource};
