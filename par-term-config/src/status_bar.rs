//! Status bar widget configuration types.
//!
//! Defines the widget identifiers, section layout, and per-widget configuration
//! used by the status bar system.

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

/// Section of the status bar where a widget is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StatusBarSection {
    /// Left-aligned section (default)
    #[default]
    Left,
    /// Center-aligned section
    Center,
    /// Right-aligned section
    Right,
}

/// Identifier for a built-in or custom status bar widget.
///
/// Serialized as a single plain string (`as_key`/`from_key`) so it round-trips
/// through `config.yaml`, which embeds the status bar via `#[serde(flatten)]`.
/// Built-in widgets use their snake_case name (e.g. `git_branch`); custom
/// widgets use `custom:<name>`. serde's flatten path cannot deserialize the
/// externally-tagged `Custom(String)` map form (`"untagged and internally tagged
/// enums do not support enum input"`), so a manual scalar representation is
/// used instead of the derived one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidgetId {
    /// Current time (HH:MM:SS)
    Clock,
    /// user@hostname
    UsernameHostname,
    /// Current working directory
    CurrentDirectory,
    /// Git branch name with icon
    GitBranch,
    /// CPU usage percentage
    CpuUsage,
    /// Memory usage (used / total)
    MemoryUsage,
    /// Network throughput (rx/tx rates)
    NetworkStatus,
    /// Bell indicator with count
    BellIndicator,
    /// Currently running command name
    CurrentCommand,
    /// Update available notification
    UpdateAvailable,
    /// Custom widget (user-defined via format string)
    Custom(String),
}

impl WidgetId {
    /// Human-readable label for UI display.
    pub fn label(&self) -> &str {
        match self {
            WidgetId::Clock => "Clock",
            WidgetId::UsernameHostname => "User@Host",
            WidgetId::CurrentDirectory => "Directory",
            WidgetId::GitBranch => "Git Branch",
            WidgetId::CpuUsage => "CPU Usage",
            WidgetId::MemoryUsage => "Memory Usage",
            WidgetId::NetworkStatus => "Network Status",
            WidgetId::BellIndicator => "Bell Indicator",
            WidgetId::CurrentCommand => "Current Command",
            WidgetId::UpdateAvailable => "Update Available",
            WidgetId::Custom(name) => name.as_str(),
        }
    }

    /// Icon/prefix character for the widget.
    pub fn icon(&self) -> &str {
        match self {
            WidgetId::Clock => "\u{1f551}",            // clock emoji
            WidgetId::UsernameHostname => "\u{1f464}", // bust in silhouette
            WidgetId::CurrentDirectory => "\u{1f4c2}", // open file folder
            WidgetId::GitBranch => "\u{1f500}",        // twisted rightwards arrows (branch)
            WidgetId::CpuUsage => "\u{1f4bb}",         // laptop
            WidgetId::MemoryUsage => "\u{1f4be}",      // floppy disk
            WidgetId::NetworkStatus => "\u{1f310}",    // globe with meridians
            WidgetId::BellIndicator => "\u{1f514}",    // bell
            WidgetId::CurrentCommand => "\u{25b6}",    // play button
            WidgetId::UpdateAvailable => "\u{2b06}",   // upwards arrow
            WidgetId::Custom(_) => "\u{2699}",         // gear
        }
    }

    /// Whether this widget requires the system monitor to be running.
    pub fn needs_system_monitor(&self) -> bool {
        matches!(
            self,
            WidgetId::CpuUsage | WidgetId::MemoryUsage | WidgetId::NetworkStatus
        )
    }

    /// Stable string key used for YAML serialization. Built-in widgets use their
    /// snake_case name; custom widgets are prefixed with `custom:`.
    fn as_key(&self) -> String {
        match self {
            WidgetId::Clock => "clock".to_string(),
            WidgetId::UsernameHostname => "username_hostname".to_string(),
            WidgetId::CurrentDirectory => "current_directory".to_string(),
            WidgetId::GitBranch => "git_branch".to_string(),
            WidgetId::CpuUsage => "cpu_usage".to_string(),
            WidgetId::MemoryUsage => "memory_usage".to_string(),
            WidgetId::NetworkStatus => "network_status".to_string(),
            WidgetId::BellIndicator => "bell_indicator".to_string(),
            WidgetId::CurrentCommand => "current_command".to_string(),
            WidgetId::UpdateAvailable => "update_available".to_string(),
            WidgetId::Custom(name) => format!("custom:{name}"),
        }
    }

    /// Parse a serialization key back into a [`WidgetId`]. Returns `None` for
    /// unrecognized built-in names.
    fn from_key(key: &str) -> Option<WidgetId> {
        if let Some(name) = key.strip_prefix("custom:") {
            return Some(WidgetId::Custom(name.to_string()));
        }
        Some(match key {
            "clock" => WidgetId::Clock,
            "username_hostname" => WidgetId::UsernameHostname,
            "current_directory" => WidgetId::CurrentDirectory,
            "git_branch" => WidgetId::GitBranch,
            "cpu_usage" => WidgetId::CpuUsage,
            "memory_usage" => WidgetId::MemoryUsage,
            "network_status" => WidgetId::NetworkStatus,
            "bell_indicator" => WidgetId::BellIndicator,
            "current_command" => WidgetId::CurrentCommand,
            "update_available" => WidgetId::UpdateAvailable,
            _ => return None,
        })
    }
}

impl Serialize for WidgetId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_key())
    }
}

impl<'de> Deserialize<'de> for WidgetId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let key = String::deserialize(deserializer)?;
        WidgetId::from_key(&key)
            .ok_or_else(|| de::Error::custom(format!("unknown status bar widget id: `{key}`")))
    }
}

/// Configuration for a single status bar widget.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StatusBarWidgetConfig {
    /// Which widget to display
    pub id: WidgetId,
    /// Whether this widget is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Section placement (left, center, right)
    #[serde(default)]
    pub section: StatusBarSection,
    /// Sort order within the section (lower values first)
    #[serde(default)]
    pub order: i32,
    /// Optional format override string with `\(variable)` interpolation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Default widget configuration set.
///
/// Returns a sensible starting set of widgets covering common use-cases.
/// System monitor widgets (CPU, memory, network) are disabled by default
/// to avoid unnecessary resource usage.
pub fn default_widgets() -> Vec<StatusBarWidgetConfig> {
    vec![
        StatusBarWidgetConfig {
            id: WidgetId::UsernameHostname,
            enabled: true,
            section: StatusBarSection::Left,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::CurrentDirectory,
            enabled: true,
            section: StatusBarSection::Left,
            order: 1,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::GitBranch,
            enabled: true,
            section: StatusBarSection::Left,
            order: 2,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::CurrentCommand,
            enabled: true,
            section: StatusBarSection::Center,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::CpuUsage,
            enabled: false,
            section: StatusBarSection::Right,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::MemoryUsage,
            enabled: false,
            section: StatusBarSection::Right,
            order: 1,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::NetworkStatus,
            enabled: false,
            section: StatusBarSection::Right,
            order: 2,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::BellIndicator,
            enabled: true,
            section: StatusBarSection::Right,
            order: 3,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::Clock,
            enabled: true,
            section: StatusBarSection::Right,
            order: 4,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::UpdateAvailable,
            enabled: true,
            section: StatusBarSection::Right,
            order: 5,
            format: None,
        },
    ]
}
