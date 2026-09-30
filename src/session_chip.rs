//! The session chip (UX.md V1): the attached par-mux session's name,
//! health, and hidden-tab count at the start of the tab bar, plus the last
//! par-mux error, which stays until the user dismisses it (M12).
//!
//! [`SessionChip`] is a pure snapshot built from window state each frame so
//! its text is unit-testable (`--screenshot` skips the egui overlay it is
//! drawn in); [`show`] draws it and reports clicks.

use egui::{Color32, RichText};

/// Daemon connection health as the chip reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MuxHealth {
    /// Commands complete normally.
    #[default]
    Connected,
    /// Attach in progress.
    Attaching,
    /// A command has been waiting on the daemon past the threshold.
    Unresponsive,
    /// The daemon answered the version check with an older build.
    Stale,
}

impl MuxHealth {
    fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Attaching => "attaching",
            Self::Unresponsive => "not responding",
            Self::Stale => "daemon out of date",
        }
    }

    fn color(self) -> Color32 {
        match self {
            Self::Connected => Color32::from_rgb(76, 175, 80),
            Self::Attaching => Color32::from_rgb(33, 150, 243),
            Self::Unresponsive | Self::Stale => Color32::from_rgb(255, 193, 7),
        }
    }
}

/// One frame's chip content. `None` session and `None` error hides it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionChip {
    /// The attached (or attaching) session's name.
    pub session: Option<String>,
    pub health: MuxHealth,
    /// Tabs of this window's session hidden by a last-pane close (M4/D7).
    pub hidden_tabs: usize,
    /// The last par-mux error, held until dismissed.
    pub last_error: Option<String>,
}

/// What a click on the chip asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionChipAction {
    /// Open the session picker (click on the name).
    OpenPicker,
    /// Dismiss the held error (click on its ×).
    DismissError,
}

impl SessionChip {
    /// Whether the chip draws at all: attached/attaching, or an error held.
    pub fn is_visible(&self) -> bool {
        self.session.is_some() || self.last_error.is_some()
    }

    /// The chip's main text: `⧉ work`, plus `· 2 hidden` and the health
    /// when it is not plain connected.
    pub fn label(&self) -> String {
        let mut text = match &self.session {
            Some(name) => format!("⧉ {name}"),
            None => "⧉ par-mux".to_string(),
        };
        if self.hidden_tabs > 0 {
            text.push_str(&format!(" · {} hidden", self.hidden_tabs));
        }
        if self.session.is_some() && self.health != MuxHealth::Connected {
            text.push_str(&format!(" · {}", self.health.label()));
        }
        text
    }

    /// Hover text: the full state, the held error, and what a click does.
    pub fn tooltip(&self) -> String {
        let mut lines = Vec::new();
        match &self.session {
            Some(name) => {
                lines.push(format!("par-mux session {name} — {}", self.health.label()));
                lines.push("Tabs here survive quitting par-term.".to_string());
            }
            None => lines.push("Not attached to a par-mux session".to_string()),
        }
        if self.hidden_tabs > 0 {
            lines.push(format!(
                "{} hidden tab(s): still running; the session picker and reopen closed tab \
                 show them again",
                self.hidden_tabs
            ));
        }
        if let Some(error) = &self.last_error {
            lines.push(format!("Last error: {error}"));
        }
        lines.push("Click to open the session picker".to_string());
        lines.join("\n")
    }
}

/// Draw the chip into `ui` (the tab bar's row) and return a click action.
pub fn show(ui: &mut egui::Ui, chip: &SessionChip, height: f32) -> Option<SessionChipAction> {
    if !chip.is_visible() {
        return None;
    }
    let mut action = None;
    let color = if chip.last_error.is_some() {
        Color32::from_rgb(244, 67, 54)
    } else {
        chip.health.color()
    };
    let button = egui::Button::new(RichText::new(chip.label()).color(color))
        .min_size(egui::vec2(0.0, height))
        .fill(Color32::TRANSPARENT);
    if ui.add(button).on_hover_text(chip.tooltip()).clicked() {
        action = Some(SessionChipAction::OpenPicker);
    }
    if let Some(error) = &chip.last_error {
        let short: String = error.chars().take(40).collect();
        let ellipsis = if error.chars().count() > 40 {
            "…"
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("{short}{ellipsis}"))
                .color(color)
                .small(),
        )
        .on_hover_text(error);
        if ui
            .add(egui::Button::new("×").fill(Color32::TRANSPARENT))
            .on_hover_text("Dismiss this error")
            .clicked()
        {
            action = Some(SessionChipAction::DismissError);
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chip(session: Option<&str>) -> SessionChip {
        SessionChip {
            session: session.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn hidden_when_local_and_error_free() {
        assert!(!chip(None).is_visible());
    }

    #[test]
    fn shows_the_session_name() {
        let c = chip(Some("work"));
        assert!(c.is_visible());
        assert_eq!(c.label(), "⧉ work");
    }

    #[test]
    fn shows_the_hidden_count_and_health() {
        let c = SessionChip {
            hidden_tabs: 2,
            health: MuxHealth::Unresponsive,
            ..chip(Some("work"))
        };
        assert_eq!(c.label(), "⧉ work · 2 hidden · not responding");
    }

    #[test]
    fn a_held_error_keeps_the_chip_after_the_session_is_gone() {
        let c = SessionChip {
            last_error: Some("par-mux: daemon connection lost".to_string()),
            ..chip(None)
        };
        assert!(c.is_visible(), "the error outlives the attach");
        assert!(c.tooltip().contains("daemon connection lost"));
    }
}
