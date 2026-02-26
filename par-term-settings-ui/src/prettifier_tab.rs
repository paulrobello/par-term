//! Content Prettifier settings tab.
//!
//! Contains:
//! - Master enable/disable toggle with scope badge
//! - Detection settings (scope, confidence threshold)
//! - Per-renderer cards with enable/disable and priority
//! - Custom renderers section (add/edit/remove)
//! - Claude Code integration settings

use super::SettingsUI;
use super::section::collapsing_section;
use std::collections::HashSet;

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
        show_detection_section(ui, settings, changes_this_frame, collapsed);
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
        show_renderers_section(ui, settings, changes_this_frame, collapsed);
    }

    // Test detection section
    if section_matches(
        &query,
        "Test Detection",
        &["test", "detection", "sample", "detect"],
    ) {
        show_test_detection_section(ui, settings, collapsed);
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
        show_custom_renderers_section(ui, settings, changes_this_frame, collapsed);
    }

    // Claude Code integration section
    if section_matches(
        &query,
        "Claude Code",
        &["claude", "claude code", "auto detect", "badge", "expand"],
    ) {
        show_claude_code_section(ui, settings, changes_this_frame, collapsed);
    }

    // Clipboard settings section
    if section_matches(
        &query,
        "Clipboard",
        &["clipboard", "copy", "source", "rendered", "vi"],
    ) {
        show_clipboard_section(ui, settings, changes_this_frame, collapsed);
    }

    // Cache settings section
    if section_matches(&query, "Cache", &["cache", "max entries", "render cache"]) {
        show_cache_section(ui, settings, changes_this_frame, collapsed);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
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

// ============================================================================
// Detection Settings
// ============================================================================

fn show_detection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Detection Settings",
        "prettifier_detection",
        true,
        collapsed,
        |ui| {
            // Detection scope dropdown.
            ui.horizontal(|ui| {
                ui.label("Detection scope:");
                let scope = &mut settings.config.content_prettifier.detection.scope;
                let label = match scope.as_str() {
                    "command_output" => "Command Output",
                    "all" => "All Output",
                    "manual_only" => "Manual Only",
                    _ => "Command Output",
                };
                egui::ComboBox::from_id_salt("prettifier_detection_scope")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(scope == "command_output", "Command Output")
                            .clicked()
                        {
                            *scope = "command_output".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui.selectable_label(scope == "all", "All Output").clicked() {
                            *scope = "all".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_label(scope == "manual_only", "Manual Only")
                            .clicked()
                        {
                            *scope = "manual_only".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });

            // Confidence threshold slider.
            ui.horizontal(|ui| {
                ui.label("Confidence threshold:");
                let threshold = &mut settings
                    .config
                    .content_prettifier
                    .detection
                    .confidence_threshold;
                if ui
                    .add(egui::Slider::new(threshold, 0.0..=1.0).step_by(0.05))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Max scan lines.
            ui.horizontal(|ui| {
                ui.label("Max scan lines:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.detection.max_scan_lines,
                        )
                        .range(50..=5000)
                        .speed(10.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Debounce ms.
            ui.horizontal(|ui| {
                ui.label("Debounce (ms):");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.detection.debounce_ms,
                        )
                        .range(0..=1000)
                        .speed(10.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Per-block toggle.
            if ui
                .checkbox(
                    &mut settings.config.content_prettifier.per_block_toggle,
                    "Per-block source/rendered toggle",
                )
                .on_hover_text("Allow toggling between source and rendered view per content block")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Respect alternate screen.
            if ui
                .checkbox(
                    &mut settings.config.content_prettifier.respect_alternate_screen,
                    "Respect alternate screen",
                )
                .on_hover_text("Treat alternate screen transitions as content block boundaries")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

// ============================================================================
// Per-renderer Cards
// ============================================================================

fn show_renderers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Renderers",
        "prettifier_renderers",
        true,
        collapsed,
        |ui| {
            ui.label("Enable or disable individual format renderers and adjust their priority.");
            ui.add_space(4.0);

            // Helper macro to avoid repeating the same pattern for each renderer.
            show_renderer_toggle(
                ui,
                "Markdown",
                "MD",
                &mut settings.config.content_prettifier.renderers.markdown,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "JSON",
                "{}",
                &mut settings.config.content_prettifier.renderers.json,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "YAML",
                "YML",
                &mut settings.config.content_prettifier.renderers.yaml,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "TOML",
                "TML",
                &mut settings.config.content_prettifier.renderers.toml,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "XML",
                "XML",
                &mut settings.config.content_prettifier.renderers.xml,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "CSV",
                "CSV",
                &mut settings.config.content_prettifier.renderers.csv,
                &mut settings.has_changes,
                changes_this_frame,
            );

            // Diff renderer with extra display_mode option.
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut settings.config.content_prettifier.renderers.diff.enabled,
                        "Diff",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("+-")
                        .monospace()
                        .small()
                        .color(egui::Color32::from_rgb(100, 160, 255)),
                );
                ui.label("Priority:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.renderers.diff.priority,
                        )
                        .range(1..=100)
                        .speed(1.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            show_renderer_toggle(
                ui,
                "Log",
                "LOG",
                &mut settings.config.content_prettifier.renderers.log,
                &mut settings.has_changes,
                changes_this_frame,
            );

            // Diagrams renderer with extra engine option.
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut settings
                            .config
                            .content_prettifier
                            .renderers
                            .diagrams
                            .enabled,
                        "Diagrams",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("DG")
                        .monospace()
                        .small()
                        .color(egui::Color32::from_rgb(100, 160, 255)),
                );
                ui.label("Priority:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings
                                .config
                                .content_prettifier
                                .renderers
                                .diagrams
                                .priority,
                        )
                        .range(1..=100)
                        .speed(1.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Diagram engine selection (indented under diagrams).
            if settings
                .config
                .content_prettifier
                .renderers
                .diagrams
                .enabled
            {
                ui.indent("diagram_settings", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Engine:");
                        let current = settings
                            .config
                            .content_prettifier
                            .renderers
                            .diagrams
                            .engine
                            .clone()
                            .unwrap_or_else(|| "auto".to_string());
                        egui::ComboBox::from_id_salt("diagram_engine")
                            .selected_text(&current)
                            .show_ui(ui, |ui| {
                                for opt in &["auto", "kroki", "mermaid_cli", "text_fallback"] {
                                    if ui.selectable_label(current == *opt, *opt).clicked() {
                                        settings
                                            .config
                                            .content_prettifier
                                            .renderers
                                            .diagrams
                                            .engine = if *opt == "auto" {
                                            None
                                        } else {
                                            Some(opt.to_string())
                                        };
                                        settings.has_changes = true;
                                        *changes_this_frame = true;
                                    }
                                }
                            });
                    });

                    // Kroki server URL (only shown when engine is kroki).
                    let is_kroki = settings
                        .config
                        .content_prettifier
                        .renderers
                        .diagrams
                        .engine
                        .as_deref()
                        == Some("kroki");
                    if is_kroki {
                        ui.horizontal(|ui| {
                            ui.label("Kroki URL:");
                            let url = settings
                                .config
                                .content_prettifier
                                .renderers
                                .diagrams
                                .kroki_server
                                .get_or_insert_with(|| "https://kroki.io".to_string());
                            if ui
                                .add(egui::TextEdit::singleline(url).desired_width(250.0))
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });
                    }
                });
            }

            show_renderer_toggle(
                ui,
                "SQL Results",
                "SQL",
                &mut settings.config.content_prettifier.renderers.sql_results,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "Stack Trace",
                "STK",
                &mut settings.config.content_prettifier.renderers.stack_trace,
                &mut settings.has_changes,
                changes_this_frame,
            );
        },
    );
}

/// Show a single renderer toggle with enable checkbox and priority drag value.
fn show_renderer_toggle(
    ui: &mut egui::Ui,
    name: &str,
    badge: &str,
    toggle: &mut par_term_config::config::prettifier::RendererToggle,
    has_changes: &mut bool,
    changes_this_frame: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui.checkbox(&mut toggle.enabled, name).changed() {
            *has_changes = true;
            *changes_this_frame = true;
        }
        ui.label(
            egui::RichText::new(badge)
                .monospace()
                .small()
                .color(egui::Color32::from_rgb(100, 160, 255)),
        );
        ui.label("Priority:");
        if ui
            .add(
                egui::DragValue::new(&mut toggle.priority)
                    .range(1..=100)
                    .speed(1.0),
            )
            .changed()
        {
            *has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Custom Renderers
// ============================================================================

fn show_custom_renderers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Custom Renderers",
        "prettifier_custom_renderers",
        false,
        collapsed,
        |ui| {
            ui.label("User-defined renderers that pipe content to external commands.");
            ui.label(
                egui::RichText::new(
                    "ANSI color output from external commands is preserved automatically.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);

            let mut delete_index: Option<usize> = None;
            let count = settings.config.content_prettifier.custom_renderers.len();

            for i in 0..count {
                let cr = &settings.config.content_prettifier.custom_renderers[i];
                let header_text = format!("{} ({})", cr.name, cr.id);

                // Use egui::CollapsingHeader directly to avoid nested
                // collapsing_section borrow conflicts.
                egui::CollapsingHeader::new(egui::RichText::new(&header_text).strong())
                    .id_salt(format!("custom_renderer_{i}"))
                    .default_open(false)
                    .show(ui, |ui| {
                        let cr = &mut settings.config.content_prettifier.custom_renderers[i];

                        // ID field.
                        ui.horizontal(|ui| {
                            ui.label("ID:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cr.id)
                                        .desired_width(150.0)
                                        .hint_text("unique_id"),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Name field.
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cr.name)
                                        .desired_width(200.0)
                                        .hint_text("Display Name"),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Priority.
                        ui.horizontal(|ui| {
                            ui.label("Priority:");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut cr.priority)
                                        .range(1..=100)
                                        .speed(1.0),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Render command.
                        ui.horizontal(|ui| {
                            ui.label("Command:");
                            let mut cmd_text = cr.render_command.clone().unwrap_or_default();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cmd_text)
                                        .desired_width(250.0)
                                        .font(egui::TextStyle::Monospace)
                                        .hint_text("e.g. bat --color=always"),
                                )
                                .changed()
                            {
                                cr.render_command = if cmd_text.is_empty() {
                                    None
                                } else {
                                    Some(cmd_text)
                                };
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Render args.
                        ui.label("Arguments:");
                        let mut remove_arg: Option<usize> = None;
                        for (j, arg) in cr.render_args.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(arg)
                                            .desired_width(200.0)
                                            .font(egui::TextStyle::Monospace),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                                if ui.small_button("-").clicked() {
                                    remove_arg = Some(j);
                                }
                            });
                        }
                        if let Some(j) = remove_arg {
                            cr.render_args.remove(j);
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            if ui.small_button("+ Add Argument").clicked() {
                                cr.render_args.push(String::new());
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Detection patterns.
                        ui.add_space(4.0);
                        ui.label("Detection patterns (regex):");
                        let mut remove_pat: Option<usize> = None;
                        for (j, pat) in cr.detect_patterns.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(egui::RichText::new("/").monospace().weak());
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(pat)
                                            .desired_width(200.0)
                                            .font(egui::TextStyle::Monospace),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                                ui.label(egui::RichText::new("/").monospace().weak());
                                if ui.small_button("-").clicked() {
                                    remove_pat = Some(j);
                                }
                            });
                        }
                        if let Some(j) = remove_pat {
                            cr.detect_patterns.remove(j);
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            if ui.small_button("+ Add Pattern").clicked() {
                                cr.detect_patterns.push(String::new());
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        ui.add_space(4.0);
                        if ui
                            .small_button(
                                egui::RichText::new("Delete Renderer")
                                    .color(egui::Color32::from_rgb(200, 80, 80)),
                            )
                            .clicked()
                        {
                            delete_index = Some(i);
                        }
                    });
            }

            if let Some(i) = delete_index {
                settings
                    .config
                    .content_prettifier
                    .custom_renderers
                    .remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(4.0);

            if ui
                .button("+ Add Custom Renderer")
                .on_hover_text("Add a user-defined renderer with external command")
                .clicked()
            {
                settings.config.content_prettifier.custom_renderers.push(
                    par_term_config::config::prettifier::CustomRendererConfig {
                        id: format!(
                            "custom_{}",
                            settings.config.content_prettifier.custom_renderers.len()
                        ),
                        name: "New Renderer".to_string(),
                        detect_patterns: vec![],
                        render_command: None,
                        render_args: vec![],
                        priority: 50,
                    },
                );
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

// ============================================================================
// Claude Code Integration
// ============================================================================

fn show_claude_code_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Claude Code Integration",
        "prettifier_claude_code",
        true,
        collapsed,
        |ui| {
            let cc = &mut settings.config.content_prettifier.claude_code_integration;

            if ui
                .checkbox(&mut cc.auto_detect, "Auto-detect Claude Code sessions")
                .on_hover_text("Automatically detect when Claude Code is running")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut cc.render_markdown, "Render Markdown")
                .on_hover_text("Render markdown content in Claude Code output")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut cc.render_diffs, "Render Diffs")
                .on_hover_text("Render diff content in Claude Code output")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut cc.auto_render_on_expand,
                    "Auto-render on expand (Ctrl+O)",
                )
                .on_hover_text("Automatically render content when a collapsed block is expanded")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut cc.show_format_badges, "Show format badges")
                .on_hover_text("Show format badges (e.g., 'MD', 'JSON') on collapsed blocks")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

// ============================================================================
// Clipboard Settings
// ============================================================================

fn show_clipboard_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Clipboard Behavior",
        "prettifier_clipboard",
        false,
        collapsed,
        |ui| {
            let clip = &mut settings.config.content_prettifier.clipboard;

            ui.horizontal(|ui| {
                ui.label("Default copy:");
                egui::ComboBox::from_id_salt("prettifier_default_copy")
                    .selected_text(clip.default_copy.as_str())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(clip.default_copy == "rendered", "rendered")
                            .clicked()
                        {
                            clip.default_copy = "rendered".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_label(clip.default_copy == "source", "source")
                            .clicked()
                        {
                            clip.default_copy = "source".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });
        },
    );
}

// ============================================================================
// Cache Settings
// ============================================================================

fn show_cache_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Render Cache",
        "prettifier_cache",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Max cache entries:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.cache.max_entries,
                        )
                        .range(8..=512)
                        .speed(1.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

fn show_test_detection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Test Detection",
        "prettifier_test_detection",
        true,
        collapsed,
        |ui| {
            ui.label("Paste sample content to test which format the detector identifies:");
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("test_detection_content")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut settings.test_detection_content)
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("Paste sample output here…"),
                    );
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Preceding command (optional):");
                ui.add(
                    egui::TextEdit::singleline(&mut settings.test_detection_command)
                        .desired_width(200.0)
                        .hint_text("e.g. git diff"),
                );
            });

            ui.add_space(4.0);
            if ui
                .add_enabled(
                    !settings.test_detection_content.is_empty(),
                    egui::Button::new("Test Detection"),
                )
                .clicked()
            {
                settings.test_detection_requested = true;
            }

            // Display results
            if let Some((ref format_id, confidence, ref matched_rules, threshold)) =
                settings.test_detection_result
            {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if format_id.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 100, 100),
                        "No format detected.",
                    );
                } else {
                    let passed = confidence >= threshold;
                    let status = if passed { "PASS" } else { "BELOW THRESHOLD" };
                    let status_color = if passed {
                        egui::Color32::from_rgb(100, 200, 100)
                    } else {
                        egui::Color32::from_rgb(200, 180, 80)
                    };

                    egui::Grid::new("test_detection_results")
                        .num_columns(2)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Format:");
                            ui.strong(format_id);
                            ui.end_row();

                            ui.label("Confidence:");
                            ui.horizontal(|ui| {
                                ui.label(format!("{:.0}%", confidence * 100.0));
                                ui.colored_label(status_color, format!("[{}]", status));
                            });
                            ui.end_row();

                            ui.label("Threshold:");
                            ui.label(format!("{:.0}%", threshold * 100.0));
                            ui.end_row();

                            if !matched_rules.is_empty() {
                                ui.label("Matched rules:");
                                ui.label(matched_rules.join(", "));
                                ui.end_row();
                            }
                        });
                }
            }
        },
    );
}
