//! Badge system for displaying session information overlays.
//!
//! Badges are semi-transparent text labels displayed in the terminal corner,
//! showing dynamic information about the session (hostname, username, path, etc.).
//! This implementation follows the iTerm2 badge system design.

use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use crate::profile::Profile;

/// Session variables available for badge interpolation
#[derive(Debug, Clone, Default)]
pub struct SessionVariables {
    /// Remote hostname (from SSH or local)
    pub hostname: String,
    /// Current username
    pub username: String,
    /// Current working directory
    pub path: String,
    /// Current foreground job name
    pub job: Option<String>,
    /// Last executed command
    pub last_command: Option<String>,
    /// Current profile name
    pub profile_name: String,
    /// TTY device name
    pub tty: String,
    /// Terminal columns
    pub columns: usize,
    /// Terminal rows
    pub rows: usize,
    /// Number of bells received
    pub bell_count: usize,
    /// Currently selected text
    pub selection: Option<String>,
    /// tmux pane title (when in tmux mode)
    pub tmux_pane_title: Option<String>,
    /// Custom variables set via escape sequences
    pub custom: HashMap<String, String>,
}

impl SessionVariables {
    /// Create new session variables with system defaults
    pub fn new() -> Self {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "~".to_string());

        Self {
            hostname,
            username,
            path,
            profile_name: "Default".to_string(),
            tty: std::env::var("TTY").unwrap_or_default(),
            columns: 80,
            rows: 24,
            ..Default::default()
        }
    }

    /// Get variable value by name
    pub fn get(&self, name: &str) -> Option<String> {
        match name {
            "session.hostname" => Some(self.hostname.clone()),
            "session.username" => Some(self.username.clone()),
            "session.path" => Some(self.path.clone()),
            "session.job" => self.job.clone(),
            "session.last_command" => self.last_command.clone(),
            "session.profile_name" => Some(self.profile_name.clone()),
            "session.tty" => Some(self.tty.clone()),
            "session.columns" => Some(self.columns.to_string()),
            "session.rows" => Some(self.rows.to_string()),
            "session.bell_count" => Some(self.bell_count.to_string()),
            "session.selection" => self.selection.clone(),
            "session.tmux_pane_title" => self.tmux_pane_title.clone(),
            _ => {
                // Check custom variables
                if let Some(custom_name) = name.strip_prefix("session.") {
                    self.custom.get(custom_name).cloned()
                } else {
                    None
                }
            }
        }
    }

    /// Update the working directory
    pub fn set_path(&mut self, path: String) {
        self.path = path;
    }

    /// Update terminal dimensions
    pub fn set_dimensions(&mut self, cols: usize, rows: usize) {
        self.columns = cols;
        self.rows = rows;
    }

    /// Increment bell count
    pub fn increment_bell(&mut self) {
        self.bell_count += 1;
    }

    /// Set a custom variable
    pub fn set_custom(&mut self, name: &str, value: String) {
        self.custom.insert(name.to_string(), value);
    }
}

/// Badge state and configuration
#[derive(Clone)]
pub struct BadgeState {
    /// Whether badge is enabled
    pub enabled: bool,
    /// Badge format string (with variable placeholders)
    pub format: String,
    /// Rendered badge text after variable interpolation
    pub rendered_text: String,
    /// Badge text color [R, G, B]
    pub color: [u8; 3],
    /// Badge opacity (0.0-1.0)
    pub alpha: f32,
    /// Font family for badge
    pub font: String,
    /// Use bold font
    pub font_bold: bool,
    /// Top margin in pixels
    pub top_margin: f32,
    /// Right margin in pixels
    pub right_margin: f32,
    /// Maximum width as fraction of terminal width (0.0-1.0)
    pub max_width: f32,
    /// Maximum height as fraction of terminal height (0.0-1.0)
    pub max_height: f32,
    /// Session variables for interpolation
    pub variables: Arc<parking_lot::RwLock<SessionVariables>>,
    /// Whether the badge needs re-rendering
    dirty: bool,
}

impl BadgeState {
    /// Create a new badge state from config
    pub fn new(config: &Config) -> Self {
        Self {
            enabled: config.badge_enabled,
            format: config.badge_format.clone(),
            rendered_text: String::new(),
            color: config.badge_color,
            alpha: config.badge_color_alpha,
            font: config.badge_font.clone(),
            font_bold: config.badge_font_bold,
            top_margin: config.badge_top_margin,
            right_margin: config.badge_right_margin,
            max_width: config.badge_max_width,
            max_height: config.badge_max_height,
            variables: Arc::new(parking_lot::RwLock::new(SessionVariables::new())),
            dirty: true,
        }
    }

    /// Update badge configuration
    pub fn update_config(&mut self, config: &Config) {
        let format_changed = self.format != config.badge_format;

        self.enabled = config.badge_enabled;
        self.format = config.badge_format.clone();
        self.color = config.badge_color;
        self.alpha = config.badge_color_alpha;
        self.font = config.badge_font.clone();
        self.font_bold = config.badge_font_bold;
        self.top_margin = config.badge_top_margin;
        self.right_margin = config.badge_right_margin;
        self.max_width = config.badge_max_width;
        self.max_height = config.badge_max_height;

        if format_changed {
            self.dirty = true;
        }
    }

    /// Set badge format directly (e.g., from OSC 1337)
    pub fn set_format(&mut self, format: String) {
        if self.format != format {
            self.format = format;
            self.dirty = true;
        }
    }

    /// Mark badge as needing re-render
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if badge needs re-rendering
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Interpolate variables in the format string
    pub fn interpolate(&mut self) {
        let variables = self.variables.read();
        self.rendered_text = interpolate_badge_format(&self.format, &variables);
        self.dirty = false;
    }

    /// Get the rendered badge text
    pub fn text(&self) -> &str {
        &self.rendered_text
    }

    /// Access session variables for mutation
    pub fn variables_mut(&self) -> parking_lot::RwLockWriteGuard<'_, SessionVariables> {
        self.variables.write()
    }

    /// Apply badge settings from a profile (overrides global config where set)
    ///
    /// This is called when a profile is activated to apply its custom badge settings.
    /// Only non-None profile settings override the current values.
    pub fn apply_profile_settings(&mut self, profile: &Profile) {
        let mut changed = false;

        // Badge format/text
        if let Some(ref text) = profile.badge_text {
            if self.format != *text {
                self.format = text.clone();
                changed = true;
            }
        }

        // Badge color
        if let Some(color) = profile.badge_color {
            self.color = color;
        }

        // Badge alpha
        if let Some(alpha) = profile.badge_color_alpha {
            self.alpha = alpha;
        }

        // Badge font
        if let Some(ref font) = profile.badge_font {
            self.font = font.clone();
        }

        // Badge font bold
        if let Some(bold) = profile.badge_font_bold {
            self.font_bold = bold;
        }

        // Badge top margin
        if let Some(margin) = profile.badge_top_margin {
            self.top_margin = margin;
        }

        // Badge right margin
        if let Some(margin) = profile.badge_right_margin {
            self.right_margin = margin;
        }

        // Badge max width
        if let Some(width) = profile.badge_max_width {
            self.max_width = width;
        }

        // Badge max height
        if let Some(height) = profile.badge_max_height {
            self.max_height = height;
        }

        if changed {
            self.dirty = true;
        }
    }
}

/// Interpolate badge format string with session variables
///
/// Replaces `\(session.*)` placeholders with actual values.
/// Supports:
/// - `\(session.hostname)` - Remote/local hostname
/// - `\(session.username)` - Current user
/// - `\(session.path)` - Working directory
/// - `\(session.job)` - Foreground job
/// - `\(session.last_command)` - Last command
/// - `\(session.profile_name)` - Profile name
/// - `\(session.tty)` - TTY device
/// - `\(session.columns)` - Terminal columns
/// - `\(session.rows)` - Terminal rows
/// - `\(session.bell_count)` - Bell count
/// - `\(session.selection)` - Selected text
/// - `\(session.tmux_pane_title)` - tmux pane title
pub fn interpolate_badge_format(format: &str, variables: &SessionVariables) -> String {
    let mut result = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'(') {
            // Skip the '('
            chars.next();

            // Collect variable name until ')'
            let mut var_name = String::new();
            for c in chars.by_ref() {
                if c == ')' {
                    break;
                }
                var_name.push(c);
            }

            // Look up variable value
            if let Some(value) = variables.get(&var_name) {
                result.push_str(&value);
            }
            // If variable not found, output nothing (empty string)
        } else {
            result.push(ch);
        }
    }

    result
}

/// Decode base64-encoded badge format (for OSC 1337 SetBadgeFormat)
///
/// Returns None if decoding fails or the format contains security risks.
pub fn decode_badge_format(base64_format: &str) -> Option<String> {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;

    let decoded = engine.decode(base64_format).ok()?;
    let format = String::from_utf8(decoded).ok()?;

    // Security check: reject formats that look like function calls
    // or contain suspicious patterns
    if format.contains("$(")
        || format.contains("`")
        || format.contains("eval")
        || format.contains("exec")
    {
        log::warn!(
            "Rejecting badge format with suspicious content: {:?}",
            format
        );
        return None;
    }

    Some(format)
}

/// Render badge using egui
///
/// This function renders the badge as a semi-transparent overlay in the top-right
/// corner of the terminal window.
pub fn render_badge(
    ctx: &egui::Context,
    badge: &BadgeState,
    window_width: f32,
    _window_height: f32,
) {
    if !badge.enabled || badge.rendered_text.is_empty() {
        return;
    }

    // Set up badge styling
    let color = egui::Color32::from_rgba_unmultiplied(
        badge.color[0],
        badge.color[1],
        badge.color[2],
        (badge.alpha * 255.0) as u8,
    );

    // Use a large font for badge
    let font_id = egui::FontId::new(24.0, egui::FontFamily::Proportional);

    // Create an area for the badge in the top-right corner
    egui::Area::new(egui::Id::new("badge_overlay"))
        .fixed_pos(egui::pos2(0.0, badge.top_margin))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            // Calculate position: measure text first to know width
            let text = &badge.rendered_text;

            // Get approximate text width using the painter
            let text_rect = ui.painter().text(
                egui::pos2(0.0, 0.0),
                egui::Align2::LEFT_TOP,
                text,
                font_id.clone(),
                egui::Color32::TRANSPARENT, // Invisible measurement
            );

            // Calculate actual position (right-aligned with margin)
            let x = window_width - text_rect.width() - badge.right_margin;
            let y = badge.top_margin;

            // Draw the actual badge text
            ui.painter().text(
                egui::pos2(x, y),
                egui::Align2::LEFT_TOP,
                text,
                font_id,
                color,
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_basic() {
        let mut vars = SessionVariables::default();
        vars.hostname = "myhost".to_string();
        vars.username = "testuser".to_string();

        let result = interpolate_badge_format("\\(session.username)@\\(session.hostname)", &vars);
        assert_eq!(result, "testuser@myhost");
    }

    #[test]
    fn test_interpolate_missing_variable() {
        let vars = SessionVariables::default();
        let result = interpolate_badge_format("Hello \\(session.nonexistent) World", &vars);
        assert_eq!(result, "Hello  World");
    }

    #[test]
    fn test_interpolate_no_variables() {
        let vars = SessionVariables::default();
        let result = interpolate_badge_format("Plain text", &vars);
        assert_eq!(result, "Plain text");
    }

    #[test]
    fn test_interpolate_escaped_backslash() {
        let vars = SessionVariables::default();
        // Just a backslash not followed by ( should pass through
        let result = interpolate_badge_format("Path: C:\\Users", &vars);
        assert_eq!(result, "Path: C:\\Users");
    }

    #[test]
    fn test_decode_badge_format_valid() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let encoded = engine.encode("Hello World");
        let decoded = decode_badge_format(&encoded);
        assert_eq!(decoded, Some("Hello World".to_string()));
    }

    #[test]
    fn test_decode_badge_format_security_check() {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;

        // Test command substitution rejection
        let encoded = engine.encode("$(whoami)");
        assert!(decode_badge_format(&encoded).is_none());

        // Test backtick rejection
        let encoded = engine.encode("`whoami`");
        assert!(decode_badge_format(&encoded).is_none());

        // Test eval rejection
        let encoded = engine.encode("eval bad");
        assert!(decode_badge_format(&encoded).is_none());
    }

    #[test]
    fn test_session_variables_get() {
        let mut vars = SessionVariables::default();
        vars.hostname = "test".to_string();
        vars.columns = 120;
        vars.rows = 40;

        assert_eq!(vars.get("session.hostname"), Some("test".to_string()));
        assert_eq!(vars.get("session.columns"), Some("120".to_string()));
        assert_eq!(vars.get("session.rows"), Some("40".to_string()));
        assert_eq!(vars.get("session.nonexistent"), None);
    }

    #[test]
    fn test_session_variables_custom() {
        let mut vars = SessionVariables::default();
        vars.set_custom("myvar", "myvalue".to_string());

        assert_eq!(vars.get("session.myvar"), Some("myvalue".to_string()));
    }
}
