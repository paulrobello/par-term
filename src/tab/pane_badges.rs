//! Tab-bar and pane-title indicators for pane modes (UX.md V5, V6).
//!
//! Pure functions over tab state so the indicator text is unit-testable:
//! `--screenshot` captures skip the egui overlay these are drawn in.

use super::Tab;

/// Zoom indicator glyph (V6). `⤢` is not in egui's default fonts, so the
/// "maximize" frame glyph stands in for it.
pub(crate) const ZOOM_BADGE: &str = "⛶";

/// Broadcast indicator glyph (V5).
pub(crate) const BROADCAST_BADGE: &str = "📡";

impl Tab {
    /// The pane-mode badge the tab bar shows before the title, or `None`.
    /// Broadcast wins over zoom: it changes where typing goes.
    pub(crate) fn pane_mode_badge(&self) -> Option<&'static str> {
        if self.broadcast_input {
            return Some(BROADCAST_BADGE);
        }
        self.pane_manager
            .as_ref()
            .is_some_and(|pm| pm.is_zoomed())
            .then_some(ZOOM_BADGE)
    }

    /// The hover text explaining [`Self::pane_mode_badge`].
    pub(crate) fn pane_mode_badge_tooltip(&self) -> &'static str {
        if self.broadcast_input {
            "Broadcast input: typing and pastes go to every pane in this tab"
        } else {
            "A pane is zoomed to fill this tab"
        }
    }

    /// The panes that receive broadcast input: every pane not opted out.
    pub(crate) fn broadcast_receivers(&self) -> Vec<crate::pane::PaneId> {
        self.pane_manager
            .as_ref()
            .map(|pm| {
                pm.all_panes()
                    .iter()
                    .filter(|p| !p.broadcast_excluded)
                    .map(|p| p.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Bounds of the panes to outline as broadcast receivers (V5), or
    /// empty when broadcast is off. Zoom-aware: only visible panes.
    pub(crate) fn broadcast_outline_bounds(&self) -> Vec<crate::pane::PaneBounds> {
        if !self.broadcast_input {
            return Vec::new();
        }
        let Some(pm) = self.pane_manager.as_ref() else {
            return Vec::new();
        };
        if !pm.has_multiple_panes() {
            return Vec::new();
        }
        pm.visible_panes()
            .iter()
            .filter(|p| !p.broadcast_excluded)
            .map(|p| p.bounds)
            .collect()
    }
}

/// A pane title with its mode markers (V6: zoom).
pub(crate) fn decorate_pane_title(title: String, zoomed: bool) -> String {
    if zoomed {
        format!("{ZOOM_BADGE} {title}")
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoomed_pane_title_carries_the_zoom_marker() {
        assert_eq!(decorate_pane_title("vim".into(), true), "⛶ vim");
        assert_eq!(decorate_pane_title("vim".into(), false), "vim");
    }
}
