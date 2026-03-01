//! Profile collection management and glob-pattern matching.
//!
//! Provides `ProfileManager`, which stores profiles, maintains display order,
//! resolves inheritance chains, and matches profiles against hostnames,
//! tmux session names, and directory paths using glob-style patterns.

use std::collections::HashMap;

use super::profile::{Profile, ProfileId};

/// Manages a collection of profiles
#[derive(Debug, Clone, Default)]
pub struct ProfileManager {
    /// All profiles indexed by ID
    profiles: HashMap<ProfileId, Profile>,

    /// Ordered list of profile IDs for display
    order: Vec<ProfileId>,
}

impl ProfileManager {
    /// Create a new empty profile manager
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Create a profile manager from a list of profiles
    pub fn from_profiles(profiles: Vec<Profile>) -> Self {
        let mut manager = Self::new();
        for profile in profiles {
            manager.add(profile);
        }
        manager.sort_by_order();
        manager
    }

    /// Add a profile to the manager
    pub fn add(&mut self, profile: Profile) {
        let id = profile.id;
        if !self.order.contains(&id) {
            self.order.push(id);
        }
        self.profiles.insert(id, profile);
    }

    /// Get a profile by ID
    pub fn get(&self, id: &ProfileId) -> Option<&Profile> {
        self.profiles.get(id)
    }

    /// Get a mutable reference to a profile by ID
    pub fn get_mut(&mut self, id: &ProfileId) -> Option<&mut Profile> {
        self.profiles.get_mut(id)
    }

    /// Update a profile (replaces if exists)
    pub fn update(&mut self, profile: Profile) {
        let id = profile.id;
        if self.profiles.contains_key(&id) {
            self.profiles.insert(id, profile);
        }
    }

    /// Remove a profile by ID
    pub fn remove(&mut self, id: &ProfileId) -> Option<Profile> {
        self.order.retain(|pid| pid != id);
        self.profiles.remove(id)
    }

    /// Get all profiles in display order
    pub fn profiles_ordered(&self) -> Vec<&Profile> {
        self.order
            .iter()
            .filter_map(|id| self.profiles.get(id))
            .collect()
    }

    /// Get all profiles as a vector (for serialization)
    pub fn to_vec(&self) -> Vec<Profile> {
        self.profiles_ordered().into_iter().cloned().collect()
    }

    /// Get the number of profiles
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Check if there are no profiles
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// Get an iterator over all profile IDs in order
    pub fn ids(&self) -> impl Iterator<Item = &ProfileId> {
        self.order.iter()
    }

    /// Move a profile earlier in the order (towards index 0)
    pub fn move_up(&mut self, id: &ProfileId) {
        if let Some(pos) = self.order.iter().position(|pid| pid == id)
            && pos > 0
        {
            self.order.swap(pos, pos - 1);
            self.update_orders();
        }
    }

    /// Move a profile later in the order (towards the end)
    pub fn move_down(&mut self, id: &ProfileId) {
        if let Some(pos) = self.order.iter().position(|pid| pid == id)
            && pos < self.order.len() - 1
        {
            self.order.swap(pos, pos + 1);
            self.update_orders();
        }
    }

    /// Sort profiles by their order field
    fn sort_by_order(&mut self) {
        self.order
            .sort_by_key(|id| self.profiles.get(id).map(|p| p.order).unwrap_or(usize::MAX));
    }

    /// Update the order field of all profiles to match their position
    fn update_orders(&mut self) {
        for (i, id) in self.order.iter().enumerate() {
            if let Some(profile) = self.profiles.get_mut(id) {
                profile.order = i;
            }
        }
    }

    /// Find a profile by name (case-insensitive)
    pub fn find_by_name(&self, name: &str) -> Option<&Profile> {
        let lower = name.to_lowercase();
        self.profiles
            .values()
            .find(|p| p.name.to_lowercase() == lower)
    }

    /// Find a profile by keyboard shortcut
    pub fn find_by_shortcut(&self, shortcut: &str) -> Option<&Profile> {
        let lower = shortcut.to_lowercase();
        self.profiles.values().find(|p| {
            p.keyboard_shortcut
                .as_ref()
                .is_some_and(|s| s.to_lowercase() == lower)
        })
    }

    /// Find all profiles with a specific tag (case-insensitive)
    pub fn find_by_tag(&self, tag: &str) -> Vec<&Profile> {
        let lower = tag.to_lowercase();
        self.profiles_ordered()
            .into_iter()
            .filter(|p| p.tags.iter().any(|t| t.to_lowercase() == lower))
            .collect()
    }

    /// Filter profiles by tag search query (matches partial tag names)
    pub fn filter_by_tags(&self, query: &str) -> Vec<&Profile> {
        if query.is_empty() {
            return self.profiles_ordered();
        }
        let lower = query.to_lowercase();
        self.profiles_ordered()
            .into_iter()
            .filter(|p| {
                p.tags.iter().any(|t| t.to_lowercase().contains(&lower))
                    || p.name.to_lowercase().contains(&lower)
            })
            .collect()
    }

    /// Get all unique tags across all profiles (sorted alphabetically)
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .profiles
            .values()
            .flat_map(|p| p.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Find profile matching a hostname pattern for automatic switching
    /// Uses glob-style pattern matching
    pub fn find_by_hostname(&self, hostname: &str) -> Option<&Profile> {
        let hostname_lower = hostname.to_lowercase();
        self.profiles_ordered().into_iter().find(|p| {
            p.hostname_patterns
                .iter()
                .any(|pattern| Self::pattern_matches(&hostname_lower, pattern))
        })
    }

    /// Find profile matching a tmux session name pattern for automatic switching
    /// Uses glob-style pattern matching
    pub fn find_by_tmux_session(&self, session_name: &str) -> Option<&Profile> {
        let session_lower = session_name.to_lowercase();
        self.profiles_ordered().into_iter().find(|p| {
            p.tmux_session_patterns
                .iter()
                .any(|pattern| Self::pattern_matches(&session_lower, pattern))
        })
    }

    /// Find profile matching a directory pattern for automatic switching based on CWD
    /// Uses glob-style pattern matching against the current working directory
    pub fn find_by_directory(&self, cwd: &str) -> Option<&Profile> {
        self.profiles_ordered().into_iter().find(|p| {
            p.directory_patterns
                .iter()
                .any(|pattern| Self::directory_pattern_matches(cwd, pattern))
        })
    }

    /// Expand `~` at the start of a pattern to the user's home directory.
    fn expand_tilde(pattern: &str) -> std::borrow::Cow<'_, str> {
        if let Some(rest) = pattern.strip_prefix('~')
            && let Some(home) = dirs::home_dir()
        {
            return std::borrow::Cow::Owned(format!("{}{}", home.display(), rest));
        }
        std::borrow::Cow::Borrowed(pattern)
    }

    /// Check if a directory path matches a glob-style pattern
    /// Unlike hostname matching, directory matching is case-sensitive on Unix
    /// and supports path-specific glob patterns.
    /// Supports `~` expansion in patterns (e.g., `~/projects/*`).
    fn directory_pattern_matches(path: &str, pattern: &str) -> bool {
        // Expand ~ to home directory in pattern
        let pattern = Self::expand_tilde(pattern);
        // Normalize trailing slashes for consistent matching
        let path = path.trim_end_matches('/');
        let pattern = pattern.trim_end_matches('/');

        if pattern == "*" {
            return true;
        }

        // Check for prefix match (pattern ends with *)
        if let Some(prefix) = pattern.strip_suffix('*') {
            return path.starts_with(prefix);
        }

        // Check for suffix match (pattern starts with *)
        if let Some(suffix) = pattern.strip_prefix('*') {
            return path.ends_with(suffix);
        }

        // Exact match
        path == pattern
    }

    /// Check if a string matches a glob-style pattern (case-insensitive)
    /// Supports: exact match, prefix match (pattern*), suffix match (*pattern),
    /// contains match (*pattern*), and wildcard (*)
    fn pattern_matches(value: &str, pattern: &str) -> bool {
        let value_lower = value.to_lowercase();
        let pattern_lower = pattern.to_lowercase();

        // Simple glob matching: * matches any characters
        if pattern_lower == "*" {
            return true;
        }

        // Check for prefix match (pattern ends with *)
        if let Some(prefix) = pattern_lower.strip_suffix('*')
            && value_lower.starts_with(prefix)
        {
            return true;
        }

        // Check for suffix match (pattern starts with *)
        if let Some(suffix) = pattern_lower.strip_prefix('*')
            && value_lower.ends_with(suffix)
        {
            return true;
        }

        // Check for contains match (*something*)
        if pattern_lower.starts_with('*')
            && pattern_lower.ends_with('*')
            && value_lower.contains(&pattern_lower[1..pattern_lower.len() - 1])
        {
            return true;
        }

        // Exact match
        value_lower == pattern_lower
    }

    /// Resolve a profile with inheritance - returns effective settings
    /// by merging parent profiles. Child values override parent values.
    pub fn resolve_profile(&self, id: &ProfileId) -> Option<Profile> {
        let profile = self.profiles.get(id)?;
        self.resolve_profile_chain(profile, &mut vec![*id])
    }

    /// Resolve profile inheritance chain, detecting cycles
    fn resolve_profile_chain(
        &self,
        profile: &Profile,
        visited: &mut Vec<ProfileId>,
    ) -> Option<Profile> {
        // If no parent, return the profile as-is
        let Some(parent_id) = profile.parent_id else {
            return Some(profile.clone());
        };

        // Detect cycles
        if visited.contains(&parent_id) {
            log::warn!(
                "Circular profile inheritance detected: {:?} -> {:?}",
                profile.id,
                parent_id
            );
            return Some(profile.clone());
        }

        // Get parent profile
        let Some(parent) = self.profiles.get(&parent_id) else {
            log::warn!(
                "Parent profile {:?} not found for profile {:?}",
                parent_id,
                profile.id
            );
            return Some(profile.clone());
        };

        // Recursively resolve parent
        visited.push(parent_id);
        let resolved_parent = self.resolve_profile_chain(parent, visited)?;

        // Merge: child overrides parent
        Some(Profile {
            id: profile.id,
            name: profile.name.clone(),
            order: profile.order,
            working_directory: profile
                .working_directory
                .clone()
                .or(resolved_parent.working_directory),
            shell: profile.shell.clone().or(resolved_parent.shell),
            login_shell: profile.login_shell.or(resolved_parent.login_shell),
            command: profile.command.clone().or(resolved_parent.command),
            command_args: profile
                .command_args
                .clone()
                .or(resolved_parent.command_args),
            tab_name: profile.tab_name.clone().or(resolved_parent.tab_name),
            icon: profile.icon.clone().or(resolved_parent.icon),
            tags: if profile.tags.is_empty() {
                resolved_parent.tags
            } else {
                profile.tags.clone()
            },
            parent_id: profile.parent_id,
            keyboard_shortcut: profile
                .keyboard_shortcut
                .clone()
                .or(resolved_parent.keyboard_shortcut),
            hostname_patterns: if profile.hostname_patterns.is_empty() {
                resolved_parent.hostname_patterns
            } else {
                profile.hostname_patterns.clone()
            },
            tmux_session_patterns: if profile.tmux_session_patterns.is_empty() {
                resolved_parent.tmux_session_patterns
            } else {
                profile.tmux_session_patterns.clone()
            },
            directory_patterns: if profile.directory_patterns.is_empty() {
                resolved_parent.directory_patterns
            } else {
                profile.directory_patterns.clone()
            },
            badge_text: profile.badge_text.clone().or(resolved_parent.badge_text),
            badge_color: profile.badge_color.or(resolved_parent.badge_color),
            badge_color_alpha: profile
                .badge_color_alpha
                .or(resolved_parent.badge_color_alpha),
            badge_font: profile.badge_font.clone().or(resolved_parent.badge_font),
            badge_font_bold: profile.badge_font_bold.or(resolved_parent.badge_font_bold),
            badge_top_margin: profile
                .badge_top_margin
                .or(resolved_parent.badge_top_margin),
            badge_right_margin: profile
                .badge_right_margin
                .or(resolved_parent.badge_right_margin),
            badge_max_width: profile.badge_max_width.or(resolved_parent.badge_max_width),
            badge_max_height: profile
                .badge_max_height
                .or(resolved_parent.badge_max_height),
            ssh_host: profile.ssh_host.clone().or(resolved_parent.ssh_host),
            ssh_user: profile.ssh_user.clone().or(resolved_parent.ssh_user),
            ssh_port: profile.ssh_port.or(resolved_parent.ssh_port),
            ssh_identity_file: profile
                .ssh_identity_file
                .clone()
                .or(resolved_parent.ssh_identity_file),
            ssh_extra_args: profile
                .ssh_extra_args
                .clone()
                .or(resolved_parent.ssh_extra_args),
            enable_prettifier: profile
                .enable_prettifier
                .or(resolved_parent.enable_prettifier),
            content_prettifier: profile
                .content_prettifier
                .clone()
                .or(resolved_parent.content_prettifier),
            source: profile.source.clone(),
        })
    }

    /// Get profiles that can be parents for a given profile
    /// (excludes the profile itself and any profiles that would create a cycle)
    pub fn get_valid_parents(&self, profile_id: &ProfileId) -> Vec<&Profile> {
        self.profiles_ordered()
            .into_iter()
            .filter(|p| {
                if p.id == *profile_id {
                    return false;
                }
                !self.has_ancestor(&p.id, profile_id)
            })
            .collect()
    }

    /// Check if a profile has a specific ancestor in its inheritance chain
    fn has_ancestor(&self, profile_id: &ProfileId, ancestor_id: &ProfileId) -> bool {
        let mut current_id = *profile_id;
        let mut visited = vec![current_id];

        while let Some(profile) = self.profiles.get(&current_id)
            && let Some(parent_id) = profile.parent_id
        {
            if parent_id == *ancestor_id {
                return true;
            }
            if visited.contains(&parent_id) {
                return false;
            }
            visited.push(parent_id);
            current_id = parent_id;
        }
        false
    }
}
