//! Profiles tab sections. Placement is set in [`crate::layout`].
//!
//! | File | Contents |
//! |------|----------|
//! | `management.rs` | Profile management section + display options section |
//! | `dynamic_sources.rs` | Dynamic profile sources list, enable/disable, edit form |

pub(crate) mod dynamic_sources;
pub(crate) mod management;
mod state;

pub use state::ProfilesTabState;
