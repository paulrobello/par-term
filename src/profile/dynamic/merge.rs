//! Profile merge logic for dynamic profile sources.
//!
//! Merges remotely fetched profiles into the local `ProfileManager`, applying
//! the configured conflict resolution strategy.

use par_term_config::ConflictResolution;
use std::time::SystemTime;

/// Merge dynamic profiles into a ProfileManager.
///
/// Steps:
/// 1. Remove existing dynamic profiles from this URL
/// 2. For each remote profile, check for name conflicts with local profiles
/// 3. Apply conflict resolution strategy
/// 4. Mark merged profiles with Dynamic source
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
