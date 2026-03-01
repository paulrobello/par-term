//! Vertical sidebar navigation for settings tabs.
//!
//! This component provides a vertical tab list on the left side of the settings UI,
//! replacing the previous horizontal tab bar for better organization.

use super::SettingsUI;
use crate::search_keywords::tab_search_keywords;

// --- Sidebar color palette ---

/// Text color for tabs that do not match the current search query (dimmed).
const COLOR_TAB_DIMMED: egui::Color32 = egui::Color32::from_rgb(80, 80, 80);

/// Text color for the currently selected tab (bright white).
const COLOR_TAB_SELECTED: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);

/// Text color for unselected tabs that match the search query.
const COLOR_TAB_NORMAL: egui::Color32 = egui::Color32::from_rgb(180, 180, 180);

/// Background fill for the selected tab row.
const COLOR_TAB_SELECTED_BG: egui::Color32 = egui::Color32::from_rgb(60, 60, 70);

/// Border/stroke color drawn around the selected tab row.
const COLOR_TAB_SELECTED_BORDER: egui::Color32 = egui::Color32::from_rgb(100, 100, 120);

/// Width of the border stroke drawn around the selected tab row (pixels).
const TAB_BORDER_WIDTH: f32 = 1.0;

/// Width of each tab button in the sidebar (pixels).
const TAB_BUTTON_WIDTH: f32 = 140.0;

/// Height of each tab button in the sidebar (pixels).
const TAB_BUTTON_HEIGHT: f32 = 32.0;

/// Vertical spacing added above and below the tab list.
const TAB_LIST_PADDING: f32 = 8.0;

/// The available settings tabs in the reorganized UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Window,
    Input,
    Terminal,
    Effects,
    Badge,
    ProgressBar,
    StatusBar,
    Profiles,
    Ssh,
    Notifications,
    Integrations,
    Automation,
    Scripts,
    Snippets,
    Actions,
    ContentPrettifier,
    Arrangements,
    AiInspector,
    Advanced,
}

impl SettingsTab {
    /// Get the display name for this tab.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Window => "Window",
            Self::Input => "Input",
            Self::Terminal => "Terminal",
            Self::Effects => "Effects",
            Self::Badge => "Badge",
            Self::ProgressBar => "Progress Bar",
            Self::StatusBar => "Status Bar",
            Self::Profiles => "Profiles",
            Self::Ssh => "SSH",
            Self::Notifications => "Notifications",
            Self::Integrations => "Integrations",
            Self::Automation => "Automation",
            Self::Scripts => "Scripts",
            Self::Snippets => "Snippets",
            Self::Actions => "Actions",
            Self::ContentPrettifier => "Prettifier",
            Self::Arrangements => "Arrangements",
            Self::AiInspector => "Assistant",
            Self::Advanced => "Advanced",
        }
    }

    /// Get the icon for this tab (using emoji for simplicity).
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Appearance => "🎨",
            Self::Window => "🪟",
            Self::Input => "⌨",
            Self::Terminal => "📟",
            Self::Effects => "✨",
            Self::Badge => "🏷",
            Self::ProgressBar => "📊",
            Self::StatusBar => "🖥",
            Self::Profiles => "👤",
            Self::Ssh => "🔗",
            Self::Notifications => "🔔",
            Self::Integrations => "🔌",
            Self::Automation => "⚡",
            Self::Scripts => "📜",
            Self::Snippets => "📝",
            Self::Actions => "🚀",
            Self::ContentPrettifier => "🔮",
            Self::Arrangements => "📐",
            Self::AiInspector => "💬",
            Self::Advanced => "⚙",
        }
    }

    /// Get all available tabs in order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Appearance,
            Self::Window,
            Self::Input,
            Self::Terminal,
            Self::Effects,
            Self::Badge,
            Self::ProgressBar,
            Self::StatusBar,
            Self::Profiles,
            Self::Ssh,
            Self::Notifications,
            Self::Integrations,
            Self::Automation,
            Self::Scripts,
            Self::Snippets,
            Self::Actions,
            Self::ContentPrettifier,
            Self::Arrangements,
            Self::AiInspector,
            Self::Advanced,
        ]
    }
}

/// Render the sidebar navigation.
///
/// Returns true if the selected tab changed.
pub fn show(ui: &mut egui::Ui, current_tab: &mut SettingsTab, search_query: &str) -> bool {
    let mut tab_changed = false;

    // Add some vertical spacing at the top
    ui.add_space(TAB_LIST_PADDING);

    for tab in SettingsTab::all() {
        let is_selected = *current_tab == *tab;

        // Check if this tab has any matches for the search query
        let has_matches = search_query.is_empty() || tab_matches_search(*tab, search_query);

        // Dim tabs that don't match search
        let text_color = if !has_matches {
            COLOR_TAB_DIMMED
        } else if is_selected {
            COLOR_TAB_SELECTED
        } else {
            COLOR_TAB_NORMAL
        };

        let bg_color = if is_selected {
            COLOR_TAB_SELECTED_BG
        } else {
            egui::Color32::TRANSPARENT
        };

        // Create a selectable button-like widget
        let response = ui.add_sized(
            [TAB_BUTTON_WIDTH, TAB_BUTTON_HEIGHT],
            egui::Button::new(
                egui::RichText::new(format!("{} {}", tab.icon(), tab.display_name()))
                    .color(text_color),
            )
            .fill(bg_color)
            .stroke(if is_selected {
                egui::Stroke::new(TAB_BORDER_WIDTH, COLOR_TAB_SELECTED_BORDER)
            } else {
                egui::Stroke::NONE
            }),
        );

        if response.clicked() && has_matches {
            *current_tab = *tab;
            tab_changed = true;
        }

        // Show tooltip with tab contents summary
        response.on_hover_text(tab_contents_summary(*tab));
    }

    ui.add_space(TAB_LIST_PADDING);

    tab_changed
}

/// Check if a tab matches the search query.
pub fn tab_matches_search(tab: SettingsTab, query: &str) -> bool {
    let query = query.to_lowercase();
    let keywords = tab_search_keywords(tab);

    // Check tab name
    if tab.display_name().to_lowercase().contains(&query) {
        return true;
    }

    // Check keywords
    keywords.iter().any(|k| k.to_lowercase().contains(&query))
}

/// Get a summary of tab contents for tooltip.
fn tab_contents_summary(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::Appearance => "Theme, fonts, cursor style and colors",
        SettingsTab::Window => "Window size, opacity, tab bar, split panes",
        SettingsTab::Input => "Keyboard shortcuts, mouse behavior, clipboard",
        SettingsTab::Terminal => "Shell, scrollback, search, scrollbar",
        SettingsTab::Effects => "Background image/shader, cursor effects",
        SettingsTab::StatusBar => {
            "Status bar widgets, layout, styling, auto-hide, and poll intervals"
        }
        SettingsTab::Badge => "Session info overlay (hostname, username, etc.)",
        SettingsTab::ProgressBar => "Progress bar style, position, and colors",
        SettingsTab::Profiles => "Create and manage terminal profiles",
        SettingsTab::Ssh => "SSH connection settings, mDNS discovery, auto-switch behavior",
        SettingsTab::Notifications => "Bell, activity alerts, desktop notifications",
        SettingsTab::Integrations => "Shell integration, shader bundle installation",
        SettingsTab::Automation => "Regex triggers, trigger actions, coprocesses",
        SettingsTab::Scripts => "External observer scripts that receive terminal events",
        SettingsTab::Snippets => "Text snippets with variable substitution",
        SettingsTab::Actions => "Custom actions (shell, text, keys)",
        SettingsTab::ContentPrettifier => {
            "Content detection, renderers, custom renderers, Claude Code integration"
        }
        SettingsTab::Arrangements => "Save and restore window layouts",
        SettingsTab::AiInspector => "Assistant agent integration, panel settings, permissions",
        SettingsTab::Advanced => {
            "tmux integration, logging, file transfers, updates, debug logging"
        }
    }
}

impl SettingsUI {
    /// Get the current selected tab.
    pub fn selected_tab(&self) -> SettingsTab {
        self.selected_tab
    }

    /// Set the selected tab.
    pub fn set_selected_tab(&mut self, tab: SettingsTab) {
        self.selected_tab = tab;
    }
}
