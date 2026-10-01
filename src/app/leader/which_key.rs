//! The leader's which-key overlay (UX.md K4, V8): after
//! `leader_overlay_delay_ms` it lists every key the armed table answers,
//! each with its action and that action's live chord (K6: the chord comes
//! from the keybinding registry, so a rebind shows here at once).
//!
//! [`build`] makes a plain snapshot from the live table and registry;
//! [`render`] draws it. The snapshot lives in `OverlayState` so the egui
//! frame closure reads it without borrowing the rest of the window.

use super::ArmedBy;
use super::table::{self, TableKey};
use crate::app::overlay::theme;

/// One overlay row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WhichKeyRow {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) action_id: String,
    /// The action's live chord, when it has one.
    pub(crate) chord: Option<String>,
    pub(crate) repeat: bool,
    /// Runs through tmux's prefix table (a tmux gateway tab).
    pub(crate) via_tmux: bool,
}

/// What the overlay shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WhichKey {
    pub(crate) title: String,
    pub(crate) rows: Vec<WhichKeyRow>,
    pub(crate) footer: String,
}

/// The live facts `build` reads.
pub(crate) struct WhichKeyInputs {
    pub(crate) vim_keys: bool,
    pub(crate) tmux_tab: bool,
    pub(crate) armed_by: ArmedBy,
    /// The leader chord, display form.
    pub(crate) leader_chord: String,
}

/// Build the overlay for the live table. `chord_of` is the registry's
/// live chord for an action; `label_of` its display name.
pub(crate) fn build(
    inputs: &WhichKeyInputs,
    chord_of: impl Fn(&str) -> Option<String>,
    label_of: impl Fn(&str) -> String,
) -> WhichKey {
    let mut rows: Vec<WhichKeyRow> = table::entries(inputs.vim_keys)
        .into_iter()
        .map(|b| {
            let via_tmux = inputs.tmux_tab && table::tmux_key(b.key, inputs.vim_keys).is_some();
            WhichKeyRow {
                key: b.key.label(),
                label: label_of(b.action),
                action_id: b.action.to_string(),
                chord: chord_of(b.action),
                repeat: b.repeat,
                via_tmux,
            }
        })
        .collect();
    if inputs.tmux_tab {
        rows.extend(
            table::tmux_only_keys(inputs.vim_keys)
                .into_iter()
                .map(|key| WhichKeyRow {
                    key: key.label(),
                    label: tmux_label(key),
                    action_id: String::new(),
                    chord: None,
                    repeat: false,
                    via_tmux: true,
                }),
        );
    }
    let title = match inputs.armed_by {
        ArmedBy::Leader => format!("Leader ({})", inputs.leader_chord),
        ArmedBy::TmuxPrefix => "tmux prefix".to_string(),
    };
    let mut footer = format!(
        "{} again sends it to the pane · ↻ stays armed · Esc cancels",
        inputs.leader_chord
    );
    if inputs.tmux_tab {
        footer.push_str(" · tmux: runs in tmux");
    }
    WhichKey {
        title,
        rows,
        footer,
    }
}

/// tmux's name for a prefix key par-term's table leaves to it.
fn tmux_label(key: TableKey) -> String {
    let name = match key {
        TableKey::Char('(') => "Previous tmux session",
        TableKey::Char(')') => "Next tmux session",
        TableKey::Char('L' | 'S') => "Switch to the previously used tmux session",
        TableKey::Char('[') => "tmux copy mode",
        TableKey::Char(']') => "Paste tmux buffer",
        TableKey::Char('{') => "Swap pane up",
        TableKey::Char('}') => "Swap pane down",
        TableKey::Char('t') => "tmux clock",
        TableKey::Char('h') => "Focus pane left",
        TableKey::Char('j') => "Focus pane down",
        TableKey::Char('k') => "Focus pane up",
        _ => return format!("tmux {}", key.label()),
    };
    name.to_string()
}

/// Rows per column before the overlay starts another column.
const ROWS_PER_COLUMN: usize = 18;

/// Draw the overlay bottom-center, above the terminal, in the shared theme.
pub(crate) fn render(ctx: &egui::Context, which_key: Option<&WhichKey>) {
    let Some(which_key) = which_key else {
        return;
    };
    egui::Area::new(egui::Id::new("leader_which_key"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme::TOAST_FILL)
                .stroke(egui::Stroke::new(1.0, theme::ACCENT))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&which_key.title)
                            .color(theme::TEXT)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal_top(|ui| {
                        for (column, rows) in which_key.rows.chunks(ROWS_PER_COLUMN).enumerate() {
                            if column > 0 {
                                ui.add_space(18.0);
                            }
                            egui::Grid::new(("leader_which_key_column", column))
                                .num_columns(3)
                                .spacing(egui::vec2(10.0, 2.0))
                                .show(ui, |ui| {
                                    for row in rows {
                                        render_row(ui, row);
                                        ui.end_row();
                                    }
                                });
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(&which_key.footer)
                            .color(theme::TEXT_MUTED)
                            .small(),
                    );
                });
        });
}

fn render_row(ui: &mut egui::Ui, row: &WhichKeyRow) {
    ui.label(
        egui::RichText::new(&row.key)
            .monospace()
            .color(theme::ACCENT)
            .strong(),
    );
    let mut label = row.label.clone();
    if row.repeat {
        label.push_str(" ↻");
    }
    if row.via_tmux {
        label.push_str(" · tmux");
    }
    ui.label(egui::RichText::new(label).color(theme::TEXT));
    ui.label(
        egui::RichText::new(row.chord.as_deref().unwrap_or(""))
            .color(theme::TEXT_MUTED)
            .small(),
    );
}
