//! Theme and tab-style methods for `Config`.
//!
//! Covers:
//! - Loading the active theme (`load_theme`)
//! - Applying system dark/light theme (`apply_system_theme`)
//! - Applying tab-style presets (`apply_tab_style`, `apply_system_tab_style`)

use super::config_struct::Config;
use crate::themes::Theme;
use crate::types::TabStyle;

impl Config {
    /// Apply tab style preset, overwriting the tab bar color/size fields.
    ///
    /// This is called when the user changes `tab_style` in settings.
    /// The `Dark` style corresponds to the existing defaults and does nothing.
    pub fn apply_tab_style(&mut self) {
        match self.tab_style {
            TabStyle::Dark => {
                // Default dark theme - restore original defaults
                self.tab_bar_background = crate::defaults::tab_bar_background();
                self.tab_active_background = crate::defaults::tab_active_background();
                self.tab_inactive_background = crate::defaults::tab_inactive_background();
                self.tab_hover_background = crate::defaults::tab_hover_background();
                self.tab_active_text = crate::defaults::tab_active_text();
                self.tab_inactive_text = crate::defaults::tab_inactive_text();
                self.tab_active_indicator = crate::defaults::tab_active_indicator();
                self.tab_border_color = crate::defaults::tab_border_color();
                self.tab_border_width = crate::defaults::tab_border_width();
                self.tab_bar_height = crate::defaults::tab_bar_height();
            }
            TabStyle::Light => {
                self.tab_bar_background = [235, 235, 235];
                self.tab_active_background = [255, 255, 255];
                self.tab_inactive_background = [225, 225, 225];
                self.tab_hover_background = [240, 240, 240];
                self.tab_active_text = [30, 30, 30];
                self.tab_inactive_text = [100, 100, 100];
                self.tab_active_indicator = [50, 120, 220];
                self.tab_border_color = [200, 200, 200];
                self.tab_border_width = 1.0;
                self.tab_bar_height = crate::defaults::tab_bar_height();
            }
            TabStyle::Compact => {
                // Smaller tabs, tighter spacing
                self.tab_bar_background = [35, 35, 35];
                self.tab_active_background = [55, 55, 55];
                self.tab_inactive_background = [35, 35, 35];
                self.tab_hover_background = [45, 45, 45];
                self.tab_active_text = [240, 240, 240];
                self.tab_inactive_text = [160, 160, 160];
                self.tab_active_indicator = [80, 140, 240];
                self.tab_border_color = [60, 60, 60];
                self.tab_border_width = 0.5;
                self.tab_bar_height = 22.0;
            }
            TabStyle::Minimal => {
                // Very clean, flat look with minimal decoration
                self.tab_bar_background = [30, 30, 30];
                self.tab_active_background = [30, 30, 30];
                self.tab_inactive_background = [30, 30, 30];
                self.tab_hover_background = [40, 40, 40];
                self.tab_active_text = [255, 255, 255];
                self.tab_inactive_text = [120, 120, 120];
                self.tab_active_indicator = [100, 150, 255];
                self.tab_border_color = [30, 30, 30]; // No visible border
                self.tab_border_width = 0.0;
                self.tab_bar_height = 26.0;
            }
            TabStyle::HighContrast => {
                // Maximum contrast for accessibility
                self.tab_bar_background = [0, 0, 0];
                self.tab_active_background = [255, 255, 255];
                self.tab_inactive_background = [30, 30, 30];
                self.tab_hover_background = [60, 60, 60];
                self.tab_active_text = [0, 0, 0];
                self.tab_inactive_text = [255, 255, 255];
                self.tab_active_indicator = [255, 255, 0];
                self.tab_border_color = [255, 255, 255];
                self.tab_border_width = 2.0;
                self.tab_bar_height = 30.0;
            }
            TabStyle::Automatic => {
                // No-op here: actual style is resolved and applied by apply_system_tab_style()
            }
        }
    }

    /// Load theme configuration
    pub fn load_theme(&self) -> Theme {
        Theme::by_name(&self.theme).unwrap_or_default()
    }

    /// Apply system theme if auto_dark_mode is enabled.
    /// Returns true if the theme was changed.
    pub fn apply_system_theme(&mut self, is_dark: bool) -> bool {
        if !self.auto_dark_mode {
            return false;
        }
        let new_theme = if is_dark {
            &self.dark_theme
        } else {
            &self.light_theme
        };
        if self.theme != *new_theme {
            self.theme = new_theme.clone();
            true
        } else {
            false
        }
    }

    /// Apply tab style based on system theme when tab_style is Automatic.
    /// Returns true if the style was applied.
    pub fn apply_system_tab_style(&mut self, is_dark: bool) -> bool {
        if self.tab_style != TabStyle::Automatic {
            return false;
        }
        let target = if is_dark {
            self.dark_tab_style
        } else {
            self.light_tab_style
        };
        // Temporarily set to concrete style, apply colors, then restore Automatic
        self.tab_style = target;
        self.apply_tab_style();
        self.tab_style = TabStyle::Automatic;
        true
    }
}
