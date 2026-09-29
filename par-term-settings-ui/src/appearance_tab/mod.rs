//! Appearance settings tab.
//!
//! Consolidates: theme_tab, font_tab, cursor_tab, badge_tab, progress_bar_tab
//!
//! Contains:
//! - Theme selection
//! - Font settings (family, size, spacing, variants)
//! - Font rendering options
//! - Cursor appearance and behavior
//! - Badge overlay settings
//! - Progress bar style, position, and colors
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `fonts_section.rs` | Theme, Auto Dark Mode, Fonts, Font Variants, Font Rendering |
//! | `cursor_section.rs` | Cursor, Cursor Locks, Cursor Effects |

use crate::SettingsUI;
use std::collections::HashSet;

mod cursor_section;
mod fonts_section;

/// Show the appearance tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    fonts_section::show_theme_section(ui, settings, changes_this_frame, collapsed);
    fonts_section::show_auto_dark_mode_section(ui, settings, changes_this_frame, collapsed);
    fonts_section::show_fonts_section(ui, settings, changes_this_frame, collapsed);
    fonts_section::show_font_variants_section(ui, settings, changes_this_frame, collapsed);
    fonts_section::show_font_rendering_section(ui, settings, changes_this_frame, collapsed);
    cursor_section::show_cursor_section(ui, settings, changes_this_frame, collapsed);
    cursor_section::show_cursor_locks_section(ui, settings, changes_this_frame, collapsed);
    cursor_section::show_cursor_effects_section(ui, settings, changes_this_frame, collapsed);
    // Badge and Progress Bar absorbed from their own tabs
    crate::badge_tab::show(ui, settings, changes_this_frame, collapsed);
    crate::progress_bar_tab::show(ui, settings, changes_this_frame, collapsed);
}

/// Search keywords for the Appearance settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        // Theme
        "theme",
        "color",
        "scheme",
        "dark",
        "light",
        // Auto dark mode
        "auto dark mode",
        "auto",
        "dark mode",
        "light mode",
        "system theme",
        "system appearance",
        "automatic",
        // Fonts
        "font",
        "family",
        "size",
        "bold",
        "italic",
        "line spacing",
        "char spacing",
        // Font rendering
        "anti-alias",
        "antialias",
        "hinting",
        "thin strokes",
        "smoothing",
        "minimum contrast",
        "contrast",
        // Cursor style
        "cursor",
        "style",
        "block",
        "beam",
        "underline",
        "blink",
        "interval",
        // Cursor appearance
        "cursor color",
        "text color",
        "unfocused cursor",
        "hollow",
        // Cursor locks
        "lock",
        "visibility",
        // Cursor effects
        "cursor guide",
        "guide",
        "cursor shadow",
        "shadow",
        "cursor boost",
        "boost",
        "glow",
        // Font variants
        "bold-italic",
        "bold italic",
        "font variant",
        "variant",
        // Badge (absorbed from badge_tab)
        "badge",
        "badge enabled",
        "badge format",
        "badge color",
        "badge opacity",
        "badge font",
        "margin",
        "top margin",
        "right margin",
        "max width",
        "max height",
        "variable",
        "session",
        "hostname",
        "username",
        "path",
        "overlay",
        "label",
        // Progress Bar (absorbed from progress_bar_tab)
        "progress",
        "progress bar",
        "bar",
        "percent",
        "osc 934",
        "osc 9;4",
        "indeterminate",
        "normal",
        "warning",
        "error",
        "bar height",
        "bar style",
        "bar position",
        "bar color",
        "opacity",
        // Section titles and section keywords
        "fonts",
        "font variants",
        "font fallback",
        "font rendering",
        "harfbuzz",
        "complex scripts",
        "opentype",
        "readability",
        "brightness",
        "hidpi",
        "retina",
        "color scheme",
        "preset",
        "colors",
        "cursor locks",
        "cursor effects",
        "cursor text color",
        "prevent applications",
        "horizontal line",
        "drop shadow",
        "shadow blur",
        "cursor row",
        "badge variables",
        "variables",
        "placement",
        "exit code",
    ]
}
