//! Copy mode integration for WindowState.
//!
//! Handles entering/exiting copy mode and the main key dispatch loop.
//! Search helpers, motion post-processing, and clipboard operations live in
//! `copy_mode_search`.

use crate::app::window_state::WindowState;
use crate::copy_mode::{CopyModeState, SearchDirection, VisualMode};
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

/// The keys `handle_copy_mode_key` below answers, as `(keys, what they do)`,
/// for the F1 help panel. Keep in step with `PLAIN_KEYS`, `CTRL_KEYS`, and
/// `NAMED_KEYS`.
pub(crate) const COPY_MODE_KEYS: &[(&str, &str)] = &[
    ("q / Escape", "Exit copy mode"),
    ("h j k l / arrows", "Left / Down / Up / Right"),
    ("w / b / e", "Word forward / back / end"),
    ("W / B / E", "WORD forward / back / end"),
    ("0 / $ / ^", "Line start / end / first non-blank"),
    ("gg / G", "Top of scrollback / bottom of buffer"),
    ("Ctrl+U / Ctrl+D", "Half page up / down"),
    ("Ctrl+B / Ctrl+F", "Full page up / down"),
    ("v / V / Ctrl+V", "Character / line / block selection"),
    ("y", "Yank (copy) selection"),
    ("1-9", "Count prefix (5j = down 5 lines)"),
    ("/ / ?", "Search forward / backward"),
    ("n / N", "Next / previous match"),
    ("m + letter / ' + letter", "Set / jump to mark"),
];

/// What a copy-mode key does once no pending prefix (search input, mark
/// name, second `g`) is waiting. State-dependent keys (`0`, `G`, `v`, `y`,
/// Escape) get their own variant; the dispatcher reads the state.
#[derive(Clone, Copy)]
pub(crate) enum CopyModeCommand {
    /// Cursor or selection change that needs no line text.
    Motion(fn(&mut CopyModeState)),
    /// Motion over the cursor line's text.
    LineMotion(fn(&mut CopyModeState, &str)),
    /// Word motion over the cursor line's text and the configured word characters.
    WordMotion(fn(&mut CopyModeState, &str, &str)),
    ZeroOrLineStart,
    CountDigit(u8),
    GotoLineOrBottom,
    PendingG,
    VisualChar,
    Yank,
    StartSearch(SearchDirection),
    SearchNext {
        reverse: bool,
    },
    MarkSet,
    MarkGoto,
    Exit,
    Escape,
}

use CopyModeCommand as C;

/// Unmodified character keys. Keep in step with `COPY_MODE_KEYS`.
const PLAIN_KEYS: &[(&str, CopyModeCommand)] = &[
    ("h", C::Motion(CopyModeState::move_left)),
    ("j", C::Motion(CopyModeState::move_down)),
    ("k", C::Motion(CopyModeState::move_up)),
    ("l", C::Motion(CopyModeState::move_right)),
    ("0", C::ZeroOrLineStart),
    ("$", C::Motion(CopyModeState::move_to_line_end)),
    ("^", C::LineMotion(CopyModeState::move_to_first_non_blank)),
    ("w", C::WordMotion(CopyModeState::move_word_forward)),
    ("b", C::WordMotion(CopyModeState::move_word_backward)),
    ("e", C::WordMotion(CopyModeState::move_word_end)),
    ("W", C::LineMotion(CopyModeState::move_big_word_forward)),
    ("B", C::LineMotion(CopyModeState::move_big_word_backward)),
    ("E", C::LineMotion(CopyModeState::move_big_word_end)),
    ("G", C::GotoLineOrBottom),
    ("g", C::PendingG),
    ("v", C::VisualChar),
    ("V", C::Motion(CopyModeState::toggle_visual_line)),
    ("y", C::Yank),
    ("/", C::StartSearch(SearchDirection::Forward)),
    ("?", C::StartSearch(SearchDirection::Backward)),
    ("n", C::SearchNext { reverse: false }),
    ("N", C::SearchNext { reverse: true }),
    ("m", C::MarkSet),
    ("'", C::MarkGoto),
    ("1", C::CountDigit(1)),
    ("2", C::CountDigit(2)),
    ("3", C::CountDigit(3)),
    ("4", C::CountDigit(4)),
    ("5", C::CountDigit(5)),
    ("6", C::CountDigit(6)),
    ("7", C::CountDigit(7)),
    ("8", C::CountDigit(8)),
    ("9", C::CountDigit(9)),
    ("q", C::Exit),
];

/// Character keys with Ctrl held.
const CTRL_KEYS: &[(&str, CopyModeCommand)] = &[
    ("u", C::Motion(CopyModeState::half_page_up)),
    ("d", C::Motion(CopyModeState::half_page_down)),
    ("b", C::Motion(CopyModeState::page_up)),
    ("f", C::Motion(CopyModeState::page_down)),
    ("v", C::Motion(CopyModeState::toggle_visual_block)),
];

/// Named keys; Ctrl is ignored for these.
const NAMED_KEYS: &[(NamedKey, CopyModeCommand)] = &[
    (NamedKey::Escape, C::Escape),
    (NamedKey::ArrowLeft, C::Motion(CopyModeState::move_left)),
    (NamedKey::ArrowRight, C::Motion(CopyModeState::move_right)),
    (NamedKey::ArrowUp, C::Motion(CopyModeState::move_up)),
    (NamedKey::ArrowDown, C::Motion(CopyModeState::move_down)),
    (NamedKey::Home, C::Motion(CopyModeState::move_to_line_start)),
    (NamedKey::End, C::Motion(CopyModeState::move_to_line_end)),
    (NamedKey::PageUp, C::Motion(CopyModeState::page_up)),
    (NamedKey::PageDown, C::Motion(CopyModeState::page_down)),
];

/// Classify a copy-mode key press; `None` means the key is ignored.
pub(crate) fn copy_mode_key_command(key: &Key, ctrl: bool) -> Option<CopyModeCommand> {
    match key {
        Key::Character(ch) => {
            let table = if ctrl { CTRL_KEYS } else { PLAIN_KEYS };
            table
                .iter()
                .find(|(name, _)| *name == ch.as_str())
                .map(|&(_, command)| command)
        }
        Key::Named(named) => NAMED_KEYS
            .iter()
            .find(|(name, _)| name == named)
            .map(|&(_, command)| command),
        _ => None,
    }
}

impl WindowState {
    /// Check if copy mode is currently active
    pub(crate) fn is_copy_mode_active(&self) -> bool {
        self.copy_mode.active
    }

    /// Enter copy mode, anchoring the cursor at the current terminal cursor position
    pub(crate) fn enter_copy_mode(&mut self) {
        if !self.config.load().copy_mode.copy_mode_enabled {
            return;
        }

        // try_lock: intentional — copy mode initialization in sync event loop.
        // On miss: copy mode is not entered this keypress. User can try again.
        // Reads the focused pane's terminal: in a mux tab that is the daemon
        // mirror on screen, while the tab terminal is a hidden local shell.
        let Some((cursor_col, cursor_row, cols, rows, scrollback_len)) =
            self.tab_manager.active_tab().and_then(|tab| {
                tab.try_with_read_terminal(|term| {
                    let (col, row) = term.cursor_position();
                    let (cols, rows) = term.dimensions();
                    let sb_len = term.scrollback_len();
                    (col, row, cols, rows, sb_len)
                })
            })
        else {
            return;
        };

        self.copy_mode
            .enter(cursor_col, cursor_row, cols, rows, scrollback_len);
        self.sync_copy_mode_selection();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        crate::debug_info!(
            "COPY_MODE",
            "Entered copy mode at ({}, {})",
            cursor_col,
            cursor_row
        );
    }

    /// Exit copy mode, clearing selection and restoring scroll
    pub(crate) fn exit_copy_mode(&mut self) {
        self.copy_mode.exit();
        // Clear selection (per-pane aware)
        self.with_active_tab_mut(|tab| {
            tab.selection_mouse_mut().selection = None;
            tab.active_cache_mut().cells = None; // Invalidate cache
        });
        // Scroll back to bottom
        self.set_scroll_target(0);
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        crate::debug_info!("COPY_MODE", "Exited copy mode");
    }

    /// Handle a key event in copy mode. Returns true if the key was consumed.
    pub(crate) fn handle_copy_mode_key(&mut self, event: &KeyEvent) {
        // Handle search input mode
        if self.copy_mode.is_searching {
            self.handle_copy_mode_search_key(event);
            return;
        }
        self.handle_copy_mode_logical_key(&event.logical_key);
    }

    /// Copy-mode dispatch outside search input; only the logical key matters.
    pub(crate) fn handle_copy_mode_logical_key(&mut self, logical_key: &Key) {
        // Handle pending mark set (waiting for mark name after 'm')
        if self.copy_mode.pending_mark_set {
            self.copy_mode.pending_mark_set = false;
            if let Key::Character(ch) = logical_key
                && let Some(c) = ch.chars().next()
                && c.is_ascii_lowercase()
            {
                self.copy_mode.set_mark(c);
                crate::debug_info!("COPY_MODE", "Set mark '{}'", c);
                self.request_redraw();
                return;
            }
            return;
        }

        // Handle pending mark goto (waiting for mark name after "'")
        if self.copy_mode.pending_mark_goto {
            self.copy_mode.pending_mark_goto = false;
            if let Key::Character(ch) = logical_key
                && let Some(c) = ch.chars().next()
                && c.is_ascii_lowercase()
                && self.copy_mode.goto_mark(c)
            {
                crate::debug_info!("COPY_MODE", "Jumped to mark '{}'", c);
                self.after_copy_mode_motion();
                return;
            }
            return;
        }

        // Handle pending 'g' (waiting for second 'g' in 'gg')
        if self.copy_mode.pending_g {
            self.copy_mode.pending_g = false;
            if let Key::Character(ch) = logical_key
                && ch.as_str() == "g"
            {
                self.copy_mode.goto_top();
                self.after_copy_mode_motion();
                return;
            }
            // Not 'g', ignore the pending state
            return;
        }

        let ctrl = self.input_handler.modifiers.state().control_key();
        let Some(command) = copy_mode_key_command(logical_key, ctrl) else {
            return;
        };

        match command {
            CopyModeCommand::Motion(motion) => {
                motion(&mut self.copy_mode);
                self.after_copy_mode_motion();
            }
            CopyModeCommand::LineMotion(motion) => {
                if let Some(text) = self.get_copy_mode_line_text() {
                    motion(&mut self.copy_mode, &text);
                    self.after_copy_mode_motion();
                }
            }
            CopyModeCommand::WordMotion(motion) => {
                if let Some(text) = self.get_copy_mode_line_text() {
                    let word_chars = self.config.load().word_selection.word_characters.clone();
                    motion(&mut self.copy_mode, &text, &word_chars);
                    self.after_copy_mode_motion();
                }
            }
            CopyModeCommand::ZeroOrLineStart => {
                // '0' can be part of a count or line start
                if self.copy_mode.count.is_some() {
                    self.copy_mode.push_count_digit(0);
                } else {
                    self.copy_mode.move_to_line_start();
                    self.after_copy_mode_motion();
                }
            }
            CopyModeCommand::CountDigit(digit) => self.copy_mode.push_count_digit(digit),
            CopyModeCommand::GotoLineOrBottom => {
                if let Some(count) = self.copy_mode.count.take() {
                    // {count}G goes to absolute line
                    self.copy_mode.goto_line(count.saturating_sub(1));
                } else {
                    self.copy_mode.goto_bottom();
                }
                self.after_copy_mode_motion();
            }
            CopyModeCommand::PendingG => self.copy_mode.pending_g = true,
            CopyModeCommand::VisualChar => {
                if self.copy_mode.pending_operator.is_some() {
                    // yv = yank to current pos (do nothing special)
                    self.copy_mode.pending_operator = None;
                } else {
                    self.copy_mode.toggle_visual_char();
                    self.after_copy_mode_motion();
                }
            }
            CopyModeCommand::Yank => {
                if self.copy_mode.visual_mode != VisualMode::None {
                    // In visual mode, yank the selection
                    self.yank_copy_mode_selection();
                } else {
                    // Set pending yank operator (yy = yank line, yw = yank word, etc.)
                    self.copy_mode.pending_operator = Some(crate::copy_mode::PendingOperator::Yank);
                }
            }
            CopyModeCommand::StartSearch(direction) => {
                self.copy_mode.start_search(direction);
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            CopyModeCommand::SearchNext { reverse } => self.execute_copy_mode_search(reverse),
            CopyModeCommand::MarkSet => self.copy_mode.pending_mark_set = true,
            CopyModeCommand::MarkGoto => self.copy_mode.pending_mark_goto = true,
            CopyModeCommand::Exit => self.exit_copy_mode(),
            CopyModeCommand::Escape => {
                if self.copy_mode.visual_mode != VisualMode::None {
                    // Exit visual mode first
                    self.copy_mode.visual_mode = VisualMode::None;
                    self.copy_mode.selection_anchor = None;
                    self.with_active_tab_mut(|tab| {
                        tab.selection_mouse_mut().selection = None;
                        tab.active_cache_mut().cells = None;
                    });
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                } else {
                    self.exit_copy_mode();
                }
            }
        }
    }
}
