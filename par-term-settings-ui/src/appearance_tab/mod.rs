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
