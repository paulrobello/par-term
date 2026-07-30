//! Crash-safe atomic file writes — re-exported from `par-term-config`.
//!
//! The implementation lives in [`par_term_config::atomic_save`]. It had to move
//! out of the root crate because `par-term-config`, `par-term-settings-ui` and
//! `par-term-update` all write files that must be atomic and none of them can
//! depend on the root crate; a Layer-1 home is the only one all four can reach.
//! Duplicating a security-sensitive write path into a second crate was the
//! alternative and was rejected.
//!
//! This module stays so that existing `crate::atomic_save::…` call sites keep
//! working. See the upstream module for the durability and permission
//! contracts, including when to prefer [`save_string_atomic_preserving_mode`]
//! over the `0o600`-forcing variants.

pub use par_term_config::atomic_save::{
    save_bytes_atomic, save_bytes_atomic_preserving_mode, save_string_atomic,
    save_string_atomic_preserving_mode, save_yaml_atomic,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Referenced by name from `src/profile/storage.rs`, and the one case worth
    /// re-running through the re-export: that a failed save leaves the previous
    /// file byte-for-byte intact. The rest of the contract is covered in
    /// `par_term_config::atomic_save`.
    #[test]
    fn failed_save_leaves_the_previous_file_intact() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("data.yaml");

        save_string_atomic(&path, "good data").expect("initial save");

        // A directory in the way makes the rename fail after the payload has
        // already been written and fsynced.
        let blocked = temp.path().join("blocked");
        fs::create_dir(&blocked).expect("blocking directory");
        assert!(save_string_atomic(&blocked, "payload").is_err());

        assert_eq!(fs::read_to_string(&path).expect("read"), "good data");
    }
}
