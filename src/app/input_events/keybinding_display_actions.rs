//! Display-related keybinding action helpers for WindowState.
//!
//! Extracted from `keybinding_actions` to keep the main dispatch function
//! under the 500-line target.
//!
//! Contains `execute_display_keybinding_action`, which handles:
//! - Font size changes (increase / decrease / reset)
//! - Cursor style cycling
//! - Tab index switching (switch_to_tab_1 … switch_to_tab_9)
//! - Tab reordering (move_tab_left / move_tab_right)
//! - Throughput mode toggle
//! - Reopen closed tab
//! - Arrangement save / SSH quick-connect / dynamic profile reload
//!
//! Dispatch goes through [`DISPLAY_ACTION_HANDLERS`] for the same reasons
//! `keybinding_actions` uses a table — see that module's header. The two
//! tables must not share a key; `dispatch_tests` enforces that, and the
//! `match` form this replaced could not.

use crate::app::window_state::WindowState;

/// Handler for one display/navigation keybinding action.
///
/// The `bool` becomes the `Some(_)` payload of
/// `execute_display_keybinding_action`: `true` when handled, `false` when the
/// name is claimed but no-op'd. A name absent from the table yields `None` so
/// the caller continues to the next handler.
pub(super) type DisplayActionHandler = fn(&mut WindowState) -> bool;

/// Exact-match dispatch table for display/navigation keybinding actions.
///
/// Entry order is irrelevant to behavior — keys are distinct literals matched
/// by equality — but keys must stay unique and must not collide with
/// `keybinding_actions::ACTION_HANDLERS`, which would shadow them silently.
/// Both properties are asserted in `dispatch_tests`.
pub(super) static DISPLAY_ACTION_HANDLERS: &[(&str, DisplayActionHandler)] = &[
    ("increase_font_size", |s: &mut WindowState| {
        s.config.rcu(|old| {
            let mut new = (**old).clone();
            new.font_size = (old.font_size + 1.0).min(72.0);
            std::sync::Arc::new(new)
        });
        s.render_loop.pending_font_rebuild = true;
        log::info!(
            "Font size increased to {} via keybinding",
            s.config.load().font_size
        );
        s.request_redraw();
        true
    }),
    ("decrease_font_size", |s: &mut WindowState| {
        s.config.rcu(|old| {
            let mut new = (**old).clone();
            new.font_size = (old.font_size - 1.0).max(6.0);
            std::sync::Arc::new(new)
        });
        s.render_loop.pending_font_rebuild = true;
        log::info!(
            "Font size decreased to {} via keybinding",
            s.config.load().font_size
        );
        s.request_redraw();
        true
    }),
    ("reset_font_size", |s: &mut WindowState| {
        s.config.rcu(|old| {
            let mut new = (**old).clone();
            new.font_size = 14.0;
            std::sync::Arc::new(new)
        });
        s.render_loop.pending_font_rebuild = true;
        log::info!("Font size reset to default (14.0) via keybinding");
        s.request_redraw();
        true
    }),
    ("cycle_cursor_style", cycle_cursor_style),
    ("move_tab_left", |s: &mut WindowState| {
        s.move_tab_left();
        log::debug!("Moved tab left via keybinding");
        true
    }),
    ("move_tab_right", |s: &mut WindowState| {
        s.move_tab_right();
        log::debug!("Moved tab right via keybinding");
        true
    }),
    ("switch_to_tab_1", |s: &mut WindowState| {
        s.switch_to_tab_index(1);
        true
    }),
    ("switch_to_tab_2", |s: &mut WindowState| {
        s.switch_to_tab_index(2);
        true
    }),
    ("switch_to_tab_3", |s: &mut WindowState| {
        s.switch_to_tab_index(3);
        true
    }),
    ("switch_to_tab_4", |s: &mut WindowState| {
        s.switch_to_tab_index(4);
        true
    }),
    ("switch_to_tab_5", |s: &mut WindowState| {
        s.switch_to_tab_index(5);
        true
    }),
    ("switch_to_tab_6", |s: &mut WindowState| {
        s.switch_to_tab_index(6);
        true
    }),
    ("switch_to_tab_7", |s: &mut WindowState| {
        s.switch_to_tab_index(7);
        true
    }),
    ("switch_to_tab_8", |s: &mut WindowState| {
        s.switch_to_tab_index(8);
        true
    }),
    ("switch_to_tab_9", |s: &mut WindowState| {
        s.switch_to_tab_index(9);
        true
    }),
    ("toggle_throughput_mode", |s: &mut WindowState| {
        s.config.rcu(|old| {
            let mut new = (**old).clone();
            new.rendering.maximize_throughput = !old.rendering.maximize_throughput;
            std::sync::Arc::new(new)
        });
        let message = if s.config.load().rendering.maximize_throughput {
            "Throughput Mode: ON"
        } else {
            "Throughput Mode: OFF"
        };
        s.show_toast(message);
        log::info!(
            "Throughput mode {}",
            if s.config.load().rendering.maximize_throughput {
                "enabled"
            } else {
                "disabled"
            }
        );
        true
    }),
    ("reopen_closed_tab", |s: &mut WindowState| {
        s.reopen_closed_tab();
        true
    }),
    ("save_arrangement", |s: &mut WindowState| {
        // Open settings to Arrangements tab
        s.overlay_state.open_settings_window_requested = true;
        s.request_redraw();
        log::info!("Save arrangement requested via keybinding");
        true
    }),
    ("ssh_quick_connect", |s: &mut WindowState| {
        s.overlay_ui.ssh_connect_ui.open(
            s.config.load().ssh.enable_mdns_discovery,
            s.config.load().ssh.mdns_scan_timeout_secs,
        );
        s.request_redraw();
        log::info!("SSH Quick Connect opened via keybinding");
        true
    }),
    ("reload_dynamic_profiles", |s: &mut WindowState| {
        s.overlay_state.reload_dynamic_profiles_requested = true;
        s.request_redraw();
        log::info!("Dynamic profiles reload requested via keybinding");
        true
    }),
];

fn cycle_cursor_style(s: &mut WindowState) -> bool {
    use crate::config::CursorStyle;
    use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

    s.config.rcu(|old| {
        let mut new = (**old).clone();
        new.cursor.cursor_style = match old.cursor.cursor_style {
            CursorStyle::Block => CursorStyle::Beam,
            CursorStyle::Beam => CursorStyle::Underline,
            CursorStyle::Underline => CursorStyle::Block,
        };
        std::sync::Arc::new(new)
    });

    s.invalidate_tab_cache();
    s.focus_state.needs_redraw = true;

    log::info!(
        "Cycled cursor style to {:?} via keybinding",
        s.config.load().cursor.cursor_style
    );

    let term_style = if s.config.load().cursor.cursor_blink {
        match s.config.load().cursor.cursor_style {
            CursorStyle::Block => TermCursorStyle::BlinkingBlock,
            CursorStyle::Beam => TermCursorStyle::BlinkingBar,
            CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
        }
    } else {
        match s.config.load().cursor.cursor_style {
            CursorStyle::Block => TermCursorStyle::SteadyBlock,
            CursorStyle::Beam => TermCursorStyle::SteadyBar,
            CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
        }
    };

    // try_lock: intentional — cursor blink toggle via keybinding in sync loop.
    // On miss: cursor style not updated this invocation. Cosmetic only.
    if let Some(tab) = s.tab_manager.active_tab()
        && let Ok(mut term) = tab.terminal.try_write()
    {
        term.set_cursor_style(term_style);
    }
    true
}

impl WindowState {
    /// Handle display- and navigation-related keybinding actions.
    ///
    /// Returns `Some(true)` when the action was handled, `Some(false)` when the
    /// action name was recognised but no-op'd, and `None` when the name is not
    /// in this handler (caller should try the next handler).
    pub(crate) fn execute_display_keybinding_action(&mut self, action: &str) -> Option<bool> {
        let (_, handler) = DISPLAY_ACTION_HANDLERS
            .iter()
            .find(|(name, _)| *name == action)?;
        Some(handler(self))
    }
}
