//! Agent-usage popup panel.
//!
//! Detail view over the usage store's snapshot, modeled on the command
//! palette's overlay skeleton (`crate::command_palette`): an egui `Window`
//! anchored center, an action-enum return for the one thing the caller must
//! do (refresh), and Escape closed egui-side so a focused overlay can always
//! dismiss itself. The `agent_usage_panel` key layer is the backstop for
//! Escape and the owner of `h`/`l` (agent switching) — keys that must not
//! reach the PTY while the panel is open.

use crate::agent_usage::records::ModelUsage;
use crate::agent_usage::store::UsageSnapshot;
use chrono::{DateTime, Utc};
use egui::{Context, Frame, Key, Window, epaint::Shadow};

/// Days drawn in the tokens-by-day chart.
const CHART_DAYS: usize = 7;

/// What the panel asked its caller to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelAction {
    /// User pressed `r` — rescan the records directory now.
    RefreshRequested,
}

/// Popup detail panel over the usage snapshot.
pub(crate) struct AgentUsagePanel {
    /// Whether the panel is currently on screen.
    pub(crate) visible: bool,
    /// Index into the snapshot's records; `h`/`l` move it.
    selected_agent: usize,
}

impl Default for AgentUsagePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentUsagePanel {
    /// Build the panel, hidden.
    pub(crate) fn new() -> Self {
        Self {
            visible: false,
            selected_agent: 0,
        }
    }

    /// Show the panel, starting from the first agent.
    pub(crate) fn open(&mut self) {
        self.visible = true;
        self.selected_agent = 0;
    }

    /// Hide the panel. Idempotent — the egui-side Escape and the key layer
    /// backstop can both call it in one frame without harm.
    pub(crate) fn close(&mut self) {
        self.visible = false;
    }

    /// Flip visibility, resetting the agent selection when opening.
    pub(crate) fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    /// Move the agent selection within `agent_count` records. Called by the
    /// key layer (which owns `h`/`l`), so switching works whether or not
    /// egui happens to hold focus.
    pub(crate) fn cycle_agent(&mut self, forward: bool, agent_count: usize) {
        if agent_count == 0 {
            self.selected_agent = 0;
            return;
        }
        let idx = self.selected_agent % agent_count;
        self.selected_agent = if forward {
            (idx + 1) % agent_count
        } else {
            (idx + agent_count - 1) % agent_count
        };
    }

    /// Draw the panel. Returns the action the caller must take, if any.
    pub(crate) fn show(&mut self, ctx: &Context, snap: &UsageSnapshot) -> Option<PanelAction> {
        if !self.visible {
            return None;
        }

        let mut action: Option<PanelAction> = None;

        // Escape closes here, on the egui side, mirroring the palette: the
        // key layer is the backstop for the unfocused case, close() is
        // idempotent, and consume_key keeps Escape away from other widgets.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            self.close();
        }
        // `r` refreshes: a manual rescan is instant (local dir read), so it
        // runs inline rather than through the update-command runner.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::R)) {
            action = Some(PanelAction::RefreshRequested);
        }

        let record = snap.records.get(self.selected_agent);
        self.selected_agent = self
            .selected_agent
            .min(snap.records.len().saturating_sub(1));

        Window::new("Agent Usage")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(Frame::popup(&ctx.global_style()).shadow(Shadow::default()))
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                ui.set_max_width(520.0);

                match record {
                    Some(record) => {
                        // ── Hero ─────────────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.heading(&record.name);
                            if let Some(tier) = &record.tier_label {
                                ui.weak(tier);
                            }
                        });
                        if let Some(status) = &record.usage_status_text
                            && !status.is_empty()
                        {
                            ui.label(status);
                        }

                        ui.add_space(6.0);

                        // ── Limits ───────────────────────────────────────
                        for limit in &record.limits {
                            let label = limit.label.as_deref().unwrap_or("Limit");
                            ui.horizontal(|ui| {
                                ui.label(label);
                                if let Some(percent) = limit.percent {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.weak(format!("{percent:.0}%"));
                                            if let Some(countdown) =
                                                reset_countdown(limit.resets_at.as_deref())
                                            {
                                                ui.weak(countdown);
                                            }
                                        },
                                    );
                                }
                            });
                            if let Some(percent) = limit.percent {
                                draw_meter(ui, (percent / 100.0).clamp(0.0, 1.0));
                            }
                            ui.add_space(4.0);
                        }

                        // ── Prepaid balance ──────────────────────────────
                        if let Some(balance) = &record.balance {
                            ui.label(format!(
                                "Balance: {:.2} {} of {:.2}{} spent",
                                balance.remaining,
                                balance.currency,
                                balance.spent,
                                if balance.estimated { " (est.)" } else { "" }
                            ));
                            let funded = balance.funded;
                            let remaining = balance.remaining;
                            let fraction = if funded > 0.0 {
                                (remaining / funded).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            draw_meter(ui, fraction);
                            ui.add_space(4.0);
                        }

                        // ── Tokens by day ────────────────────────────────
                        if !record.recent_days.is_empty() {
                            ui.weak("Tokens, last days");
                            draw_day_chart(
                                ui,
                                &record.recent_days[..CHART_DAYS.min(record.recent_days.len())],
                            );
                            ui.add_space(4.0);
                        }

                        // ── Tokens by model ──────────────────────────────
                        if !record.model_usage.is_empty() {
                            ui.weak("Tokens by model");
                            let mut rows: Vec<_> = record.model_usage.iter().collect();
                            let total = |u: &ModelUsage| {
                                u.input_tokens
                                    + u.output_tokens
                                    + u.cache_read_input_tokens
                                    + u.cache_creation_input_tokens
                            };
                            rows.sort_by(|a, b| total(b.1).total_cmp(&total(a.1)));
                            for (model, usage) in rows {
                                ui.horizontal(|ui| {
                                    ui.label(model.as_str());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.weak(format_tokens(
                                                usage.input_tokens
                                                    + usage.output_tokens
                                                    + usage.cache_read_input_tokens
                                                    + usage.cache_creation_input_tokens,
                                            ));
                                        },
                                    );
                                });
                            }
                        }

                        // ── Footer ───────────────────────────────────────
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.weak(format!("updated {}", record.updated_at));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| ui.weak("h/l agent · r refresh · Esc close"),
                            );
                        });
                    }
                    None => {
                        ui.heading("Agent Usage");
                        ui.label("No usage records found.");
                        ui.weak(
                            "Collectors write <agent-id>.json into the records directory; \
                             the panel renders whatever appears there.",
                        );
                    }
                }

                for error in &snap.errors {
                    ui.weak(
                        egui::RichText::new(error).color(egui::Color32::from_rgb(220, 120, 90)),
                    );
                }
            });

        action
    }
}

/// Human reset countdown ("3h12m") from an RFC3339 timestamp.
fn reset_countdown(resets_at: Option<&str>) -> Option<String> {
    let raw = resets_at?;
    let parsed = DateTime::parse_from_rfc3339(raw).ok()?;
    let remaining = parsed.signed_duration_since(Utc::now());
    if remaining <= chrono::Duration::zero() {
        return None;
    }
    let mins = remaining.num_minutes();
    Some(if mins >= 60 {
        format!("resets in {}h{}m", mins / 60, mins % 60)
    } else {
        format!("resets in {mins}m")
    })
}

/// Format a token count with k/M/B suffixes.
fn format_tokens(tokens: f64) -> String {
    if tokens >= 1e9 {
        format!("{:.1}B", tokens / 1e9)
    } else if tokens >= 1e6 {
        format!("{:.1}M", tokens / 1e6)
    } else if tokens >= 1e3 {
        format!("{:.1}k", tokens / 1e3)
    } else {
        format!("{tokens:.0}")
    }
}

/// One full-width meter bar drawn with the painter — no plot dependency.
fn draw_meter(ui: &mut egui::Ui, fraction: f64) {
    use egui::{Sense, Vec2};
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 6.0), Sense::hover());
    let track = rect;
    ui.painter()
        .rect_filled(track, 3.0, egui::Color32::from_gray(70));
    let fill_width = track.width() as f64 * fraction.clamp(0.0, 1.0);
    let mut fill = track;
    fill.set_width(fill_width as f32);
    let color = if fraction >= 0.9 {
        egui::Color32::from_rgb(220, 100, 90)
    } else if fraction >= 0.7 {
        egui::Color32::from_rgb(235, 180, 70)
    } else {
        egui::Color32::from_rgb(110, 190, 120)
    };
    ui.painter().rect_filled(fill, 3.0, color);
}

/// Mini bar chart for recent days, drawn with the painter.
fn draw_day_chart(ui: &mut egui::Ui, days: &[crate::agent_usage::records::DayUsage]) {
    use egui::{Sense, Vec2};
    if days.is_empty() {
        return;
    }
    let max = days
        .iter()
        .map(|d| d.tokens)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 40.0), Sense::hover());
    let n = days.len() as f32;
    let slot = rect.width() / n;
    for (i, day) in days.iter().enumerate() {
        let h = (rect.height() as f64 * (day.tokens / max)).clamp(2.0, rect.height() as f64) as f32;
        let x = rect.left() + slot * i as f32 + 2.0;
        let bar = egui::Rect::from_min_size(
            egui::pos2(x, rect.bottom() - h),
            Vec2::new((slot - 4.0).max(1.0), h),
        );
        ui.painter()
            .rect_filled(bar, 1.0, egui::Color32::from_rgb(110, 160, 220));
        ui.painter().text(
            egui::pos2(x + bar.width() / 2.0, rect.bottom() + 8.0),
            egui::Align2::CENTER_TOP,
            day.date
                .rsplit_once('-')
                .map(|(_, d)| d)
                .unwrap_or(&day.date),
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().weak_text_color(),
        );
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden() {
        assert!(!AgentUsagePanel::new().visible);
    }

    #[test]
    fn toggle_flips_visibility_and_open_resets_selection() {
        let mut panel = AgentUsagePanel::new();
        panel.toggle();
        assert!(panel.visible);
        panel.selected_agent = 5;
        panel.close();
        panel.open();
        assert_eq!(panel.selected_agent, 0, "opening resets to the first agent");
        panel.toggle();
        assert!(!panel.visible);
    }

    #[test]
    fn cycle_agent_wraps_both_ways() {
        let mut panel = AgentUsagePanel::new();
        panel.cycle_agent(true, 3);
        assert_eq!(panel.selected_agent, 1);
        panel.cycle_agent(true, 3);
        panel.cycle_agent(true, 3);
        assert_eq!(panel.selected_agent, 0, "forward wraps to first");
        panel.cycle_agent(false, 3);
        assert_eq!(panel.selected_agent, 2, "backward wraps to last");
    }

    #[test]
    fn cycle_agent_with_no_records_is_a_noop() {
        let mut panel = AgentUsagePanel::new();
        panel.cycle_agent(true, 0);
        assert_eq!(panel.selected_agent, 0);
    }

    #[test]
    fn show_holds_the_egui_side_escape_close() {
        // Same reasoning as the palette: while egui holds focus the key
        // layer never sees Escape, so show() must close on the egui side.
        let source = include_str!("panel.rs");
        // Assembled at runtime so the test's own source cannot match itself.
        let needle = ["consume", "_key"].join("");
        let input_call = ["input", "_mut"].join("");
        assert!(
            source.contains(&needle) && source.contains(&input_call),
            "show() must close the panel on the egui-side Escape"
        );
    }

    #[test]
    fn panel_is_registered_as_modal() {
        // The Phase 1 lesson, made structural: an overlay absent from
        // any_modal_ui_visible() leaks keystrokes to the PTY.
        let source = include_str!("../app/window_state/ui_query_helpers.rs");
        let body = source
            .split("fn any_modal_ui_visible")
            .nth(1)
            .expect("any_modal_ui_visible present in ui_query_helpers.rs");
        let body = body.split('}').next().unwrap_or_default();
        assert!(
            body.contains("agent_usage_panel.visible"),
            "agent_usage_panel missing from any_modal_ui_visible — keystrokes \
             leak to the PTY while the panel is open"
        );
    }

    #[test]
    fn reset_countdown_formats_hours_and_minutes() {
        // 2h5m out, so a minute-boundary race cannot round the hours down
        // between constructing the timestamp and reading the countdown.
        let in_2h = (Utc::now() + chrono::Duration::minutes(125)).to_rfc3339();
        let text = reset_countdown(Some(&in_2h)).expect("parses future timestamp");
        assert!(text.starts_with("resets in 2h"), "got: {text}");
    }

    #[test]
    fn reset_countdown_rejects_garbage_and_past() {
        assert_eq!(reset_countdown(None), None);
        assert_eq!(reset_countdown(Some("not a date")), None);
        let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(reset_countdown(Some(&past)), None);
    }

    #[test]
    fn format_tokens_picks_sensible_units() {
        assert_eq!(format_tokens(950.0), "950");
        assert_eq!(format_tokens(8_200_000.0), "8.2M");
        assert_eq!(format_tokens(1.4e9), "1.4B");
    }
}
