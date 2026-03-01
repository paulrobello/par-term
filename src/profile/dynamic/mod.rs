//! Dynamic profile source configuration, fetch, cache, merge, and manager.
//!
//! This module handles all aspects of remote dynamic profiles:
//!
//! - **Cache** (`cache`): SHA-256-keyed filesystem cache for fetched profiles
//! - **Fetch** (`fetch`): HTTPS-only HTTP fetch with timeout and size limits
//! - **Merge** (`merge`): Merge remote profiles into the local `ProfileManager`
//! - **Manager** (`manager`): Background tokio tasks with mpsc channel updates

mod cache;
mod fetch;
mod manager;
mod merge;
#[cfg(test)]
mod tests;

// Re-export configuration types from par-term-config
pub use par_term_config::{ConflictResolution, DynamicProfileSource};

pub use cache::{CacheMeta, cache_dir, read_cache, url_to_cache_filename, write_cache};
pub use fetch::{FetchResult, fetch_profiles};
pub use manager::{DynamicProfileManager, DynamicProfileUpdate, SourceStatus};
pub use merge::merge_dynamic_profiles;
