//! The tree picker (UX.md A15, iTerm2's Open Quickly, `Cmd + Shift + O`):
//! every window, tab, and pane in one fuzzy list — hidden par-mux tabs
//! included — and Enter jumps to the chosen row.
//!
//! The rows come from a cross-window snapshot the window manager builds
//! each frame while the picker is open ([`TreeSnapshot`]); the picker only
//! filters, draws, and reports the chosen [`TreeTarget`]. The jump itself
//! is applied by the manager (it may focus another window).

use crate::tab::TabId;
use egui::{Context, Frame, Key, RichText, Window, epaint::Shadow};
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
    selected: usize,
    scroll_offset: usize,
    request_focus: bool,
    rows: TreeSnapshot,
}

impl TreePickerUI {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.selected = 0;
        self.scroll_offset = 0;
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
    pub fn show(&mut self, ctx: &Context) -> Option<TreeTarget> {
        if !self.visible {
            return None;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.close();
            return None;
        }
        let matches: Vec<TreeRow> = self.filtered().into_iter().cloned().collect();
        if self.selected >= matches.len() {
            self.selected = matches.len().saturating_sub(1);
        }
        let downs = ctx.input(|i| i.num_presses(Key::ArrowDown));
        if downs > 0 && !matches.is_empty() {
            self.selected = (self.selected + downs).min(matches.len() - 1);
        }
        let ups = ctx.input(|i| i.num_presses(Key::ArrowUp));
        if ups > 0 {
            self.selected = self.selected.saturating_sub(ups);
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + VISIBLE_ROWS {
            self.scroll_offset = self.selected + 1 - VISIBLE_ROWS;
        }

        let mut chosen = None;
        if ctx.input(|i| i.key_pressed(Key::Enter))
            && let Some(row) = matches.get(self.selected)
        {
            chosen = Some(row.target);
        }

        Window::new("Open Quickly")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .frame(Frame::popup(&ctx.global_style()).shadow(Shadow::default()))
            .show(ctx, |ui| {
                ui.set_min_width(520.0);
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("Window, tab, pane, or directory")
                        .desired_width(f32::INFINITY),
                );
                if self.request_focus {
                    field.request_focus();
                    self.request_focus = false;
                }
                ui.separator();
                for (i, row) in matches
                    .iter()
                    .enumerate()
                    .skip(self.scroll_offset)
                    .take(VISIBLE_ROWS)
                {
                    let selected = i == self.selected;
                    ui.horizontal(|ui| {
                        ui.add_space(f32::from(row.depth) * 16.0);
                        let label = if row.depth == 0 {
                            RichText::new(&row.label).strong()
                        } else {
                            RichText::new(&row.label)
                        };
                        if ui.selectable_label(selected, label).clicked() {
                            chosen = Some(row.target);
                        }
                        if !row.detail.is_empty() {
                            ui.weak(&row.detail);
                        }
                    });
                }
                if matches.is_empty() {
                    ui.weak("Nothing matches");
                }
            });

        if chosen.is_some() {
            self.close();
        }
        chosen
    }
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
}
