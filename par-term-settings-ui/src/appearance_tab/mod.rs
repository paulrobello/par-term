//! Appearance settings tab.
//!
//! Consolidates: theme_tab, font_tab, cursor_tab
//!
//! Contains:
//! - Theme selection
//! - Font settings (family, size, spacing, variants)
//! - Text shaping (ligatures, kerning)
//! - Font rendering options
//! - Cursor appearance and behavior
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `fonts_section.rs` | Theme, Auto Dark Mode, Fonts, Font Variants, Text Shaping, Font Rendering |
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
    fonts_section::show_text_shaping_section(ui, settings, changes_this_frame, collapsed);
    fonts_section::show_font_rendering_section(ui, settings, changes_this_frame, collapsed);
    cursor_section::show_cursor_section(ui, settings, changes_this_frame, collapsed);
    cursor_section::show_cursor_locks_section(ui, settings, changes_this_frame, collapsed);
    cursor_section::show_cursor_effects_section(ui, settings, changes_this_frame, collapsed);
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
        // Text shaping
        "text shaping",
        "shaping",
        "ligatures",
        "kerning",
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
    ]
}
