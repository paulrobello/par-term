//! Status bar widget configuration types.
//!
//! Defines the widget identifiers, section layout, and per-widget configuration
//! used by the status bar system.

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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
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
            WidgetId::Custom(name) => name.as_str(),
        }
    }

    /// Icon/prefix character for the widget.
    pub fn icon(&self) -> &str {
        match self {
            WidgetId::Clock => "\u{1f551}",           // clock emoji
            WidgetId::UsernameHostname => "\u{1f464}", // bust in silhouette
            WidgetId::CurrentDirectory => "\u{1f4c2}", // open file folder
            WidgetId::GitBranch => "\u{e0a0}",         // powerline branch symbol
            WidgetId::CpuUsage => "\u{1f4bb}",         // laptop
            WidgetId::MemoryUsage => "\u{1f4be}",      // floppy disk
            WidgetId::NetworkStatus => "\u{1f310}",    // globe with meridians
            WidgetId::BellIndicator => "\u{1f514}",    // bell
            WidgetId::CurrentCommand => "\u{25b6}",    // play button
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
    ]
}
