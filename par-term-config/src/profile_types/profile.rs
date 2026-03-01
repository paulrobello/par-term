//! Core `Profile` struct and its direct implementation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::dynamic::ProfileSource;

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

    /// Shell to use for this profile (e.g. "/bin/zsh", "/usr/bin/fish")
    /// When set, overrides the global custom_shell / $SHELL for this profile.
    /// Takes precedence over global config but is overridden by `command`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,

    /// Per-profile login shell override.
    /// None = inherit global config.login_shell, Some(true/false) = override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_shell: Option<bool>,

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

    /// Directory patterns for automatic profile switching based on CWD
    /// Supports glob patterns (e.g., "/Users/*/projects/work-*", "/home/user/dev/*")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directory_patterns: Vec<String>,

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

    /// SSH hostname for direct connection (profile acts as SSH bookmark)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_host: Option<String>,

    /// SSH user for direct connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_user: Option<String>,

    /// SSH port for direct connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,

    /// SSH identity file path for direct connection
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_identity_file: Option<String>,

    /// Extra SSH arguments (e.g., "-o StrictHostKeyChecking=no")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_extra_args: Option<String>,

    // ========================================================================
    // Content Prettifier overrides (step 6)
    // ========================================================================
    /// Per-profile prettifier enable override (None = inherit global).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_prettifier: Option<bool>,

    /// Per-profile prettifier config overrides (None = inherit global).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_prettifier: Option<crate::config::prettifier::PrettifierConfigOverride>,

    /// Where this profile was loaded from (runtime-only, not persisted to YAML)
    #[serde(skip)]
    pub source: ProfileSource,
}

impl Profile {
    /// Create a new profile with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            working_directory: None,
            shell: None,
            login_shell: None,
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
            directory_patterns: Vec::new(),
            badge_text: None,
            badge_color: None,
            badge_color_alpha: None,
            badge_font: None,
            badge_font_bold: None,
            badge_top_margin: None,
            badge_right_margin: None,
            badge_max_width: None,
            badge_max_height: None,
            ssh_host: None,
            ssh_user: None,
            ssh_port: None,
            ssh_identity_file: None,
            ssh_extra_args: None,
            enable_prettifier: None,
            content_prettifier: None,
            source: ProfileSource::default(),
        }
    }

    /// Create a profile with a specific ID (for testing or deserialization)
    pub fn with_id(id: ProfileId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            working_directory: None,
            shell: None,
            login_shell: None,
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
            directory_patterns: Vec::new(),
            badge_text: None,
            badge_color: None,
            badge_color_alpha: None,
            badge_font: None,
            badge_font_bold: None,
            badge_top_margin: None,
            badge_right_margin: None,
            badge_max_width: None,
            badge_max_height: None,
            ssh_host: None,
            ssh_user: None,
            ssh_port: None,
            ssh_identity_file: None,
            ssh_extra_args: None,
            enable_prettifier: None,
            content_prettifier: None,
            source: ProfileSource::default(),
        }
    }

    /// Builder method to set working directory
    pub fn working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    /// Builder method to set shell
    pub fn shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }

    /// Builder method to set per-profile login shell
    pub fn login_shell(mut self, login: bool) -> Self {
        self.login_shell = Some(login);
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

    /// Builder method to set directory patterns
    pub fn directory_patterns(mut self, patterns: Vec<String>) -> Self {
        self.directory_patterns = patterns;
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

    /// Builder method to set SSH host
    pub fn ssh_host(mut self, host: impl Into<String>) -> Self {
        self.ssh_host = Some(host.into());
        self
    }

    /// Builder method to set SSH user
    pub fn ssh_user(mut self, user: impl Into<String>) -> Self {
        self.ssh_user = Some(user.into());
        self
    }

    /// Builder method to set SSH port
    pub fn ssh_port(mut self, port: u16) -> Self {
        self.ssh_port = Some(port);
        self
    }

    /// Builder method to set prettifier enabled override
    pub fn enable_prettifier(mut self, enabled: bool) -> Self {
        self.enable_prettifier = Some(enabled);
        self
    }

    /// Builder method to set prettifier config override
    pub fn content_prettifier(
        mut self,
        config: crate::config::prettifier::PrettifierConfigOverride,
    ) -> Self {
        self.content_prettifier = Some(config);
        self
    }

    /// Build the SSH command arguments for this profile's SSH connection.
    /// Returns None if ssh_host is not set.
    pub fn ssh_command_args(&self) -> Option<Vec<String>> {
        let host = self.ssh_host.as_ref()?;
        let mut args = Vec::new();

        if let Some(port) = self.ssh_port
            && port != 22
        {
            args.push("-p".to_string());
            args.push(port.to_string());
        }

        if let Some(ref identity) = self.ssh_identity_file {
            args.push("-i".to_string());
            args.push(identity.clone());
        }

        if let Some(ref extra) = self.ssh_extra_args {
            args.extend(extra.split_whitespace().map(String::from));
        }

        let target = if let Some(ref user) = self.ssh_user {
            format!("{}@{}", user, host)
        } else {
            host.clone()
        };
        args.push(target);

        Some(args)
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
