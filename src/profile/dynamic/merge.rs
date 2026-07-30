//! Profile merge logic for dynamic profile sources.
//!
//! Merges remotely fetched profiles into the local `ProfileManager`, applying
//! the configured conflict resolution strategy.

use par_term_config::ConflictResolution;
use std::time::SystemTime;

/// Returns true for a glob pattern that matches every possible value.
///
/// `ProfileManager::pattern_matches` treats a bare `*` as a wildcard, and `**`
/// degenerates to a `contains("")` test, so both match everything.
fn is_catch_all_pattern(pattern: &str) -> bool {
    let pattern = pattern.trim();
    pattern == "*" || pattern == "**"
}

/// Returns true for the "match everything and run a command" shape.
///
/// A dynamic profile source shipping `hostname_patterns: ["*"]` alongside a
/// `command` would fire that command on the first hostname the terminal
/// reports over OSC 7. A user can legitimately write such a profile locally;
/// a remote source cannot, so these are dropped at merge time rather than
/// relying on the confirmation dialog alone.
fn is_catch_all_command_profile(profile: &par_term_config::Profile) -> bool {
    let has_command = profile
        .command
        .as_deref()
        .is_some_and(|command| !command.trim().is_empty());

    has_command
        && profile
            .hostname_patterns
            .iter()
            .chain(profile.directory_patterns.iter())
            .chain(profile.tmux_session_patterns.iter())
            .any(|pattern| is_catch_all_pattern(pattern))
}

/// Merge dynamic profiles into a ProfileManager.
///
/// Steps:
/// 1. Remove existing dynamic profiles from this URL
/// 2. Reject remote profiles that pair a catch-all auto-switch pattern with a command
/// 3. For each remote profile, check for name conflicts with local profiles
/// 4. Apply conflict resolution strategy
/// 5. Mark merged profiles with Dynamic source
pub fn merge_dynamic_profiles(
    manager: &mut par_term_config::ProfileManager,
    remote_profiles: &[par_term_config::Profile],
    url: &str,
    conflict_resolution: &ConflictResolution,
) {
    // Remove existing dynamic profiles from this URL
    let to_remove: Vec<par_term_config::ProfileId> = manager
        .profiles_ordered()
        .iter()
        .filter(
            |p| matches!(&p.source, par_term_config::ProfileSource::Dynamic { url: u, .. } if u == url),
        )
        .map(|p| p.id)
        .collect();
    for id in &to_remove {
        manager.remove(id);
    }

    // Merge remote profiles
    let now = SystemTime::now();
    for remote in remote_profiles {
        if is_catch_all_command_profile(remote) {
            log::error!(
                "Rejecting dynamic profile '{}' from {}: it pairs a catch-all \
                 auto-switch pattern with a command, which would run that command \
                 on the first hostname or directory the terminal reports.",
                remote.name,
                url
            );
            crate::debug_error!(
                "DYNAMIC_PROFILE",
                "REJECTED catch-all command profile '{}' from {}",
                remote.name,
                url
            );
            continue;
        }

        let existing = manager.find_by_name(&remote.name);
        match (existing, conflict_resolution) {
            (Some(_), ConflictResolution::LocalWins) => {
                crate::debug_info!("DYNAMIC_PROFILE", "Skipping '{}' (local wins)", remote.name);
            }
            (Some(local), ConflictResolution::RemoteWins) => {
                let local_id = local.id;
                manager.remove(&local_id);
                let mut profile = remote.clone();
                profile.id = uuid::Uuid::new_v4();
                profile.source = par_term_config::ProfileSource::Dynamic {
                    url: url.to_string(),
                    last_fetched: Some(now),
                };
                manager.add(profile);
                crate::debug_info!(
                    "DYNAMIC_PROFILE",
                    "Remote '{}' overwrites local",
                    remote.name
                );
            }
            (None, _) => {
                let mut profile = remote.clone();
                profile.id = uuid::Uuid::new_v4();
                profile.source = par_term_config::ProfileSource::Dynamic {
                    url: url.to_string(),
                    last_fetched: Some(now),
                };
                manager.add(profile);
                crate::debug_info!("DYNAMIC_PROFILE", "Added remote '{}'", remote.name);
            }
        }
    }
}
