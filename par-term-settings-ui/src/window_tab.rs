//! Window settings tab.
//!
//! Consolidates: window_tab (original), tab_bar_tab, panes_tab
//!
//! Contains:
//! - Display settings (title, dimensions, padding)
//! - Transparency settings (opacity, blur)
//! - Performance settings (FPS, VSync, power saving)
//! - Window behavior (decorations, always on top, etc.)
//! - Tab bar settings
//! - Split panes settings

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::{
    DividerStyle, PaneTitlePosition, PowerPreference, TabBarMode, TabBarPosition, TabStyle,
    VsyncMode, WindowType,
};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the window tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Display section
    if section_matches(
        &query,
        "Display",
        &[
            "title",
            "columns",
            "rows",
            "padding",
            "size",
            "window title",
            "allow title change",
        ],
    ) {
        show_display_section(ui, settings, changes_this_frame, collapsed);
    }

    // Transparency section
    if section_matches(
        &query,
        "Transparency",
        &[
            "opacity",
            "blur",
            "transparent",
            "background",
            "default background",
            "text opaque",
        ],
    ) {
        show_transparency_section(ui, settings, changes_this_frame, collapsed);
    }

    // Performance section (collapsed by default)
    if section_matches(
        &query,
        "Performance",
        &[
            "fps",
            "vsync",
            "refresh",
            "power",
            "unfocused",
            "gpu",
            "flicker",
            "reduce",
            "throughput",
            "render interval",
            "batch",
            "mailbox",
            "fifo",
            "gpu preference",
            "power saving",
        ],
    ) {
        show_performance_section(ui, settings, changes_this_frame, collapsed);
    }

    // Window Behavior section (collapsed by default)
    if section_matches(
        &query,
        "Window Behavior",
        &[
            "decorations",
            "always on top",
            "window type",
            "monitor",
            "lock",
            "edge-anchored",
            "primary monitor",
            "window number",
        ],
    ) {
        show_behavior_section(ui, settings, changes_this_frame, collapsed);
    }

    // Tab Bar section
    if section_matches(
        &query,
        "Tab Bar",
        &[
            "tab",
            "tabs",
            "bar",
            "index",
            "close button",
            "profile drawer",
            "stretch",
            "html titles",
            "inherit directory",
            "max tabs",
        ],
    ) {
        show_tab_bar_section(ui, settings, changes_this_frame, collapsed);
    }

    // Tab Bar Appearance section (collapsed by default)
    if section_matches(
        &query,
        "Tab Bar Appearance",
        &[
            "tab color",
            "tab border",
            "inactive tab",
            "dimming",
            "tab style",
            "minimum tab width",
            "active indicator",
            "activity indicator",
            "bell indicator",
        ],
    ) {
        show_tab_bar_appearance_section(ui, settings, changes_this_frame, collapsed);
    }

    // Split Panes section
    if section_matches(
        &query,
        "Split Panes",
        &[
            "pane",
            "split",
            "divider",
            "focus indicator",
            "hit width",
            "drag area",
            "max panes",
            "min pane size",
            "pane padding",
        ],
    ) {
        show_panes_section(ui, settings, changes_this_frame, collapsed);
    }

    // Pane Appearance section (collapsed by default)
    if section_matches(
        &query,
        "Pane Appearance",
        &[
            "pane color",
            "pane title",
            "inactive pane",
            "pane opacity",
            "hover color",
            "dim inactive",
            "title height",
            "title position",
            "pane background",
        ],
    ) {
        show_pane_appearance_section(ui, settings, changes_this_frame, collapsed);
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
// Display Section
// ============================================================================

fn show_display_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Display", "window_display", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Title:");
            if ui
                .text_edit_singleline(&mut settings.config.window_title)
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.allow_title_change,
                "Allow apps to change window title",
            )
            .on_hover_text(
                "When enabled, terminal applications can change the window title via OSC escape sequences",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Columns:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.cols, 40..=300),
                )
                .on_hover_text("Number of columns in the terminal grid (determines window width)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Rows:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.rows, 10..=100),
                )
                .on_hover_text("Number of rows in the terminal grid (determines window height)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Show current size and button to use it
        ui.horizontal(|ui| {
            let current_size = format!(
                "Current: {}x{}",
                settings.current_cols, settings.current_rows
            );
            ui.label(&current_size);

            // Show button (disabled if sizes already match)
            let differs = settings.current_cols != settings.config.cols
                || settings.current_rows != settings.config.rows;
            if ui
                .add_enabled(differs, egui::Button::new("Use Current Size"))
                .on_hover_text(if differs {
                    "Set the configured columns and rows to match the current window size"
                } else {
                    "Config already matches current window size"
                })
                .clicked()
            {
                settings.config.cols = settings.current_cols;
                settings.config.rows = settings.current_rows;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Padding:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.window_padding, 0.0..=50.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.hide_window_padding_on_split,
                "Hide padding on split",
            )
            .on_hover_text(
                "Automatically remove window padding when panes are split (panes have their own padding)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Transparency Section
// ============================================================================

fn show_transparency_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Transparency",
        "window_transparency",
        true,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                let response = ui.add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.window_opacity, 0.1..=1.0),
                );
                if response.changed() {
                    log::info!(
                        "Opacity slider changed to: {}",
                        settings.config.window_opacity
                    );
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
            .checkbox(
                &mut settings.config.transparency_affects_only_default_background,
                "Transparency affects only default background",
            )
            .on_hover_text(
                "When enabled, colored backgrounds (syntax highlighting, status bars) remain opaque for better readability",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            if ui
            .checkbox(&mut settings.config.keep_text_opaque, "Keep text opaque")
            .on_hover_text(
                "When enabled, text is always rendered at full opacity regardless of window transparency",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            // Blur settings (macOS only)
            #[cfg(target_os = "macos")]
            {
                ui.add_space(8.0);

                if ui
                .checkbox(&mut settings.config.blur_enabled, "Enable window blur")
                .on_hover_text(
                    "Blur content behind the transparent window for better readability (requires transparency)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

                if settings.config.blur_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Blur radius:");
                        // Convert u32 to i32 for slider, clamp to valid range
                        let mut radius_i32 = settings.config.blur_radius.min(64) as i32;
                        if ui
                            .add(egui::Slider::new(&mut radius_i32, 1..=64))
                            .on_hover_text("Blur intensity (higher = more blur)")
                            .changed()
                        {
                            settings.config.blur_radius = radius_i32 as u32;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                }
            }
        },
    );
}

// ============================================================================
// Performance Section
// ============================================================================

fn show_performance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Performance",
        "window_performance",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Max FPS:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.max_fps, 1..=240),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("VSync Mode:");
                let current = match settings.config.vsync_mode {
                    VsyncMode::Immediate => 0,
                    VsyncMode::Mailbox => 1,
                    VsyncMode::Fifo => 2,
                };
                let mut selected = current;

                // Helper to format mode name with support indicator
                let format_mode = |mode: VsyncMode, name: &str| -> String {
                    if settings.is_vsync_mode_supported(mode) {
                        name.to_string()
                    } else {
                        format!("{} (not supported)", name)
                    }
                };

                egui::ComboBox::from_id_salt("window_vsync_mode")
                    .selected_text(match current {
                        0 => format_mode(VsyncMode::Immediate, "Immediate (No VSync)"),
                        1 => format_mode(VsyncMode::Mailbox, "Mailbox (Balanced)"),
                        2 => format_mode(VsyncMode::Fifo, "FIFO (VSync)"),
                        _ => "Unknown".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected,
                            0,
                            format_mode(VsyncMode::Immediate, "Immediate (No VSync)"),
                        );
                        ui.selectable_value(
                            &mut selected,
                            1,
                            format_mode(VsyncMode::Mailbox, "Mailbox (Balanced)"),
                        );
                        ui.selectable_value(
                            &mut selected,
                            2,
                            format_mode(VsyncMode::Fifo, "FIFO (VSync)"),
                        );
                    });
                if selected != current {
                    let new_mode = match selected {
                        0 => VsyncMode::Immediate,
                        1 => VsyncMode::Mailbox,
                        2 => VsyncMode::Fifo,
                        _ => VsyncMode::Immediate,
                    };

                    // Check if the mode is supported
                    if settings.is_vsync_mode_supported(new_mode) {
                        settings.config.vsync_mode = new_mode;
                        settings.vsync_warning = None;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    } else {
                        // Set warning and revert to Fifo (always supported)
                        settings.vsync_warning = Some(format!(
                            "{:?} is not supported on this display. Using FIFO instead.",
                            new_mode
                        ));
                        settings.config.vsync_mode = VsyncMode::Fifo;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                }
            });

            // Show vsync warning if present
            if let Some(ref warning) = settings.vsync_warning {
                ui.colored_label(egui::Color32::YELLOW, warning);
            }

            ui.horizontal(|ui| {
                ui.label("GPU Power Preference:");
                let current_pref = settings.config.power_preference;
                egui::ComboBox::from_id_salt("gpu_power_preference")
                    .selected_text(current_pref.display_name())
                    .show_ui(ui, |ui| {
                        for pref in PowerPreference::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.power_preference,
                                    *pref,
                                    pref.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });
            ui.colored_label(
                egui::Color32::GRAY,
                "Note: Requires app restart to take effect",
            );

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Power Saving").strong());

            if ui
                .checkbox(
                    &mut settings.config.pause_shaders_on_blur,
                    "Pause shader animations when unfocused",
                )
                .on_hover_text(
                    "Reduces GPU usage by pausing animated shaders when the window is not in focus",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
            .checkbox(
                &mut settings.config.pause_refresh_on_blur,
                "Reduce refresh rate when unfocused",
            )
            .on_hover_text(
                "Reduces CPU/GPU usage by lowering the frame rate when the window is not in focus",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            ui.horizontal(|ui| {
                ui.label("Unfocused FPS:");
                if ui
                    .add_enabled(
                        settings.config.pause_refresh_on_blur,
                        egui::Slider::new(&mut settings.config.unfocused_fps, 1..=30),
                    )
                    .on_hover_text(
                        "Target frame rate when window is unfocused (lower = more power savings)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Flicker Reduction").strong());

            if ui
                .checkbox(
                    &mut settings.config.reduce_flicker,
                    "Reduce flicker during fast updates",
                )
                .on_hover_text(
                    "Delays screen redraws while the cursor is hidden (DECTCEM off).\n\
                 Many terminal programs hide the cursor during bulk updates.\n\
                 This batches updates to reduce visual flicker.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Maximum delay:");
                if ui
                    .add_enabled(
                        settings.config.reduce_flicker,
                        egui::Slider::new(&mut settings.config.reduce_flicker_delay_ms, 1..=100)
                            .suffix("ms"),
                    )
                    .on_hover_text(
                        "Maximum time to wait for cursor to become visible.\n\
                     Lower = more responsive, Higher = smoother for slow programs.\n\
                     Default: 16ms (~1 frame at 60fps)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(10.0);
            ui.label(egui::RichText::new("Throughput Mode").strong());

            if ui
                .checkbox(&mut settings.config.maximize_throughput, {
                    #[cfg(target_os = "macos")]
                    {
                        "Maximize throughput (Cmd+Shift+T)"
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        "Maximize throughput (Ctrl+Shift+M)"
                    }
                })
                .on_hover_text(
                    "Batches screen updates during bulk terminal output.\n\
                 Reduces CPU overhead when processing large outputs.\n\
                 Trade-off: display updates are delayed by the interval below.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Render interval:");
                if ui
                    .add_enabled(
                        settings.config.maximize_throughput,
                        egui::Slider::new(
                            &mut settings.config.throughput_render_interval_ms,
                            50..=500,
                        )
                        .suffix("ms"),
                    )
                    .on_hover_text(
                        "How often to update the display in throughput mode.\n\
                     Lower = more responsive, Higher = better throughput.\n\
                     Default: 100ms (~10 updates/sec)",
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

// ============================================================================
// Window Behavior Section
// ============================================================================

fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Window Behavior",
        "window_behavior",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.window_decorations,
                    "Window decorations",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.window_always_on_top, "Always on top")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.lock_window_size, "Lock window size")
                .on_hover_text("Prevent window from being resized by the user")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.show_window_number,
                    "Show window number in title",
                )
                .on_hover_text(
                    "Display window index number in the title bar (useful for multiple windows)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            // Window type dropdown
            ui.horizontal(|ui| {
                ui.label("Window type:");
                let current_type = settings.config.window_type;
                egui::ComboBox::from_id_salt("window_window_type")
                    .selected_text(current_type.display_name())
                    .show_ui(ui, |ui| {
                        for window_type in WindowType::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.window_type,
                                    *window_type,
                                    window_type.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Target monitor setting
            ui.horizontal(|ui| {
                ui.label("Target monitor:");
                let mut monitor_index = settings.config.target_monitor.unwrap_or(0) as i32;
                let mut use_default = settings.config.target_monitor.is_none();

                if ui
                    .checkbox(&mut use_default, "Auto")
                    .on_hover_text("Let the OS decide which monitor to open on")
                    .changed()
                {
                    if use_default {
                        settings.config.target_monitor = None;
                    } else {
                        settings.config.target_monitor = Some(0);
                    }
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !use_default
                    && ui
                        .add(egui::Slider::new(&mut monitor_index, 0..=7))
                        .on_hover_text("Monitor index (0 = primary)")
                        .changed()
                {
                    settings.config.target_monitor = Some(monitor_index as usize);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if settings.config.window_type.is_edge() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Note: Edge-anchored windows take effect on next window creation",
                );
            }

            // Target macOS Space setting (only visible on macOS)
            if cfg!(target_os = "macos") {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Target Space:");
                    let mut space_number = settings.config.target_space.unwrap_or(1) as i32;
                    let mut use_default = settings.config.target_space.is_none();

                    if ui
                        .checkbox(&mut use_default, "Auto")
                        .on_hover_text("Let the OS decide which Space (virtual desktop) to open on")
                        .changed()
                    {
                        if use_default {
                            settings.config.target_space = None;
                        } else {
                            settings.config.target_space = Some(1);
                        }
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    if !use_default
                        && ui
                            .add(egui::Slider::new(&mut space_number, 1..=16))
                            .on_hover_text("Space number in Mission Control (1 = first Space)")
                            .changed()
                    {
                        settings.config.target_space = Some(space_number as u32);
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Note: Target Space takes effect on next window creation",
                );
            }
        },
    );
}

// ============================================================================
// Tab Bar Section
// ============================================================================

fn show_tab_bar_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Tab Bar", "window_tab_bar", true, collapsed, |ui| {
        // Tab style preset dropdown
        ui.horizontal(|ui| {
            ui.label("Tab style:");
            let current_style = settings.config.tab_style;
            egui::ComboBox::from_id_salt("window_tab_style")
                .selected_text(current_style.display_name())
                .show_ui(ui, |ui| {
                    for style in TabStyle::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.tab_style,
                                *style,
                                style.display_name(),
                            )
                            .changed()
                        {
                            settings.config.apply_tab_style();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Show light/dark sub-style dropdowns when Automatic is selected
        if settings.config.tab_style == TabStyle::Automatic {
            ui.indent("auto_tab_style_indent", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Light tab style:");
                    let current = settings.config.light_tab_style;
                    egui::ComboBox::from_id_salt("window_light_tab_style")
                        .selected_text(current.display_name())
                        .show_ui(ui, |ui| {
                            for style in TabStyle::all_concrete() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.light_tab_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Dark tab style:");
                    let current = settings.config.dark_tab_style;
                    egui::ComboBox::from_id_salt("window_dark_tab_style")
                        .selected_text(current.display_name())
                        .show_ui(ui, |ui| {
                            for style in TabStyle::all_concrete() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.dark_tab_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
            });
        }

        ui.horizontal(|ui| {
            ui.label("Show tab bar:");
            let current = match settings.config.tab_bar_mode {
                TabBarMode::Always => 0,
                TabBarMode::WhenMultiple => 1,
                TabBarMode::Never => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("window_tab_bar_mode")
                .selected_text(match current {
                    0 => "Always",
                    1 => "When multiple tabs",
                    2 => "Never",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Always");
                    ui.selectable_value(&mut selected, 1, "When multiple tabs");
                    ui.selectable_value(&mut selected, 2, "Never");
                });
            if selected != current {
                settings.config.tab_bar_mode = match selected {
                    0 => TabBarMode::Always,
                    1 => TabBarMode::WhenMultiple,
                    2 => TabBarMode::Never,
                    _ => TabBarMode::WhenMultiple,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Position:");
            let current_position = settings.config.tab_bar_position;
            egui::ComboBox::from_id_salt("window_tab_bar_position")
                .selected_text(current_position.display_name())
                .show_ui(ui, |ui| {
                    for &pos in TabBarPosition::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.tab_bar_position,
                                pos,
                                pos.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Show tab bar width slider only for Left position
        if settings.config.tab_bar_position == TabBarPosition::Left {
            ui.horizontal(|ui| {
                ui.label("Tab bar width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_bar_width, 100.0..=300.0)
                            .step_by(1.0)
                            .suffix("px"),
                    )
                    .on_hover_text("Width of the left tab bar panel")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Tab bar height:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.tab_bar_height, 20.0..=50.0)
                        .step_by(1.0)
                        .suffix("px"),
                )
                .on_hover_text("Height of the tab bar in pixels")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.tab_show_index,
                "Show tab index numbers",
            )
            .on_hover_text("Display tab numbers (1, 2, 3...) in tab titles for keyboard navigation")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.tab_show_close_button,
                "Show close button on tabs",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.tab_stretch_to_fill,
                "Stretch tabs to fill bar",
            )
            .on_hover_text("Make tabs share available width evenly when they fit without scrolling")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(&mut settings.config.tab_html_titles, "HTML tab titles")
            .on_hover_text(
                "Render limited HTML in tab titles: <b>, <i>, <u>, <span style=\"color:...\">",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut settings.config.tab_inherit_cwd,
                "New tabs inherit current directory",
            )
            .on_hover_text("When opening a new tab, start in the same directory as the current tab")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.show_profile_drawer_button,
                "Show profile drawer button",
            )
            .on_hover_text(
                "Show the profile drawer toggle button on the right edge of the terminal window",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.new_tab_shortcut_shows_profiles,
                "New tab shortcut shows profile picker",
            )
            .on_hover_text(
                "When enabled, the new tab keyboard shortcut (Cmd+T / Ctrl+Shift+T) shows a profile selection dropdown instead of immediately creating a default tab",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Maximum tabs:");
            // Convert usize to u32 for slider
            let mut max_tabs = settings.config.max_tabs as u32;
            if ui
                .add(egui::Slider::new(&mut max_tabs, 0..=50))
                .on_hover_text("Maximum number of tabs allowed (0 = unlimited)")
                .changed()
            {
                settings.config.max_tabs = max_tabs as usize;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if settings.config.max_tabs == 0 {
                ui.label("(unlimited)");
            }
        });
    });
}

// ============================================================================
// Tab Bar Appearance Section
// ============================================================================

fn show_tab_bar_appearance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Tab Bar Appearance",
        "window_tab_bar_appearance",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Minimum tab width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_min_width, 120.0..=512.0)
                            .step_by(1.0)
                            .suffix("px"),
                    )
                    .on_hover_text("Minimum width for tabs before horizontal scrolling is enabled")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Border").strong());

            ui.horizontal(|ui| {
                ui.label("Border width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_border_width, 0.0..=3.0)
                            .step_by(0.5)
                            .suffix("px"),
                    )
                    .on_hover_text("Width of the border around each tab (0 = no border)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Border color:");
                let mut color = settings.config.tab_border_color;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_border_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
                .checkbox(
                    &mut settings.config.tab_inactive_outline_only,
                    "Inactive tabs outline only",
                )
                .on_hover_text("Render inactive tabs with just an outline border and no fill")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Inactive Tab Dimming").strong());

            if ui
                .checkbox(&mut settings.config.dim_inactive_tabs, "Dim inactive tabs")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.dim_inactive_tabs {
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    if ui
                        .add(
                            egui::Slider::new(&mut settings.config.inactive_tab_opacity, 0.2..=1.0)
                                .step_by(0.05),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Background Colors").strong());

            ui.horizontal(|ui| {
                ui.label("Tab bar background:");
                let mut color = settings.config.tab_bar_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_bar_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Active tab:");
                let mut color = settings.config.tab_active_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_active_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Inactive tab:");
                let mut color = settings.config.tab_inactive_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_inactive_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Hovered tab:");
                let mut color = settings.config.tab_hover_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_hover_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Text Colors").strong());

            ui.horizontal(|ui| {
                ui.label("Active tab text:");
                let mut color = settings.config.tab_active_text;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_active_text = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Inactive tab text:");
                let mut color = settings.config.tab_inactive_text;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_inactive_text = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Indicator Colors").strong());

            ui.horizontal(|ui| {
                ui.label("Active tab border:");
                let mut color = settings.config.tab_active_indicator;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_active_indicator = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Activity indicator:");
                let mut color = settings.config.tab_activity_indicator;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_activity_indicator = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Bell indicator:");
                let mut color = settings.config.tab_bell_indicator;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_bell_indicator = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Close Button Colors").strong());

            ui.horizontal(|ui| {
                ui.label("Close button:");
                let mut color = settings.config.tab_close_button;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_close_button = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Close button hover:");
                let mut color = settings.config.tab_close_button_hover;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_close_button_hover = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

// ============================================================================
// Split Panes Section
// ============================================================================

fn show_panes_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Split Panes", "window_panes", true, collapsed, |ui| {
        ui.label("Configure split pane behavior and appearance");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Dividers").strong());

        ui.horizontal(|ui| {
            ui.label("Divider Width:");
            let mut width = settings.config.pane_divider_width.unwrap_or(2.0);
            if ui
                .add(egui::Slider::new(&mut width, 1.0..=10.0).suffix(" px"))
                .on_hover_text("Visual width of dividers between panes")
                .changed()
            {
                settings.config.pane_divider_width = Some(width);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Drag Hit Width:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.pane_divider_hit_width, 4.0..=20.0)
                        .suffix(" px"),
                )
                .on_hover_text("Width of the drag area for resizing (larger = easier to grab)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Pane Padding:");
            if ui
                .add(egui::Slider::new(&mut settings.config.pane_padding, 0.0..=20.0).suffix(" px"))
                .on_hover_text("Padding inside panes (space between content and border/divider)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Divider Style:");
            let current_style = settings.config.pane_divider_style;
            egui::ComboBox::from_id_salt("pane_divider_style")
                .selected_text(current_style.display_name())
                .show_ui(ui, |ui| {
                    for style in DividerStyle::ALL {
                        if ui
                            .selectable_value(
                                &mut settings.config.pane_divider_style,
                                *style,
                                style.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Focus Indicator").strong());

        if ui
            .checkbox(
                &mut settings.config.pane_focus_indicator,
                "Show focus indicator",
            )
            .on_hover_text("Draw a border around the focused pane")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.pane_focus_indicator {
            ui.horizontal(|ui| {
                ui.label("Focus Color:");
                let mut color = settings.config.pane_focus_color;
                let egui_color = egui::Color32::from_rgb(color[0], color[1], color[2]);
                let mut edit_color = egui_color;
                if ui.color_edit_button_srgba(&mut edit_color).changed() {
                    color = [edit_color.r(), edit_color.g(), edit_color.b()];
                    settings.config.pane_focus_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Focus Width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.pane_focus_width, 1.0..=5.0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Width of the focus indicator border")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Limits").strong());

        ui.horizontal(|ui| {
            ui.label("Max Panes:");
            if ui
                .add(egui::Slider::new(&mut settings.config.max_panes, 0..=32))
                .on_hover_text("Maximum number of panes per tab (0 = unlimited)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Min Pane Size:");
            if ui
                .add(egui::Slider::new(&mut settings.config.pane_min_size, 5..=40).suffix(" cells"))
                .on_hover_text("Minimum pane size in cells (prevents tiny unusable panes)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Keyboard Shortcuts").weak().small());
        #[cfg(target_os = "macos")]
        {
            ui.label(
                egui::RichText::new("  Cmd+D: Horizontal split, Cmd+Shift+D: Vertical split")
                    .weak()
                    .small(),
            );
            ui.label(
                egui::RichText::new("  Cmd+Option+Arrow: Navigate, Cmd+Option+Shift+Arrow: Resize")
                    .weak()
                    .small(),
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            ui.label(
                egui::RichText::new(
                    "  Ctrl+Shift+D: Horizontal split, Ctrl+Shift+E: Vertical split",
                )
                .weak()
                .small(),
            );
            ui.label(
                egui::RichText::new("  Ctrl+Alt+Arrow: Navigate, Ctrl+Alt+Shift+Arrow: Resize")
                    .weak()
                    .small(),
            );
        }
    });
}

// ============================================================================
// Pane Appearance Section
// ============================================================================

fn show_pane_appearance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Pane Appearance",
        "window_pane_appearance",
        false,
        collapsed,
        |ui| {
            ui.label(egui::RichText::new("Divider Colors").strong());

            ui.horizontal(|ui| {
                ui.label("Divider Color:");
                let mut color = settings.config.pane_divider_color;
                let egui_color = egui::Color32::from_rgb(color[0], color[1], color[2]);
                let mut edit_color = egui_color;
                if ui.color_edit_button_srgba(&mut edit_color).changed() {
                    color = [edit_color.r(), edit_color.g(), edit_color.b()];
                    settings.config.pane_divider_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Hover Color:");
                let mut color = settings.config.pane_divider_hover_color;
                let egui_color = egui::Color32::from_rgb(color[0], color[1], color[2]);
                let mut edit_color = egui_color;
                if ui
                    .color_edit_button_srgba(&mut edit_color)
                    .on_hover_text("Color when hovering over a divider for resize")
                    .changed()
                {
                    color = [edit_color.r(), edit_color.g(), edit_color.b()];
                    settings.config.pane_divider_hover_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Inactive Panes").strong());

            if ui
                .checkbox(
                    &mut settings.config.dim_inactive_panes,
                    "Dim inactive panes",
                )
                .on_hover_text("Reduce opacity of panes that don't have focus")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.dim_inactive_panes {
                ui.horizontal(|ui| {
                    ui.label("Inactive Opacity:");
                    if ui
                        .add(egui::Slider::new(
                            &mut settings.config.inactive_pane_opacity,
                            0.3..=1.0,
                        ))
                        .on_hover_text("Opacity level for unfocused panes (1.0 = fully visible)")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Pane Titles").strong());

            if ui
                .checkbox(&mut settings.config.show_pane_titles, "Show pane titles")
                .on_hover_text("Display a title bar at the top of each pane")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.show_pane_titles {
                ui.horizontal(|ui| {
                    ui.label("Title Height:");
                    if ui
                        .add(
                            egui::Slider::new(&mut settings.config.pane_title_height, 14.0..=30.0)
                                .suffix(" px"),
                        )
                        .on_hover_text("Height of pane title bars")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Title Position:");
                    let current_pos = settings.config.pane_title_position;
                    egui::ComboBox::from_id_salt("pane_title_position")
                        .selected_text(current_pos.display_name())
                        .show_ui(ui, |ui| {
                            for pos in PaneTitlePosition::ALL {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.pane_title_position,
                                        *pos,
                                        pos.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Title text color:");
                    let mut color = settings.config.pane_title_color;
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        settings.config.pane_title_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Title background:");
                    let mut color = settings.config.pane_title_bg_color;
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        settings.config.pane_title_bg_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Background Integration").strong());

            ui.horizontal(|ui| {
            ui.label("Pane Opacity:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.pane_background_opacity,
                    0.5..=1.0,
                ))
                .on_hover_text(
                    "Pane background opacity (lower values let background image/shader show through)",
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
