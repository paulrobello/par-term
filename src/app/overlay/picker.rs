//! The shared list/picker component (UX.md OV5).
//!
//! One keyboard model and one scroll window for every picker: a filter
//! field focused on open; ArrowUp/ArrowDown, PageUp/PageDown, Home/End;
//! a scrolling window that always keeps the selection drawn (B62); Enter
//! picks, with Shift+Enter and Cmd/Ctrl+Enter alternates when the picker
//! offers them; Escape closes; a footer names the live keys.
//!
//! [`ListNav`] is the pure selection/scroll state machine — testable
//! without egui. [`show_list`] draws a picker over a caller-owned row
//! list; the caller keeps its own row type and filtering, and hands the
//! component already-filtered rows plus a row renderer.

use egui::{Context, Frame, Key, RichText, Window, epaint::Shadow};

/// Selection and scroll window over a filtered list of `len` rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ListNav {
    /// Index into the filtered list.
    pub(crate) selected: usize,
    /// First drawn row, in the same index space as `selected` (B62).
    pub(crate) scroll_offset: usize,
}

/// One frame's navigation input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NavInput {
    pub(crate) down: usize,
    pub(crate) up: usize,
    pub(crate) page_down: usize,
    pub(crate) page_up: usize,
    pub(crate) home: bool,
    pub(crate) end: bool,
}

impl ListNav {
    /// Back to the top (a fresh open or a changed query).
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Apply one frame's navigation over `len` rows with `visible` rows
    /// drawn, then keep the selection inside the list and the drawn window.
    pub(crate) fn apply(&mut self, input: NavInput, len: usize, visible: usize) {
        let last = len.saturating_sub(1);
        let page = visible.max(1);
        let mut sel = self.selected.min(last);
        sel = (sel + input.down + input.page_down * page).min(last);
        sel = sel.saturating_sub(input.up + input.page_up * page);
        if input.home {
            sel = 0;
        }
        if input.end {
            sel = last;
        }
        self.selected = sel;
        self.keep_visible(len, visible);
    }

    /// Scroll the window so `selected` stays drawn; a list shorter than
    /// the window parks at 0.
    pub(crate) fn keep_visible(&mut self, len: usize, visible: usize) {
        self.selected = self.selected.min(len.saturating_sub(1));
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible {
            self.scroll_offset = self.selected + 1 - visible;
        }
        self.scroll_offset = self.scroll_offset.min(len.saturating_sub(visible));
    }

    /// Whether the selected row is inside the drawn window.
    pub(crate) fn selected_is_visible(&self, visible: usize) -> bool {
        self.selected >= self.scroll_offset && self.selected < self.scroll_offset + visible
    }
}

/// Which activation a row got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Activation {
    /// Enter or a click.
    Primary,
    /// Shift+Enter.
    Shift,
    /// Cmd/Ctrl+Enter.
    Command,
}

/// What a picker frame produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ListOutcome {
    /// Nothing chosen yet.
    Open,
    /// The row at this filtered index was activated.
    Chosen { index: usize, how: Activation },
    /// Escape: close without a choice.
    Closed,
}

/// Static configuration for one picker.
pub(crate) struct ListConfig<'a> {
    /// egui window id (and invisible title).
    pub(crate) id: &'a str,
    /// Filter hint text.
    pub(crate) hint: &'a str,
    /// Rows drawn before the list scrolls.
    pub(crate) visible_rows: usize,
    /// Minimum width (see `theme`).
    pub(crate) width: f32,
    /// Shown when no row matches.
    pub(crate) empty_text: &'a str,
    /// Footer: the live keys, e.g. `"Enter run · Esc close"`.
    pub(crate) footer: &'a str,
    /// Offer Shift+Enter / Cmd+Enter activations.
    pub(crate) alternates: bool,
}

/// Read this frame's navigation keys. `num_presses`, not `key_pressed`:
/// presses can coalesce into one frame (fast typing, key repeat, the
/// ui-test harness), and each must move the selection.
fn read_nav(ctx: &Context) -> NavInput {
    ctx.input(|i| NavInput {
        down: i.num_presses(Key::ArrowDown),
        up: i.num_presses(Key::ArrowUp),
        page_down: i.num_presses(Key::PageDown),
        page_up: i.num_presses(Key::PageUp),
        home: i.key_pressed(Key::Home) && i.modifiers.is_none(),
        end: i.key_pressed(Key::End) && i.modifiers.is_none(),
    })
}

/// Read the activation chord, consuming it so no other widget acts on it.
fn read_activation(ctx: &Context, alternates: bool) -> Option<Activation> {
    ctx.input_mut(|i| {
        if alternates && i.consume_key(egui::Modifiers::COMMAND, Key::Enter) {
            Some(Activation::Command)
        } else if alternates && i.consume_key(egui::Modifiers::SHIFT, Key::Enter) {
            Some(Activation::Shift)
        } else if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
            Some(Activation::Primary)
        } else {
            None
        }
    })
}

/// Draw a picker for one frame.
///
/// `query` is the caller's filter text (edited by the field); `rows_len`
/// is the number of rows matching it; `draw_row(ui, index, selected)`
/// draws filtered row `index` and returns whether it was clicked. The
/// caller re-filters when `query` changes (the returned `query_changed`),
/// and should reset `nav` then.
pub(crate) fn show_list(
    ctx: &Context,
    config: &ListConfig<'_>,
    query: &mut String,
    request_focus: &mut bool,
    nav: &mut ListNav,
    rows_len: usize,
    mut draw_row: impl FnMut(&mut egui::Ui, usize, bool) -> bool,
) -> (ListOutcome, bool) {
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
        return (ListOutcome::Closed, false);
    }
    nav.apply(read_nav(ctx), rows_len, config.visible_rows);
    let mut outcome = ListOutcome::Open;
    if let Some(how) = read_activation(ctx, config.alternates)
        && rows_len > 0
    {
        outcome = ListOutcome::Chosen {
            index: nav.selected,
            how,
        };
    }

    let mut query_changed = false;
    Window::new(config.id)
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
        .frame(Frame::popup(&ctx.global_style()).shadow(Shadow::default()))
        .show(ctx, |ui| {
            ui.set_min_width(config.width);
            let field = ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text(config.hint)
                    .desired_width(f32::INFINITY),
            );
            if *request_focus {
                field.request_focus();
                *request_focus = false;
            }
            query_changed = field.changed();
            ui.separator();
            let end = (nav.scroll_offset + config.visible_rows).min(rows_len);
            for index in nav.scroll_offset..end {
                if draw_row(ui, index, index == nav.selected) {
                    outcome = ListOutcome::Chosen {
                        index,
                        how: Activation::Primary,
                    };
                }
            }
            if rows_len == 0 {
                ui.weak(config.empty_text);
            }
            ui.separator();
            ui.label(RichText::new(config.footer).weak().small());
        });
    (outcome, query_changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(n: usize) -> NavInput {
        NavInput {
            down: n,
            ..Default::default()
        }
    }

    #[test]
    fn arrowing_to_the_20th_row_scrolls_it_into_view() {
        // B62: 12 rows drawn; selecting row 19 advances the window so row
        // 19 is its last drawn row, never an invisible selection.
        let mut nav = ListNav::default();
        nav.apply(down(19), 50, 12);
        assert_eq!(nav.selected, 19);
        assert_eq!(nav.scroll_offset, 8);
        assert!(nav.selected_is_visible(12));
    }

    #[test]
    fn arrowing_back_up_pulls_the_window_home() {
        let mut nav = ListNav {
            selected: 19,
            scroll_offset: 8,
        };
        nav.apply(
            NavInput {
                up: 19,
                ..Default::default()
            },
            50,
            12,
        );
        assert_eq!(nav, ListNav::default());
    }

    #[test]
    fn a_shorter_list_parks_the_window_at_zero_and_clamps_the_selection() {
        // A stale offset from a longer result list must not blank a short one.
        let mut nav = ListNav {
            selected: 30,
            scroll_offset: 40,
        };
        nav.apply(NavInput::default(), 5, 12);
        assert_eq!(nav.selected, 4, "selection clamps to the last row");
        assert_eq!(nav.scroll_offset, 0);
        assert!(nav.selected_is_visible(12));
        nav.apply(NavInput::default(), 0, 12);
        assert_eq!(nav, ListNav::default(), "an empty list parks at 0");
    }

    #[test]
    fn page_keys_move_by_the_window_and_home_end_jump() {
        let mut nav = ListNav::default();
        nav.apply(
            NavInput {
                page_down: 2,
                ..Default::default()
            },
            100,
            10,
        );
        assert_eq!(nav.selected, 20);
        assert!(nav.selected_is_visible(10));
        nav.apply(
            NavInput {
                end: true,
                ..Default::default()
            },
            100,
            10,
        );
        assert_eq!(nav.selected, 99);
        assert_eq!(nav.scroll_offset, 90);
        nav.apply(
            NavInput {
                page_up: 1,
                ..Default::default()
            },
            100,
            10,
        );
        assert_eq!(nav.selected, 89);
        nav.apply(
            NavInput {
                home: true,
                ..Default::default()
            },
            100,
            10,
        );
        assert_eq!(nav, ListNav::default());
    }

    #[test]
    fn enter_alternates_resolve_by_modifier() {
        let ctx = egui::Context::default();
        let key = |modifiers| egui::Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        for (modifiers, alternates, expected) in [
            (egui::Modifiers::NONE, true, Some(Activation::Primary)),
            (egui::Modifiers::SHIFT, true, Some(Activation::Shift)),
            (egui::Modifiers::COMMAND, true, Some(Activation::Command)),
            // Without alternates, Shift+Enter is plain Enter (egui matches
            // NONE logically, ignoring Shift) — the palette's old behavior.
            (egui::Modifiers::SHIFT, false, Some(Activation::Primary)),
        ] {
            let mut got = None;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    events: vec![key(modifiers)],
                    ..Default::default()
                },
                |ui| got = read_activation(ui.ctx(), alternates),
            );
            out.textures_delta.clear();
            assert_eq!(got, expected, "{modifiers:?} alternates={alternates}");
        }
    }
}
