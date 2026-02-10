//! Progress bar overlay rendering using egui.
//!
//! Renders progress bars from OSC 9;4 (simple) and OSC 934 (named/concurrent)
//! protocols as thin bar overlays at the top or bottom of the terminal window.

use crate::config::{Config, ProgressBarPosition, ProgressBarStyle};
pub use par_term_emu_core_rust::terminal::NamedProgressBar;
use par_term_emu_core_rust::terminal::{ProgressBar, ProgressState};
use std::collections::HashMap;

/// Snapshot of all active progress bars for rendering.
///
/// Captured from the terminal before the mutable renderer borrow
/// to avoid lock contention during egui rendering.
#[derive(Debug, Clone)]
pub struct ProgressBarSnapshot {
    /// Simple progress bar (OSC 9;4)
    pub simple: ProgressBar,
    /// Named progress bars (OSC 934)
    pub named: HashMap<String, NamedProgressBar>,
}

impl ProgressBarSnapshot {
    /// Check if any progress bar is active
    pub fn has_active(&self) -> bool {
        self.simple.is_active() || self.named.values().any(|b| b.state.is_active())
    }
}

/// Render progress bar overlays using egui.
pub fn render_progress_bars(
    ctx: &egui::Context,
    snapshot: &ProgressBarSnapshot,
    config: &Config,
    window_width: f32,
    window_height: f32,
) {
    if !config.progress_bar_enabled || !snapshot.has_active() {
        return;
    }

    let bar_height = config.progress_bar_height;
    let alpha = (config.progress_bar_opacity * 255.0) as u8;

    // Calculate Y position based on config
    let base_y = match config.progress_bar_position {
        ProgressBarPosition::Top => 0.0,
        ProgressBarPosition::Bottom => window_height - bar_height,
    };

    // Collect all active bars: simple bar first, then named bars sorted by ID
    let mut bars: Vec<BarRenderInfo> = Vec::new();

    if snapshot.simple.is_active() {
        bars.push(BarRenderInfo {
            state: snapshot.simple.state,
            percent: snapshot.simple.progress,
            label: None,
        });
    }

    let mut named_sorted: Vec<_> = snapshot
        .named
        .values()
        .filter(|b| b.state.is_active())
        .collect();
    named_sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for bar in named_sorted {
        bars.push(BarRenderInfo {
            state: bar.state,
            percent: bar.percent,
            label: bar.label.as_deref(),
        });
    }

    if bars.is_empty() {
        return;
    }

    // For multiple bars, stack them (each gets its own row)
    let total_height = bar_height * bars.len() as f32;
    let stacked_y = match config.progress_bar_position {
        ProgressBarPosition::Top => base_y,
        ProgressBarPosition::Bottom => window_height - total_height,
    };

    egui::Area::new(egui::Id::new("progress_bar_overlay"))
        .fixed_pos(egui::pos2(0.0, stacked_y))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();

            for (i, bar) in bars.iter().enumerate() {
                let y_offset = i as f32 * bar_height;
                let bar_y = stacked_y + y_offset;

                let color = state_color(bar.state, config, alpha);
                let bg_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, alpha / 2);

                // Draw background track
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(0.0, bar_y),
                        egui::vec2(window_width, bar_height),
                    ),
                    0.0,
                    bg_color,
                );

                if bar.state == ProgressState::Indeterminate {
                    // Animated indeterminate bar: use time-based animation
                    let time = ctx.input(|i| i.time) as f32;
                    let cycle = (time * 1.5).sin() * 0.5 + 0.5; // 0..1 oscillation
                    let bar_w = window_width * 0.3;
                    let x = cycle * (window_width - bar_w);
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x, bar_y),
                            egui::vec2(bar_w, bar_height),
                        ),
                        0.0,
                        color,
                    );
                    // Request repaint for animation
                    ctx.request_repaint();
                } else {
                    // Determinate bar: fill based on percentage
                    let fill_width = window_width * (bar.percent as f32 / 100.0);
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(0.0, bar_y),
                            egui::vec2(fill_width, bar_height),
                        ),
                        0.0,
                        color,
                    );
                }

                // Draw text overlay if style requires it
                if config.progress_bar_style == ProgressBarStyle::BarWithText && bar_height >= 10.0
                {
                    let text = if let Some(label) = bar.label {
                        if bar.state == ProgressState::Indeterminate {
                            label.to_string()
                        } else {
                            format!("{} {}%", label, bar.percent)
                        }
                    } else if bar.state == ProgressState::Indeterminate {
                        String::new()
                    } else {
                        format!("{}%", bar.percent)
                    };

                    if !text.is_empty() {
                        let font_size = (bar_height - 2.0).clamp(8.0, 12.0);
                        let font_id = egui::FontId::new(font_size, egui::FontFamily::Proportional);
                        let text_color = egui::Color32::WHITE;
                        painter.text(
                            egui::pos2(6.0, bar_y + bar_height / 2.0),
                            egui::Align2::LEFT_CENTER,
                            &text,
                            font_id,
                            text_color,
                        );
                    }
                }
            }
        });
}

/// Info needed to render a single progress bar.
struct BarRenderInfo<'a> {
    state: ProgressState,
    percent: u8,
    label: Option<&'a str>,
}

/// Get the color for a progress state from config.
fn state_color(state: ProgressState, config: &Config, alpha: u8) -> egui::Color32 {
    let rgb = match state {
        ProgressState::Normal => config.progress_bar_normal_color,
        ProgressState::Warning => config.progress_bar_warning_color,
        ProgressState::Error => config.progress_bar_error_color,
        ProgressState::Indeterminate => config.progress_bar_indeterminate_color,
        ProgressState::Hidden => [0, 0, 0],
    };
    egui::Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_has_active_empty() {
        let snap = ProgressBarSnapshot {
            simple: ProgressBar::hidden(),
            named: HashMap::new(),
        };
        assert!(!snap.has_active());
    }

    #[test]
    fn test_snapshot_has_active_simple() {
        let snap = ProgressBarSnapshot {
            simple: ProgressBar::normal(50),
            named: HashMap::new(),
        };
        assert!(snap.has_active());
    }

    #[test]
    fn test_snapshot_has_active_named() {
        let mut named = HashMap::new();
        named.insert(
            "test".to_string(),
            NamedProgressBar {
                id: "test".to_string(),
                state: ProgressState::Normal,
                percent: 50,
                label: Some("Testing".to_string()),
            },
        );
        let snap = ProgressBarSnapshot {
            simple: ProgressBar::hidden(),
            named,
        };
        assert!(snap.has_active());
    }

    #[test]
    fn test_state_color_normal() {
        let config = Config::default();
        let color = state_color(ProgressState::Normal, &config, 255);
        assert_eq!(
            color,
            egui::Color32::from_rgba_unmultiplied(
                config.progress_bar_normal_color[0],
                config.progress_bar_normal_color[1],
                config.progress_bar_normal_color[2],
                255,
            )
        );
    }

    #[test]
    fn test_state_color_warning() {
        let config = Config::default();
        let color = state_color(ProgressState::Warning, &config, 200);
        assert_eq!(
            color,
            egui::Color32::from_rgba_unmultiplied(
                config.progress_bar_warning_color[0],
                config.progress_bar_warning_color[1],
                config.progress_bar_warning_color[2],
                200,
            )
        );
    }

    #[test]
    fn test_state_color_error() {
        let config = Config::default();
        let color = state_color(ProgressState::Error, &config, 128);
        assert_eq!(
            color,
            egui::Color32::from_rgba_unmultiplied(
                config.progress_bar_error_color[0],
                config.progress_bar_error_color[1],
                config.progress_bar_error_color[2],
                128,
            )
        );
    }
}
