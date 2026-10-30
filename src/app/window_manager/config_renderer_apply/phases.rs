use std::sync::Arc;

use crate::app::window_state::WindowState;
use crate::app::window_state::config_updates::ConfigChanges;
use crate::config::Config;
use par_term_terminal::conversion::{
    to_core_ambiguous_width, to_core_normalization_form, to_core_unicode_version,
};

/// Apply the always-on window-appearance settings (opacity, translucency) and the
/// change-gated transparency, text-opacity, link-underline, vsync, scrollbar and
/// cursor-color settings.
pub(super) fn apply_appearance(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) {
    let Some(renderer) = window_state.renderer.as_mut() else {
        return;
    };
    // Update opacity
    renderer.update_opacity(config.window.window_opacity);

    // macOS 27 renders the titlebar from the NSWindow background color when
    // the window is non-opaque; keep that color in sync with the translucency
    // setting (see macos_metal::set_window_background_for_translucency).
    #[cfg(target_os = "macos")]
    if let Some(window) = &window_state.window {
        let translucent = config.window.window_opacity < 1.0
            || (config.background.background_image_enabled
                && config.background.background_image_opacity < 1.0);
        if let Err(e) =
            crate::macos_metal::set_window_background_for_translucency(window, translucent)
        {
            log::warn!("Failed to update window background for translucency: {}", e);
        }
    }

    // Update transparency mode if changed
    if changes.transparency_mode {
        renderer.set_transparency_affects_only_default_background(
            config
                .background
                .transparency_affects_only_default_background,
        );
        window_state.focus_state.needs_redraw = true;
    }

    // Update text opacity mode if changed
    if changes.keep_text_opaque {
        renderer.set_keep_text_opaque(config.background.keep_text_opaque);
        window_state.focus_state.needs_redraw = true;
    }

    if changes.link_underline_style {
        renderer.set_link_underline_style(config.semantic_history.link_underline_style);
        window_state.focus_state.needs_redraw = true;
    }

    // Update vsync mode if changed
    if changes.vsync_mode {
        let (actual_mode, _changed) = renderer.update_vsync_mode(config.rendering.vsync_mode);
        // If the actual mode differs, update config
        if actual_mode != config.rendering.vsync_mode {
            window_state.config.rcu(|old| {
                let mut new = (**old).clone();
                new.rendering.vsync_mode = actual_mode;
                std::sync::Arc::new(new)
            });
            log::warn!(
                "Vsync mode {:?} is not supported. Using {:?} instead.",
                config.rendering.vsync_mode,
                actual_mode
            );
        }
    }

    // Update scrollbar appearance
    renderer.update_scrollbar_appearance(
        config.scrollbar.scrollbar_width,
        config.scrollbar.scrollbar_thumb_color,
        config.scrollbar.scrollbar_track_color,
    );
    window_state.focus_state.needs_redraw = true;

    // Update cursor color
    if changes.cursor_color {
        renderer.update_cursor_color(config.cursor.cursor_color);
    }

    // Update cursor text color
    if changes.cursor_text_color {
        renderer.update_cursor_text_color(config.cursor.cursor_text_color);
    }
}

/// Apply change-gated cursor style/blink, cursor enhancements and command
/// separator settings.
pub(super) fn apply_cursor(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) {
    // Update cursor style and blink for all tabs
    if changes.cursor_style || changes.cursor_blink {
        use crate::config::CursorStyle as ConfigCursorStyle;
        use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

        let term_style = if config.cursor.cursor_blink {
            match config.cursor.cursor_style {
                ConfigCursorStyle::Block => TermCursorStyle::BlinkingBlock,
                ConfigCursorStyle::Beam => TermCursorStyle::BlinkingBar,
                ConfigCursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
            }
        } else {
            match config.cursor.cursor_style {
                ConfigCursorStyle::Block => TermCursorStyle::SteadyBlock,
                ConfigCursorStyle::Beam => TermCursorStyle::SteadyBar,
                ConfigCursorStyle::Underline => TermCursorStyle::SteadyUnderline,
            }
        };

        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_cursor_style(term_style);
            }
            tab.active_cache_mut().cells = None; // Invalidate cache to redraw cursor
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply cursor enhancement changes
    if changes.cursor_enhancements {
        // Re-borrow renderer (can't hold it across the tabs_mut loop above)
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_cursor_guide(
                config.cursor.cursor_guide_enabled,
                config.cursor.cursor_guide_color,
            );
            renderer.update_cursor_shadow(
                config.cursor.cursor_shadow_enabled,
                config.cursor.cursor_shadow_color,
                config.cursor.cursor_shadow_offset,
                config.cursor.cursor_shadow_blur,
            );
            renderer
                .update_cursor_boost(config.cursor.cursor_boost, config.cursor.cursor_boost_color);
            renderer.update_unfocused_cursor_style(config.cursor.unfocused_cursor_style);
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply command separator changes
    if changes.command_separator {
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_command_separator(
                config.command_separator.command_separator_enabled,
                config.command_separator.command_separator_thickness,
                config.command_separator.command_separator_opacity,
                config.command_separator.command_separator_exit_color,
                config.command_separator.command_separator_color,
            );
        }
        window_state.focus_state.needs_redraw = true;
    }
}

/// Apply change-gated background (mode/color/image), per-pane background and
/// inline image display settings.
pub(super) fn apply_backgrounds(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) {
    // Apply background changes (mode, color, or image)
    if changes.any_bg_change() {
        // Expand tilde in path
        let expanded_path = config.background.background_image.as_ref().map(|p| {
            if let Some(rest) = p.strip_prefix("~/")
                && let Some(home) = dirs::home_dir()
            {
                return home.join(rest).to_string_lossy().to_string();
            }
            p.clone()
        });
        if let Some(renderer) = &mut window_state.renderer {
            renderer.set_background(
                config.image.background_mode,
                config.image.background_color,
                expanded_path.as_deref(),
                config.background.background_image_mode,
                config.background.background_image_opacity,
                config.background.background_image_enabled,
            );
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply per-pane background changes to existing panes
    if changes.pane_backgrounds {
        // Pre-load all pane background textures into the renderer cache
        if let Some(renderer) = &mut window_state.renderer {
            for pb_config in &config.image.pane_backgrounds {
                if let Err(e) = renderer.load_pane_background(&pb_config.image) {
                    log::error!(
                        "Failed to load pane {} background '{}': {}",
                        pb_config.index,
                        pb_config.image,
                        e
                    );
                }
            }
        }

        for tab in window_state.tab_manager.tabs_mut() {
            if let Some(pm) = tab.pane_manager_mut() {
                let panes = pm.all_panes_mut();
                for (index, pane) in panes.into_iter().enumerate() {
                    if let Some((image_path, mode, opacity, darken)) =
                        config.get_pane_background(index)
                    {
                        let bg = crate::pane::PaneBackground {
                            image_path: Some(image_path),
                            mode,
                            opacity,
                            darken,
                        };
                        pane.set_background(bg);
                    } else {
                        // Clear pane background if no longer configured
                        pane.set_background(crate::pane::PaneBackground::new());
                    }
                }
            }
        }
        if let Some(renderer) = &mut window_state.renderer {
            renderer.mark_dirty();
        }
        window_state.focus_state.needs_redraw = true;
    }

    // Apply inline image settings changes
    if changes.image_scaling_mode {
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_image_scaling_mode(config.image.image_scaling_mode);
        }
        window_state.focus_state.needs_redraw = true;
    }
    if changes.image_preserve_aspect_ratio {
        if let Some(renderer) = &mut window_state.renderer {
            renderer.update_image_preserve_aspect_ratio(config.image.image_preserve_aspect_ratio);
        }
        window_state.focus_state.needs_redraw = true;
    }
}

/// Apply change-gated settings that fan out to every tab's terminal: theme,
/// answerback, notification and OSC limits, Unicode width and normalization.
pub(super) fn apply_terminal_settings(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) {
    // Apply theme changes to all tabs and all pane terminals
    if changes.theme {
        // par-mux panes answer OSC 10/11 from the daemon's copy of the
        // client theme — push the new colors or apps in mux panes keep
        // misdetecting dark/light mode after a theme switch.
        #[cfg(feature = "mux")]
        window_state.push_mux_client_colors();
        let theme = config.load_theme();
        for tab in window_state.tab_manager.tabs_mut() {
            // Set theme on tab's primary terminal
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_theme(theme.clone());
            }
            // Set theme on additional split pane terminals (primary pane shares
            // tab.terminal's Arc, so skip it to avoid double-locking). Theme
            // changes recolor cells without bumping update_generation, so every
            // pane's cross-frame cell cache must be invalidated here too.
            let tab_terminal = Arc::clone(&tab.terminal);
            if let Some(pm) = tab.pane_manager_mut() {
                for pane in pm.all_panes_mut() {
                    if !Arc::ptr_eq(&pane.terminal, &tab_terminal)
                        && let Ok(mut term) = pane.terminal.try_write()
                    {
                        term.set_theme(theme.clone());
                    }
                    pane.cache.invalidate_pane_cells();
                }
            }
        }
    }

    // Update ENQ answerback string across all tabs when changed
    if changes.answerback_string {
        let answerback = if config.shell.answerback_string.is_empty() {
            None
        } else {
            Some(config.shell.answerback_string.clone())
        };
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_read() {
                term.set_answerback_string(answerback.clone());
            }
        }
    }

    // Update notification buffer limit across all tabs when changed
    if changes.max_notifications {
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_read() {
                term.set_max_notifications(config.notifications.notification_max_buffer);
            }
        }
    }

    // Update OSC data length limit across all tabs when changed
    if changes.max_osc_data_length {
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_read() {
                term.set_max_osc_data_length(config.max_osc_data_length);
            }
        }
    }

    // Apply Unicode width settings
    if changes.unicode_width {
        let width_config = par_term_emu_core_rust::WidthConfig::new(
            to_core_unicode_version(config.unicode.unicode_version),
            to_core_ambiguous_width(config.unicode.ambiguous_width),
        );
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_read() {
                term.set_width_config(width_config);
            }
        }
    }

    // Apply Unicode normalization form
    if changes.normalization_form {
        for tab in window_state.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_read() {
                term.set_normalization_form(to_core_normalization_form(
                    config.unicode.normalization_form,
                ));
            }
        }
    }
}
