//! Profile types and manager for terminal session configurations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a profile
pub type ProfileId = Uuid;

/// A terminal session profile containing configuration for how to start a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Unique identifier for this profile
    pub id: ProfileId,

    /// Display name for the profile
    pub name: String,

    /// Working directory for the session (if None, uses config default or inherits)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,

    /// Command to run instead of the default shell
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Arguments for the command
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_args: Option<Vec<String>>,

    /// Custom tab name (if None, uses default naming)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_name: Option<String>,

    /// Icon identifier for the profile (emoji or icon name)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// Display order in the profile list
    #[serde(default)]
    pub order: usize,

    // ========================================================================
    // New fields for enhanced profile system (issue #78)
    // ========================================================================
    /// Searchable tags to organize and filter profiles
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Parent profile ID for inheritance (child overrides parent settings)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ProfileId>,

    /// Keyboard shortcut for quick launch (e.g., "Cmd+1", "Ctrl+Shift+1")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyboard_shortcut: Option<String>,

    /// Hostname patterns for automatic profile switching when SSH connects
    /// Supports glob patterns (e.g., "*.example.com", "server-*")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hostname_patterns: Vec<String>,

    /// Tmux session name patterns for automatic profile switching when connecting via tmux control mode
    /// Supports glob patterns (e.g., "work-*", "dev-session", "*-production")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tmux_session_patterns: Vec<String>,

    /// Per-profile badge text (overrides global badge_format when this profile is active)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_text: Option<String>,

    /// Per-profile badge color [R, G, B] (overrides global badge_color)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_color: Option<[u8; 3]>,

    /// Per-profile badge opacity 0.0-1.0 (overrides global badge_color_alpha)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_color_alpha: Option<f32>,

    /// Per-profile badge font family (overrides global badge_font)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_font: Option<String>,

    /// Per-profile badge font bold (overrides global badge_font_bold)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_font_bold: Option<bool>,

    /// Per-profile badge top margin in pixels (overrides global badge_top_margin)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_top_margin: Option<f32>,

    /// Per-profile badge right margin in pixels (overrides global badge_right_margin)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_right_margin: Option<f32>,

    /// Per-profile badge max width as fraction 0.0-1.0 (overrides global badge_max_width)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_max_width: Option<f32>,

    /// Per-profile badge max height as fraction 0.0-1.0 (overrides global badge_max_height)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_max_height: Option<f32>,
}

#[allow(dead_code)]
impl Profile {
    /// Create a new profile with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            working_directory: None,
            command: None,
            command_args: None,
            tab_name: None,
            icon: None,
            order: 0,
            tags: Vec::new(),
            parent_id: None,
            keyboard_shortcut: None,
            hostname_patterns: Vec::new(),
            tmux_session_patterns: Vec::new(),
            badge_text: None,
            badge_color: None,
            badge_color_alpha: None,
            badge_font: None,
            badge_font_bold: None,
            badge_top_margin: None,
            badge_right_margin: None,
            badge_max_width: None,
            badge_max_height: None,
        }
    }

    /// Create a profile with a specific ID (for testing or deserialization)
    pub fn with_id(id: ProfileId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            working_directory: None,
            command: None,
            command_args: None,
            tab_name: None,
            icon: None,
            order: 0,
            tags: Vec::new(),
            parent_id: None,
            keyboard_shortcut: None,
            hostname_patterns: Vec::new(),
            tmux_session_patterns: Vec::new(),
            badge_text: None,
            badge_color: None,
            badge_color_alpha: None,
            badge_font: None,
            badge_font_bold: None,
            badge_top_margin: None,
            badge_right_margin: None,
            badge_max_width: None,
            badge_max_height: None,
        }
    }

    /// Builder method to set working directory
    pub fn working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    /// Builder method to set command
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Builder method to set command arguments
    pub fn command_args(mut self, args: Vec<String>) -> Self {
        self.command_args = Some(args);
        self
    }

    /// Builder method to set tab name
    pub fn tab_name(mut self, name: impl Into<String>) -> Self {
        self.tab_name = Some(name.into());
        self
    }

    /// Builder method to set icon
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Builder method to set order
    pub fn order(mut self, order: usize) -> Self {
        self.order = order;
        self
    }

    /// Builder method to set tags
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Builder method to set parent profile ID
    pub fn parent_id(mut self, parent_id: ProfileId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Builder method to set keyboard shortcut
    pub fn keyboard_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.keyboard_shortcut = Some(shortcut.into());
        self
    }

    /// Builder method to set hostname patterns
    pub fn hostname_patterns(mut self, patterns: Vec<String>) -> Self {
        self.hostname_patterns = patterns;
        self
    }

    /// Builder method to set tmux session patterns
    pub fn tmux_session_patterns(mut self, patterns: Vec<String>) -> Self {
        self.tmux_session_patterns = patterns;
        self
    }

    /// Builder method to set badge text
    pub fn badge_text(mut self, text: impl Into<String>) -> Self {
        self.badge_text = Some(text.into());
        self
    }

    /// Builder method to set badge color
    pub fn badge_color(mut self, color: [u8; 3]) -> Self {
        self.badge_color = Some(color);
        self
    }

    /// Builder method to set badge color alpha
    pub fn badge_color_alpha(mut self, alpha: f32) -> Self {
        self.badge_color_alpha = Some(alpha);
        self
    }

    /// Builder method to set badge font
    pub fn badge_font(mut self, font: impl Into<String>) -> Self {
        self.badge_font = Some(font.into());
        self
    }

    /// Builder method to set badge font bold
    pub fn badge_font_bold(mut self, bold: bool) -> Self {
        self.badge_font_bold = Some(bold);
        self
    }

    /// Builder method to set badge top margin
    pub fn badge_top_margin(mut self, margin: f32) -> Self {
        self.badge_top_margin = Some(margin);
        self
    }

    /// Builder method to set badge right margin
    pub fn badge_right_margin(mut self, margin: f32) -> Self {
        self.badge_right_margin = Some(margin);
        self
    }

    /// Builder method to set badge max width
    pub fn badge_max_width(mut self, width: f32) -> Self {
        self.badge_max_width = Some(width);
        self
    }

    /// Builder method to set badge max height
    pub fn badge_max_height(mut self, height: f32) -> Self {
        self.badge_max_height = Some(height);
        self
    }

    /// Get the display label (icon + name if icon exists)
    pub fn display_label(&self) -> String {
        if let Some(icon) = &self.icon {
            format!("{} {}", icon, self.name)
        } else {
            self.name.clone()
        }
    }

    /// Validate the profile configuration
    /// Returns a list of validation warnings (not errors - profiles can be incomplete)
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.name.trim().is_empty() {
            warnings.push("Profile name is empty".to_string());
        }

        if let Some(dir) = &self.working_directory
            && !dir.is_empty()
            && !std::path::Path::new(dir).exists()
        {
            warnings.push(format!("Working directory does not exist: {}", dir));
        }

        warnings
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::new("New Profile")
    }
}

/// Manages a collection of profiles
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProfileManager {
    /// All profiles indexed by ID
    profiles: HashMap<ProfileId, Profile>,

    /// Ordered list of profile IDs for display
    order: Vec<ProfileId>,
}

#[allow(dead_code)]
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

    // ========================================================================
    // New methods for enhanced profile system (issue #78)
    // ========================================================================

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

    // Keep the old method name as an alias for backwards compatibility
    #[allow(dead_code)]
    fn hostname_matches(hostname: &str, pattern: &str) -> bool {
        Self::pattern_matches(hostname, pattern)
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
            // Merge optional fields: child wins if set, otherwise use parent
            working_directory: profile
                .working_directory
                .clone()
                .or(resolved_parent.working_directory),
            command: profile.command.clone().or(resolved_parent.command),
            command_args: profile
                .command_args
                .clone()
                .or(resolved_parent.command_args),
            tab_name: profile.tab_name.clone().or(resolved_parent.tab_name),
            icon: profile.icon.clone().or(resolved_parent.icon),
            // New fields
            tags: if profile.tags.is_empty() {
                resolved_parent.tags
            } else {
                profile.tags.clone()
            },
            parent_id: profile.parent_id, // Keep original parent reference
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
        })
    }

    /// Get profiles that can be parents for a given profile
    /// (excludes the profile itself and any profiles that would create a cycle)
    pub fn get_valid_parents(&self, profile_id: &ProfileId) -> Vec<&Profile> {
        self.profiles_ordered()
            .into_iter()
            .filter(|p| {
                // Cannot be own parent
                if p.id == *profile_id {
                    return false;
                }
                // Check if this profile has the target as an ancestor (would create cycle)
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
                // Cycle detected, stop
                return false;
            }
            visited.push(parent_id);
            current_id = parent_id;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("Test Profile");
        assert!(!profile.id.is_nil());
        assert_eq!(profile.name, "Test Profile");
        assert!(profile.working_directory.is_none());
        assert!(profile.command.is_none());
    }

    #[test]
    fn test_profile_builder() {
        let profile = Profile::new("SSH Server")
            .working_directory("/home/user")
            .command("ssh")
            .command_args(vec!["user@server".to_string()])
            .tab_name("Remote")
            .icon("🖥");

        assert_eq!(profile.name, "SSH Server");
        assert_eq!(profile.working_directory.as_deref(), Some("/home/user"));
        assert_eq!(profile.command.as_deref(), Some("ssh"));
        assert_eq!(profile.command_args, Some(vec!["user@server".to_string()]));
        assert_eq!(profile.tab_name.as_deref(), Some("Remote"));
        assert_eq!(profile.icon.as_deref(), Some("🖥"));
    }

    #[test]
    fn test_profile_display_label() {
        let profile_no_icon = Profile::new("Basic");
        assert_eq!(profile_no_icon.display_label(), "Basic");

        let profile_with_icon = Profile::new("Server").icon("🖥");
        assert_eq!(profile_with_icon.display_label(), "🖥 Server");
    }

    #[test]
    fn test_profile_manager_basic_operations() {
        let mut manager = ProfileManager::new();
        assert!(manager.is_empty());

        let profile = Profile::new("First");
        let id = profile.id;
        manager.add(profile);

        assert_eq!(manager.len(), 1);
        assert!(manager.get(&id).is_some());
        assert_eq!(manager.get(&id).unwrap().name, "First");

        // Remove
        let removed = manager.remove(&id);
        assert!(removed.is_some());
        assert!(manager.is_empty());
    }

    #[test]
    fn test_profile_manager_ordering() {
        let mut manager = ProfileManager::new();

        let p1 = Profile::new("First").order(0);
        let p2 = Profile::new("Second").order(1);
        let p3 = Profile::new("Third").order(2);

        let id1 = p1.id;
        let id2 = p2.id;
        let id3 = p3.id;

        manager.add(p1);
        manager.add(p2);
        manager.add(p3);

        let ordered = manager.profiles_ordered();
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].id, id1);
        assert_eq!(ordered[1].id, id2);
        assert_eq!(ordered[2].id, id3);

        // Move second to first position
        manager.move_up(&id2);
        let ordered = manager.profiles_ordered();
        assert_eq!(ordered[0].id, id2);
        assert_eq!(ordered[1].id, id1);

        // Move second (now first) down
        manager.move_down(&id2);
        let ordered = manager.profiles_ordered();
        assert_eq!(ordered[0].id, id1);
        assert_eq!(ordered[1].id, id2);
    }

    #[test]
    fn test_profile_serialization() {
        let profile = Profile::new("Test")
            .working_directory("/tmp")
            .command("bash")
            .tab_name("My Tab");

        let yaml = serde_yaml::to_string(&profile).unwrap();
        let deserialized: Profile = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.id, profile.id);
        assert_eq!(deserialized.name, profile.name);
        assert_eq!(deserialized.working_directory, profile.working_directory);
        assert_eq!(deserialized.command, profile.command);
        assert_eq!(deserialized.tab_name, profile.tab_name);
    }

    #[test]
    fn test_profile_validation() {
        let valid = Profile::new("Valid Profile");
        assert!(valid.validate().is_empty());

        let empty_name = Profile::new("");
        let warnings = empty_name.validate();
        assert!(!warnings.is_empty());

        let bad_dir = Profile::new("Bad Dir").working_directory("/nonexistent/path/12345");
        let warnings = bad_dir.validate();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_find_by_name() {
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("Production Server"));
        manager.add(Profile::new("Development"));

        assert!(manager.find_by_name("production server").is_some());
        assert!(manager.find_by_name("DEVELOPMENT").is_some());
        assert!(manager.find_by_name("nonexistent").is_none());
    }

    // ========================================================================
    // Tests for new profile features (issue #78)
    // ========================================================================

    #[test]
    fn test_profile_new_fields() {
        let profile = Profile::new("Enhanced")
            .tags(vec!["ssh".to_string(), "production".to_string()])
            .keyboard_shortcut("Cmd+1")
            .hostname_patterns(vec!["*.example.com".to_string()])
            .badge_text("PROD");

        assert_eq!(profile.tags, vec!["ssh", "production"]);
        assert_eq!(profile.keyboard_shortcut.as_deref(), Some("Cmd+1"));
        assert_eq!(profile.hostname_patterns, vec!["*.example.com"]);
        assert_eq!(profile.badge_text.as_deref(), Some("PROD"));
    }

    #[test]
    fn test_profile_serialization_new_fields() {
        let profile = Profile::new("Test")
            .tags(vec!["tag1".to_string(), "tag2".to_string()])
            .keyboard_shortcut("Ctrl+Shift+1")
            .hostname_patterns(vec!["server-*".to_string()])
            .badge_text("TEST");

        let yaml = serde_yaml::to_string(&profile).unwrap();
        let deserialized: Profile = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.tags, profile.tags);
        assert_eq!(deserialized.keyboard_shortcut, profile.keyboard_shortcut);
        assert_eq!(deserialized.hostname_patterns, profile.hostname_patterns);
        assert_eq!(deserialized.badge_text, profile.badge_text);
    }

    #[test]
    fn test_find_by_shortcut() {
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("SSH").keyboard_shortcut("Cmd+1"));
        manager.add(Profile::new("Dev").keyboard_shortcut("Cmd+2"));
        manager.add(Profile::new("No Shortcut"));

        assert!(manager.find_by_shortcut("cmd+1").is_some());
        assert_eq!(manager.find_by_shortcut("Cmd+1").unwrap().name, "SSH");
        assert!(manager.find_by_shortcut("cmd+3").is_none());
    }

    #[test]
    fn test_find_by_tag() {
        let mut manager = ProfileManager::new();
        manager
            .add(Profile::new("Prod SSH").tags(vec!["ssh".to_string(), "production".to_string()]));
        manager
            .add(Profile::new("Dev SSH").tags(vec!["ssh".to_string(), "development".to_string()]));
        manager.add(Profile::new("Local").tags(vec!["local".to_string()]));

        let ssh_profiles = manager.find_by_tag("ssh");
        assert_eq!(ssh_profiles.len(), 2);

        let prod_profiles = manager.find_by_tag("PRODUCTION"); // case-insensitive
        assert_eq!(prod_profiles.len(), 1);
        assert_eq!(prod_profiles[0].name, "Prod SSH");

        let no_match = manager.find_by_tag("nonexistent");
        assert!(no_match.is_empty());
    }

    #[test]
    fn test_filter_by_tags() {
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("Production Server").tags(vec!["prod".to_string()]));
        manager.add(Profile::new("Dev Server").tags(vec!["dev".to_string()]));
        manager.add(Profile::new("Local").tags(vec!["local".to_string()]));

        // Filter by partial tag match
        let filtered = manager.filter_by_tags("prod");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Production Server");

        // Filter by name match (fallback)
        let filtered = manager.filter_by_tags("Server");
        assert_eq!(filtered.len(), 2);

        // Empty filter returns all
        let all = manager.filter_by_tags("");
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_all_tags() {
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("P1").tags(vec!["ssh".to_string(), "production".to_string()]));
        manager.add(Profile::new("P2").tags(vec!["ssh".to_string(), "development".to_string()]));
        manager.add(Profile::new("P3").tags(vec!["local".to_string()]));

        let tags = manager.all_tags();
        assert_eq!(tags, vec!["development", "local", "production", "ssh"]);
    }

    #[test]
    fn test_hostname_matching() {
        assert!(ProfileManager::hostname_matches(
            "server.example.com",
            "*.example.com"
        ));
        assert!(ProfileManager::hostname_matches(
            "server.example.com",
            "server*"
        ));
        assert!(ProfileManager::hostname_matches(
            "myserver.example.com",
            "*server*"
        ));
        assert!(ProfileManager::hostname_matches(
            "server.example.com",
            "server.example.com"
        ));
        assert!(ProfileManager::hostname_matches("anything", "*"));

        assert!(!ProfileManager::hostname_matches(
            "server.other.com",
            "*.example.com"
        ));
        assert!(!ProfileManager::hostname_matches(
            "other.example.com",
            "server*"
        ));
    }

    #[test]
    fn test_find_by_hostname() {
        let mut manager = ProfileManager::new();
        manager
            .add(Profile::new("Example.com").hostname_patterns(vec!["*.example.com".to_string()]));
        manager.add(Profile::new("Dev Servers").hostname_patterns(vec!["dev-*".to_string()]));
        manager.add(Profile::new("Catch-all")); // No patterns

        assert_eq!(
            manager.find_by_hostname("server.example.com").unwrap().name,
            "Example.com"
        );
        assert_eq!(
            manager.find_by_hostname("dev-web-01").unwrap().name,
            "Dev Servers"
        );
        assert!(manager.find_by_hostname("unknown.host").is_none());
    }

    #[test]
    fn test_pattern_matching() {
        // Suffix match
        assert!(ProfileManager::pattern_matches("work-session", "*-session"));
        // Prefix match
        assert!(ProfileManager::pattern_matches("dev-server-01", "dev-*"));
        // Contains match
        assert!(ProfileManager::pattern_matches(
            "my-production-env",
            "*production*"
        ));
        // Exact match
        assert!(ProfileManager::pattern_matches("main", "main"));
        // Wildcard
        assert!(ProfileManager::pattern_matches("anything", "*"));
        // Case insensitive
        assert!(ProfileManager::pattern_matches("WORK-SESSION", "*-session"));
        assert!(ProfileManager::pattern_matches("work-session", "*-SESSION"));

        // Non-matches
        assert!(!ProfileManager::pattern_matches("work-session", "dev-*"));
        assert!(!ProfileManager::pattern_matches("dev-server", "*-session"));
    }

    #[test]
    fn test_find_by_tmux_session() {
        let mut manager = ProfileManager::new();
        manager.add(Profile::new("Work Profile").tmux_session_patterns(vec!["work-*".to_string()]));
        manager.add(
            Profile::new("Dev Profile")
                .tmux_session_patterns(vec!["dev-*".to_string(), "*-development".to_string()]),
        );
        manager.add(Profile::new("Default")); // No patterns

        assert_eq!(
            manager.find_by_tmux_session("work-main").unwrap().name,
            "Work Profile"
        );
        assert_eq!(
            manager.find_by_tmux_session("dev-feature").unwrap().name,
            "Dev Profile"
        );
        assert_eq!(
            manager
                .find_by_tmux_session("staging-development")
                .unwrap()
                .name,
            "Dev Profile"
        );
        assert!(manager.find_by_tmux_session("production").is_none());
    }

    #[test]
    fn test_profile_inheritance() {
        let mut manager = ProfileManager::new();

        // Create parent profile
        let parent = Profile::new("Base SSH")
            .working_directory("/home/user")
            .command("ssh")
            .tags(vec!["ssh".to_string()])
            .badge_text("SSH");
        let parent_id = parent.id;
        manager.add(parent);

        // Create child profile that overrides some settings
        let child = Profile::new("Production SSH")
            .parent_id(parent_id)
            .command_args(vec!["prod@server.example.com".to_string()])
            .badge_text("PROD"); // Override parent badge
        let child_id = child.id;
        manager.add(child);

        // Resolve child profile
        let resolved = manager.resolve_profile(&child_id).unwrap();

        // Child values
        assert_eq!(resolved.name, "Production SSH");
        assert_eq!(resolved.badge_text.as_deref(), Some("PROD")); // Child override
        assert_eq!(
            resolved.command_args,
            Some(vec!["prod@server.example.com".to_string()])
        );

        // Inherited from parent
        assert_eq!(resolved.working_directory.as_deref(), Some("/home/user"));
        assert_eq!(resolved.command.as_deref(), Some("ssh"));
        assert_eq!(resolved.tags, vec!["ssh"]);
    }

    #[test]
    fn test_profile_inheritance_chain() {
        let mut manager = ProfileManager::new();

        // Grandparent -> Parent -> Child
        let grandparent = Profile::new("Base")
            .working_directory("/base")
            .command("bash");
        let grandparent_id = grandparent.id;
        manager.add(grandparent);

        let parent = Profile::new("SSH Base")
            .parent_id(grandparent_id)
            .command("ssh"); // Override command
        let parent_id = parent.id;
        manager.add(parent);

        let child = Profile::new("Production")
            .parent_id(parent_id)
            .command_args(vec!["user@prod".to_string()]);
        let child_id = child.id;
        manager.add(child);

        let resolved = manager.resolve_profile(&child_id).unwrap();

        assert_eq!(resolved.working_directory.as_deref(), Some("/base")); // From grandparent
        assert_eq!(resolved.command.as_deref(), Some("ssh")); // From parent
        assert_eq!(resolved.command_args, Some(vec!["user@prod".to_string()])); // Own value
    }

    #[test]
    fn test_profile_inheritance_cycle_detection() {
        let mut manager = ProfileManager::new();

        // Create profiles that reference each other
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let mut p1 = Profile::with_id(id1, "Profile 1");
        p1.parent_id = Some(id2);
        manager.add(p1);

        let mut p2 = Profile::with_id(id2, "Profile 2");
        p2.parent_id = Some(id1);
        manager.add(p2);

        // Resolving should not loop forever - cycle detection should kick in
        let resolved = manager.resolve_profile(&id1);
        assert!(resolved.is_some()); // Should still return something
    }

    #[test]
    fn test_get_valid_parents() {
        let mut manager = ProfileManager::new();

        let p1 = Profile::new("Profile 1");
        let id1 = p1.id;
        manager.add(p1);

        let mut p2 = Profile::new("Profile 2");
        p2.parent_id = Some(id1);
        let id2 = p2.id;
        manager.add(p2);

        let p3 = Profile::new("Profile 3");
        let id3 = p3.id;
        manager.add(p3);

        // Profile 1 can have Profile 3 as parent (but not Profile 2, which has Profile 1 as ancestor)
        // Profile 2 has Profile 1 as parent, so if Profile 1 had Profile 2 as parent -> cycle
        let valid_for_p1 = manager.get_valid_parents(&id1);
        assert_eq!(valid_for_p1.len(), 1); // Only Profile 3
        assert!(valid_for_p1.iter().any(|p| p.id == id3));

        // Profile 2 can have Profile 3 as parent, and Profile 1 is already its parent
        let valid_for_p2 = manager.get_valid_parents(&id2);
        assert!(valid_for_p2.iter().any(|p| p.id == id3));
        assert!(valid_for_p2.iter().any(|p| p.id == id1));

        // Profile 3 can have Profile 1 or Profile 2 as parent
        let valid_for_p3 = manager.get_valid_parents(&id3);
        assert_eq!(valid_for_p3.len(), 2);
    }
}
