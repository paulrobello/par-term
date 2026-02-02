//! Vertical sidebar navigation for settings tabs.
//!
//! This component provides a vertical tab list on the left side of the settings UI,
//! replacing the previous horizontal tab bar for better organization.

use super::SettingsUI;

/// The available settings tabs in the reorganized UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Window,
    Input,
    Terminal,
    Effects,
    Notifications,
    Integrations,
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
            Self::Notifications => "Notifications",
            Self::Integrations => "Integrations",
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
            Self::Notifications => "🔔",
            Self::Integrations => "🔌",
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
            Self::Notifications,
            Self::Integrations,
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
    ui.add_space(8.0);

    for tab in SettingsTab::all() {
        let is_selected = *current_tab == *tab;

        // Check if this tab has any matches for the search query
        let has_matches = search_query.is_empty() || tab_matches_search(*tab, search_query);

        // Dim tabs that don't match search
        let text_color = if !has_matches {
            egui::Color32::from_rgb(80, 80, 80)
        } else if is_selected {
            egui::Color32::from_rgb(255, 255, 255)
        } else {
            egui::Color32::from_rgb(180, 180, 180)
        };

        let bg_color = if is_selected {
            egui::Color32::from_rgb(60, 60, 70)
        } else {
            egui::Color32::TRANSPARENT
        };

        // Create a selectable button-like widget
        let response = ui.add_sized(
            [140.0, 32.0],
            egui::Button::new(
                egui::RichText::new(format!("{} {}", tab.icon(), tab.display_name()))
                    .color(text_color),
            )
            .fill(bg_color)
            .stroke(if is_selected {
                egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 120))
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

    ui.add_space(8.0);

    tab_changed
}

/// Check if a tab matches the search query.
fn tab_matches_search(tab: SettingsTab, query: &str) -> bool {
    let query = query.to_lowercase();
    let keywords = tab_search_keywords(tab);

    // Check tab name
    if tab.display_name().to_lowercase().contains(&query) {
        return true;
    }

    // Check keywords
    keywords.iter().any(|k| k.to_lowercase().contains(&query))
}

/// Get search keywords for a tab.
fn tab_search_keywords(tab: SettingsTab) -> &'static [&'static str] {
    match tab {
        SettingsTab::Appearance => &[
            "theme",
            "font",
            "cursor",
            "color",
            "style",
            "blink",
            "ligatures",
            "kerning",
            "text shaping",
            "anti-alias",
            "hinting",
        ],
        SettingsTab::Window => &[
            "window",
            "title",
            "size",
            "columns",
            "rows",
            "padding",
            "opacity",
            "transparency",
            "blur",
            "tab bar",
            "tabs",
            "panes",
            "split",
            "decorations",
            "always on top",
            "fps",
            "vsync",
        ],
        SettingsTab::Input => &[
            "keyboard",
            "mouse",
            "keybindings",
            "shortcuts",
            "hotkey",
            "option",
            "alt",
            "meta",
            "selection",
            "clipboard",
            "copy",
            "paste",
            "scroll",
            "double-click",
        ],
        SettingsTab::Terminal => &[
            "shell",
            "scrollback",
            "search",
            "scrollbar",
            "unicode",
            "answerback",
            "login",
            "working directory",
            "initial text",
            "anti-idle",
        ],
        SettingsTab::Effects => &[
            "background",
            "shader",
            "image",
            "animation",
            "cursor shader",
            "trail",
            "glow",
            "cubemap",
            "channel",
            "texture",
        ],
        SettingsTab::Notifications => &[
            "bell",
            "notification",
            "activity",
            "silence",
            "sound",
            "visual",
            "desktop",
            "alert",
        ],
        SettingsTab::Integrations => &[
            "shell integration",
            "bash",
            "zsh",
            "fish",
            "shaders",
            "install",
            "uninstall",
            "bundle",
        ],
        SettingsTab::Advanced => &[
            "tmux",
            "logging",
            "session",
            "screenshot",
            "update",
            "version",
            "auto-attach",
        ],
    }
}

/// Get a summary of tab contents for tooltip.
fn tab_contents_summary(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::Appearance => "Theme, fonts, cursor style and colors",
        SettingsTab::Window => "Window size, opacity, tab bar, split panes",
        SettingsTab::Input => "Keyboard shortcuts, mouse behavior, clipboard",
        SettingsTab::Terminal => "Shell, scrollback, search, scrollbar",
        SettingsTab::Effects => "Background image/shader, cursor effects",
        SettingsTab::Notifications => "Bell, activity alerts, desktop notifications",
        SettingsTab::Integrations => "Shell integration, shader bundle installation",
        SettingsTab::Advanced => "tmux integration, logging, updates",
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
