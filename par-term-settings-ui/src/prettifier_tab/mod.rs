//! Content Prettifier settings tab.
//!
//! Contains:
//! - Master enable/disable toggle with scope badge
//! - Detection settings (scope, confidence threshold)
//! - Per-renderer cards with enable/disable and priority
//! - Custom renderers section (add/edit/remove)
//! - Claude Code integration settings

use super::SettingsUI;
use super::section::section_matches;
use std::collections::HashSet;

mod cache;
mod claude_code;
mod clipboard;
mod custom_renderers;
mod detection;
mod renderers;
mod test_detection;

/// Show the Content Prettifier tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Master toggle section
    if section_matches(
        &query,
        "Prettifier",
        &["prettifier", "prettify", "enable", "toggle", "detect"],
    ) {
        show_master_toggle(ui, settings, changes_this_frame);
        ui.add_space(8.0);
    }

    // Detection settings section
    if section_matches(
        &query,
        "Detection",
        &[
            "detection",
            "scope",
            "confidence",
            "threshold",
            "scan",
            "debounce",
        ],
    ) {
        detection::show_detection_section(ui, settings, changes_this_frame, collapsed);
    }

    // Per-renderer settings section
    if section_matches(
        &query,
        "Renderers",
        &[
            "renderer",
            "markdown",
            "json",
            "yaml",
            "toml",
            "xml",
            "csv",
            "diff",
            "log",
            "diagram",
            "sql",
            "stack trace",
            "priority",
        ],
    ) {
        renderers::show_renderers_section(ui, settings, changes_this_frame, collapsed);
    }

    // Test detection section
    if section_matches(
        &query,
        "Test Detection",
        &["test", "detection", "sample", "detect"],
    ) {
        test_detection::show_test_detection_section(ui, settings, collapsed);
    }

    // Custom renderers section
    if section_matches(
        &query,
        "Custom Renderers",
        &[
            "custom",
            "external",
            "command",
            "user-defined",
            "user defined",
        ],
    ) {
        custom_renderers::show_custom_renderers_section(
            ui,
            settings,
            changes_this_frame,
            collapsed,
        );
    }

    // Claude Code integration section
    if section_matches(
        &query,
        "Claude Code",
        &["claude", "claude code", "auto detect", "badge", "expand"],
    ) {
        claude_code::show_claude_code_section(ui, settings, changes_this_frame, collapsed);
    }

    // Clipboard settings section
    if section_matches(
        &query,
        "Clipboard",
        &["clipboard", "copy", "source", "rendered", "vi"],
    ) {
        clipboard::show_clipboard_section(ui, settings, changes_this_frame, collapsed);
    }

    // Cache settings section
    if section_matches(&query, "Cache", &["cache", "max entries", "render cache"]) {
        cache::show_cache_section(ui, settings, changes_this_frame, collapsed);
    }
}

// ============================================================================
// Master Toggle
// ============================================================================

fn show_master_toggle(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.horizontal(|ui| {
        if ui
            .checkbox(
                &mut settings.config.enable_prettifier,
                egui::RichText::new("Enable Prettifier").strong(),
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
        ui.label(
            egui::RichText::new("[Global]")
                .small()
                .color(egui::Color32::from_rgb(100, 160, 255)),
        );
    });

    // Build dynamic subtitle listing enabled renderers.
    let renderers = &settings.config.content_prettifier.renderers;
    let mut enabled_formats = Vec::new();
    if renderers.markdown.enabled {
        enabled_formats.push("Markdown");
    }
    if renderers.json.enabled {
        enabled_formats.push("JSON");
    }
    if renderers.yaml.enabled {
        enabled_formats.push("YAML");
    }
    if renderers.toml.enabled {
        enabled_formats.push("TOML");
    }
    if renderers.xml.enabled {
        enabled_formats.push("XML");
    }
    if renderers.csv.enabled {
        enabled_formats.push("CSV");
    }
    if renderers.diff.enabled {
        enabled_formats.push("Diff");
    }
    if renderers.log.enabled {
        enabled_formats.push("Log");
    }
    if renderers.diagrams.enabled {
        enabled_formats.push("Diagrams");
    }
    if renderers.sql_results.enabled {
        enabled_formats.push("SQL");
    }
    if renderers.stack_trace.enabled {
        enabled_formats.push("Stack Trace");
    }

    if !enabled_formats.is_empty() {
        let subtitle = format!(
            "Automatically detects and renders structured content including {}.",
            enabled_formats.join(", ")
        );
        ui.label(egui::RichText::new(subtitle).small().weak());
    }

    // Global toggle keybinding display.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Global toggle:")
                .small()
                .color(egui::Color32::GRAY),
        );
        ui.label(
            egui::RichText::new(&settings.config.content_prettifier.global_toggle_key)
                .small()
                .monospace()
                .color(egui::Color32::from_rgb(180, 180, 100)),
        );
    });
}

/// Search keywords for the Content Prettifier settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "prettifier",
        "prettify",
        "pretty",
        "content",
        "detect",
        "detection",
        "render",
        "markdown",
        "json",
        "yaml",
        "toml",
        "xml",
        "csv",
        "diff",
        "diagram",
        "mermaid",
        "log",
        "stack trace",
        "confidence",
        "gutter",
        "badge",
        "toggle",
        "source",
        "rendered",
        "custom",
        "claude code",
        "external command",
        "test detection",
        "sample",
        // Diagram engine
        "engine",
        "kroki",
        "native",
        "text fallback",
        // Display options
        "alternate screen",
        "per-block",
        "per block",
        "block",
        // Clipboard
        "clipboard",
        "copy",
        // Cache
        "cache",
        "max entries",
        // Detection tuning
        "scope",
        "threshold",
        "debounce",
        "scan",
    ]
}
