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
        self.order.sort_by_key(|id| {
            self.profiles
                .get(id)
                .map(|p| p.order)
                .unwrap_or(usize::MAX)
        });
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
        assert_eq!(
            profile.working_directory.as_deref(),
            Some("/home/user")
        );
        assert_eq!(profile.command.as_deref(), Some("ssh"));
        assert_eq!(
            profile.command_args,
            Some(vec!["user@server".to_string()])
        );
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
}
