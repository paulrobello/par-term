//! The tree picker (UX.md A15, iTerm2's Open Quickly, `Cmd + Shift + O`):
//! every window, tab, and pane in one fuzzy list — hidden par-mux tabs
//! included — and Enter jumps to the chosen row.
//!
//! The rows come from a cross-window snapshot the window manager builds
//! each frame while the picker is open ([`TreeSnapshot`]); the picker only
//! filters, draws, and reports the chosen [`TreeTarget`]. The jump itself
//! is applied by the manager (it may focus another window).

use crate::tab::TabId;
use egui::{Context, RichText};
use winit::window::WindowId;

/// Rows drawn before the list scrolls.
const VISIBLE_ROWS: usize = 14;

/// What a row jumps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeTarget {
    Window(WindowId),
    Tab(WindowId, TabId),
    Pane(WindowId, TabId, crate::pane::PaneId),
}

/// One row of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub target: TreeTarget,
    /// 0 window, 1 tab, 2 pane — the indent.
    pub depth: u8,
    /// What the row reads (`Window 2`, `build`, `vim`).
    pub label: String,
    /// Secondary text: session, cwd, `hidden`, agent state.
    pub detail: String,
}

impl TreeRow {
    /// The text the filter matches: label plus detail, so a query can find
    /// a pane by its directory or a tab by its session.
    fn haystack(&self) -> String {
        format!("{} {}", self.label, self.detail)
    }
}

/// The cross-window snapshot, in display order.
pub type TreeSnapshot = Vec<TreeRow>;

/// The tree picker overlay.
#[derive(Default)]
pub struct TreePickerUI {
    pub visible: bool,
    query: String,
    /// Selection and drawn window, on the shared picker (UX.md OV5).
    nav: crate::app::overlay::picker::ListNav,
    request_focus: bool,
    rows: TreeSnapshot,
    /// The live `toggle_tree_picker` chord for the footer (UX.md OV5),
    /// set by the opening action; `None` when unbound.
    toggle_chord: Option<String>,
}

impl TreePickerUI {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the live toggle chord the footer names (it closes the
    /// picker under the overlay stack).
    pub fn set_toggle_chord(&mut self, chord: Option<String>) {
        self.toggle_chord = chord;
    }

    /// The chord the footer names.
    #[cfg(test)]
    pub(crate) fn toggle_chord(&self) -> Option<&str> {
        self.toggle_chord.as_deref()
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.nav.reset();
        self.request_focus = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    /// Test seam: set the filter text as if typed.
    #[cfg(test)]
    pub(crate) fn set_query_for_test(&mut self, query: &str) {
        self.query = query.to_string();
    }

    /// Replace the snapshot (every frame while open).
    pub fn set_rows(&mut self, rows: TreeSnapshot) {
        self.rows = rows;
    }

    /// Rows matching the query, in tree order. An empty query shows the
    /// whole tree; otherwise fuzzy matches rank best first, and a matched
    /// row keeps its depth so the indent still says what it is.
    pub fn filtered(&self) -> Vec<&TreeRow> {
        if self.query.trim().is_empty() {
            return self.rows.iter().collect();
        }
        let mut scored: Vec<(u32, usize, &TreeRow)> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                crate::command_palette::fuzzy::score(&self.query, &row.haystack())
                    .map(|s| (s, i, row))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, row)| row).collect()
    }

    /// Draw the picker; returns the chosen target on Enter or click.
    ///
    /// Keys, scrolling, and the footer come from the shared picker (UX.md
    /// OV5): arrows, PageUp/PageDown, Home/End, Enter jumps, Escape closes.
    pub fn show(&mut self, ctx: &Context) -> Option<TreeTarget> {
        use crate::app::overlay::picker::{self, ListConfig, ListOutcome};

        if !self.visible {
            return None;
        }
        let matches: Vec<TreeRow> = self.filtered().into_iter().cloned().collect();
        let config = ListConfig {
            id: "Open Quickly",
            hint: "Window, tab, pane, or directory",
            visible_rows: VISIBLE_ROWS,
            width: crate::app::overlay::theme::WIDTH_MEDIUM,
            empty_text: "Nothing matches",
            enter_verb: "jump",
            toggle_chord: self.toggle_chord.as_deref(),
            alternates: false,
        };
        let (outcome, query_changed) = picker::show_list(
            ctx,
            &config,
            &mut self.query,
            &mut self.request_focus,
            &mut self.nav,
            matches.len(),
            |ui, index, selected| draw_row(ui, &matches[index], selected),
        );
        if query_changed {
            self.nav.reset();
        }
        match outcome {
            ListOutcome::Chosen { index, .. } => {
                self.close();
                matches.get(index).map(|row| row.target)
            }
            ListOutcome::Closed => {
                self.close();
                None
            }
            ListOutcome::Open => None,
        }
    }
}

/// One tree row: indented by depth, windows bold, detail dimmed.
fn draw_row(ui: &mut egui::Ui, row: &TreeRow, selected: bool) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(f32::from(row.depth) * 16.0);
        let label = if row.depth == 0 {
            RichText::new(&row.label).strong()
        } else {
            RichText::new(&row.label)
        };
        clicked = ui.selectable_label(selected, label).clicked();
        if !row.detail.is_empty() {
            ui.weak(&row.detail);
        }
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(target: TreeTarget, depth: u8, label: &str, detail: &str) -> TreeRow {
        TreeRow {
            target,
            depth,
            label: label.to_string(),
            detail: detail.to_string(),
        }
    }

    #[test]
    fn an_empty_query_lists_the_whole_tree_in_order() {
        let w = WindowId::from(1u64);
        let mut picker = TreePickerUI::new();
        picker.set_rows(vec![
            row(TreeTarget::Window(w), 0, "Window 1", ""),
            row(TreeTarget::Tab(w, 1), 1, "build", ""),
            row(TreeTarget::Pane(w, 1, 1), 2, "cargo", "~/src"),
        ]);
        assert_eq!(picker.filtered().len(), 3);
    }

    #[test]
    fn a_query_matches_detail_as_well_as_label() {
        let w = WindowId::from(1u64);
        let mut picker = TreePickerUI::new();
        picker.set_rows(vec![
            row(TreeTarget::Tab(w, 1), 1, "build", ""),
            row(TreeTarget::Tab(w, 2), 1, "logs", "hidden · par-mux work"),
        ]);
        picker.query = "hidden".to_string();
        let hits = picker.filtered();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target, TreeTarget::Tab(w, 2));
    }

    fn frame(ctx: &Context, picker: &mut TreePickerUI, keys: &[egui::Key]) -> Option<TreeTarget> {
        let events = keys
            .iter()
            .map(|&key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            })
            .collect();
        let mut chosen = None;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| chosen = picker.show(ui.ctx()),
        );
        out.textures_delta.clear();
        chosen
    }

    #[test]
    fn the_shared_picker_keys_drive_the_tree() {
        // UX.md OV5 through show(): End jumps to the last row past the
        // 14-row window and keeps it drawn, Enter jumps to it, and Escape
        // closes without a choice.
        let w = WindowId::from(1u64);
        let rows: Vec<TreeRow> = (0..30)
            .map(|i| row(TreeTarget::Tab(w, i), 1, &format!("tab {i}"), ""))
            .collect();
        let ctx = Context::default();
        let mut picker = TreePickerUI::new();
        picker.set_rows(rows);
        picker.open();
        frame(&ctx, &mut picker, &[]);
        frame(&ctx, &mut picker, &[egui::Key::End]);
        assert!(picker.nav.selected_is_visible(VISIBLE_ROWS));
        assert_eq!(
            frame(&ctx, &mut picker, &[egui::Key::Enter]),
            Some(TreeTarget::Tab(w, 29))
        );
        assert!(!picker.visible, "a jump closes the picker");

        picker.open();
        frame(&ctx, &mut picker, &[]);
        assert_eq!(frame(&ctx, &mut picker, &[egui::Key::Escape]), None);
        assert!(!picker.visible, "Escape closes the picker");
    }
}
