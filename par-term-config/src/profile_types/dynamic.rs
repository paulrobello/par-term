//! Runtime profile source tracking.
//!
//! Tracks where a profile was loaded from. This information is not persisted
//! to disk — it exists only in memory while the application is running.

/// Tracks where a profile came from (runtime-only, not persisted)
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ProfileSource {
    #[default]
    Local,
    Dynamic {
        url: String,
        last_fetched: Option<std::time::SystemTime>,
    },
}

impl ProfileSource {
    /// Returns true if this profile was fetched from a remote source
    pub fn is_dynamic(&self) -> bool {
        matches!(self, ProfileSource::Dynamic { .. })
    }
}
