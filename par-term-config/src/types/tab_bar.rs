//! Tab bar, window, and status bar configuration types.

use serde::{Deserialize, Serialize};

// ============================================================================
// Tab Bar Types
// ============================================================================

/// Tab visual style preset
///
/// Controls the cosmetic appearance of tabs (colors, sizes, spacing).
/// Each preset applies a set of color/size/spacing adjustments to the tab bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabStyle {
    /// Default dark theme styling
    #[default]
    Dark,
    /// Light theme tab styling
    Light,
    /// Smaller tabs, more visible terminal content
    Compact,
    /// Clean, minimal tab appearance
    Minimal,
    /// Enhanced contrast for accessibility
    HighContrast,
    /// Automatically switch between light/dark styles based on system theme
    Automatic,
}

impl TabStyle {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            TabStyle::Dark => "Dark",
            TabStyle::Light => "Light",
            TabStyle::Compact => "Compact",
            TabStyle::Minimal => "Minimal",
            TabStyle::HighContrast => "High Contrast",
            TabStyle::Automatic => "Automatic",
        }
    }

    /// All available styles for UI iteration
    pub fn all() -> &'static [TabStyle] {
        &[
            TabStyle::Dark,
            TabStyle::Light,
            TabStyle::Compact,
            TabStyle::Minimal,
            TabStyle::HighContrast,
            TabStyle::Automatic,
        ]
    }

    /// All concrete styles (excludes Automatic) — for sub-style dropdowns
    pub fn all_concrete() -> &'static [TabStyle] {
        &[
            TabStyle::Dark,
            TabStyle::Light,
            TabStyle::Compact,
            TabStyle::Minimal,
            TabStyle::HighContrast,
        ]
    }
}

/// Tab bar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabBarPosition {
    /// Tab bar at the top of the window (default)
    #[default]
    Top,
    /// Tab bar at the bottom of the window
    Bottom,
    /// Tab bar on the left side of the window (vertical layout)
    Left,
}

impl TabBarPosition {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            TabBarPosition::Top => "Top",
            TabBarPosition::Bottom => "Bottom",
            TabBarPosition::Left => "Left",
        }
    }

    /// All available positions for UI iteration
    pub fn all() -> &'static [TabBarPosition] {
        &[
            TabBarPosition::Top,
            TabBarPosition::Bottom,
            TabBarPosition::Left,
        ]
    }

    /// Returns true if the tab bar is horizontal (top or bottom)
    pub fn is_horizontal(&self) -> bool {
        matches!(self, TabBarPosition::Top | TabBarPosition::Bottom)
    }
}

/// Tab bar visibility mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabBarMode {
    /// Always show tab bar (default)
    #[default]
    Always,
    /// Show tab bar only when there are multiple tabs
    WhenMultiple,
    /// Never show tab bar
    Never,
}

/// Controls how tab titles are automatically updated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabTitleMode {
    /// OSC title first, then CWD from shell integration, then keep default
    #[default]
    Auto,
    /// Only update from explicit OSC escape sequences; never auto-set from CWD
    OscOnly,
}

// ============================================================================
// Window Types
// ============================================================================

/// Window type for different display modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowType {
    /// Normal window (default)
    #[default]
    Normal,
    /// Start in fullscreen mode
    Fullscreen,
    /// Edge-anchored window (for dropdown/Quake-style terminals)
    /// Note: Edge-anchored windows require additional platform-specific support
    EdgeTop,
    /// Edge-anchored to bottom of screen
    EdgeBottom,
    /// Edge-anchored to left of screen
    EdgeLeft,
    /// Edge-anchored to right of screen
    EdgeRight,
}

impl WindowType {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            WindowType::Normal => "Normal",
            WindowType::Fullscreen => "Fullscreen",
            WindowType::EdgeTop => "Edge (Top)",
            WindowType::EdgeBottom => "Edge (Bottom)",
            WindowType::EdgeLeft => "Edge (Left)",
            WindowType::EdgeRight => "Edge (Right)",
        }
    }

    /// All available window types for UI iteration
    pub fn all() -> &'static [WindowType] {
        &[
            WindowType::Normal,
            WindowType::Fullscreen,
            WindowType::EdgeTop,
            WindowType::EdgeBottom,
            WindowType::EdgeLeft,
            WindowType::EdgeRight,
        ]
    }

    /// Returns true if this is an edge-anchored window type
    pub fn is_edge(&self) -> bool {
        matches!(
            self,
            WindowType::EdgeTop
                | WindowType::EdgeBottom
                | WindowType::EdgeLeft
                | WindowType::EdgeRight
        )
    }
}

// ============================================================================
// Status Bar Types
// ============================================================================

/// Status bar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StatusBarPosition {
    /// Status bar at the top of the window
    Top,
    /// Status bar at the bottom of the window (default)
    #[default]
    Bottom,
}
