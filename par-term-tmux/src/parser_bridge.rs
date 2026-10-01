//! Parser bridge for tmux control mode
//!
//! This module bridges the core library's `TmuxControlParser` with the frontend's
//! notification types. It converts core library notifications to the frontend's
//! `TmuxNotification` enum and handles pane ID format conversion.

use crate::session::TmuxNotification;
use crate::types::{TmuxPaneId, TmuxWindowId};

/// Parsed ID with the prefix character stripped
#[derive(Debug, Clone)]
pub enum ParsedId {
    /// Pane ID (from %N format)
    Pane(TmuxPaneId),
    /// Window ID (from @N format)
    Window(TmuxWindowId),
    /// Session ID (from $N format)
    Session(u64),
    /// Unparsed string
    Raw(String),
}

impl ParsedId {
    /// Parse an ID string, stripping the prefix character
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        if s.is_empty() {
            return Self::Raw(String::new());
        }

        match s.chars().next() {
            Some('%') => {
                // Pane ID: %N
                s[1..]
                    .parse()
                    .ok()
                    .map_or_else(|| Self::Raw(s.to_string()), Self::Pane)
            }
            Some('@') => {
                // Window ID: @N
                s[1..]
                    .parse()
                    .ok()
                    .map_or_else(|| Self::Raw(s.to_string()), Self::Window)
            }
            Some('$') => {
                // Session ID: $N
                s[1..]
                    .parse()
                    .ok()
                    .map_or_else(|| Self::Raw(s.to_string()), Self::Session)
            }
            _ => Self::Raw(s.to_string()),
        }
    }

    /// Get as pane ID if this is a pane
    pub fn as_pane(&self) -> Option<TmuxPaneId> {
        match self {
            Self::Pane(id) => Some(*id),
            _ => None,
        }
    }

    /// Get as window ID if this is a window
    pub fn as_window(&self) -> Option<TmuxWindowId> {
        match self {
            Self::Window(id) => Some(*id),
            _ => None,
        }
    }
}

/// Bridge for converting core library tmux notifications to frontend types
pub struct ParserBridge;

impl ParserBridge {
    /// Convert a core library TmuxNotification to the frontend's TmuxNotification
    pub fn convert(
        notification: par_term_emu_core_rust::tmux_control::TmuxNotification,
    ) -> Option<TmuxNotification> {
        use par_term_emu_core_rust::tmux_control::TmuxNotification as CoreNotification;

        match notification {
            CoreNotification::SessionChanged {
                session_id: _,
                name,
            } => Some(TmuxNotification::SessionStarted(name)),

            CoreNotification::SessionRenamed {
                session_id: _,
                name,
            } => Some(TmuxNotification::SessionRenamed(name)),

            // `..` ignores fields newer core versions add to `WindowAdd`, so the
            // vendored (local-core) gate keeps compiling when the core grows it.
            CoreNotification::WindowAdd { window_id, .. } => ParsedId::parse(&window_id)
                .as_window()
                .map(TmuxNotification::WindowAdd),

            CoreNotification::WindowClose { window_id } => ParsedId::parse(&window_id)
                .as_window()
                .map(TmuxNotification::WindowClose),

            CoreNotification::WindowRenamed { window_id, name } => ParsedId::parse(&window_id)
                .as_window()
                .map(|id| TmuxNotification::WindowRenamed { id, name }),

            CoreNotification::LayoutChange {
                window_id,
                window_layout,
                window_visible_layout,
                window_raw_flags,
            } => ParsedId::parse(&window_id)
                .as_window()
                .map(|id| TmuxNotification::LayoutChange {
                    window_id: id,
                    layout: window_layout,
                    zoomed: zoomed_pane(&window_raw_flags, &window_visible_layout),
                }),

            CoreNotification::Output { pane_id, data } => ParsedId::parse(&pane_id)
                .as_pane()
                .map(|id| TmuxNotification::Output { pane_id: id, data }),

            CoreNotification::Exit => Some(TmuxNotification::SessionEnded),

            CoreNotification::Pause { pane_id: _ } => Some(TmuxNotification::Pause),

            CoreNotification::Continue => Some(TmuxNotification::Continue),

            CoreNotification::Error {
                timestamp: _,
                command_number: _,
                flags,
            } => Some(TmuxNotification::Error(flags)),

            // Unlinked window events (from other sessions) - we don't track these
            CoreNotification::UnlinkedWindowAdd { .. }
            | CoreNotification::UnlinkedWindowClose { .. }
            | CoreNotification::UnlinkedWindowRenamed { .. } => None,

            // Session-level events we don't handle directly
            CoreNotification::SessionsChanged
            | CoreNotification::SessionWindowChanged { .. }
            | CoreNotification::ClientSessionChanged { .. }
            | CoreNotification::ClientDetached { .. } => None,

            // %begin indicates control mode has started
            CoreNotification::Begin { .. } => Some(TmuxNotification::ControlModeStarted),
            // %end is internal to control mode protocol - ignore it
            CoreNotification::End { .. } => None,

            // Pane mode changes - not handled yet
            CoreNotification::PaneModeChanged { .. } => None,

            // Window pane changed - update focused pane
            CoreNotification::WindowPaneChanged { pane_id, .. } => ParsedId::parse(&pane_id)
                .as_pane()
                .map(|id| TmuxNotification::PaneFocusChanged { pane_id: id }),

            // Extended output (flow control) - treat as regular output
            CoreNotification::ExtendedOutput {
                pane_id,
                delay_ms: _,
                data,
            } => ParsedId::parse(&pane_id)
                .as_pane()
                .map(|id| TmuxNotification::Output { pane_id: id, data }),

            // Subscription changes - not used in gateway mode
            CoreNotification::SubscriptionChanged { .. } => None,

            // Paste buffer changes - could be used for clipboard sync
            CoreNotification::PasteBufferChanged { .. }
            | CoreNotification::PasteBufferDeleted { .. } => None,

            // Unknown notifications
            CoreNotification::Unknown { line } => {
                log::trace!("[TMUX] Unknown notification: {}", line);
                None
            }

            // Terminal output (non-control mode data) - should not happen in gateway mode
            // but if it does, treat as error
            CoreNotification::TerminalOutput { data } => {
                log::trace!(
                    "[TMUX] Unexpected terminal output in control mode: {} bytes",
                    data.len()
                );
                None
            }

            // Core variants this bridge predates (AgentStateChanged arrived
            // with the local 0.50 line) and any future ones: not gateway
            // concerns. A named arm per variant would fail compilation
            // against the published 0.49 pin, and a plain wildcard is
            // unreachable there (an error under -D warnings) — so the arm
            // carries the allow, the one form valid on both versions.
            #[allow(unreachable_patterns)]
            _ => {
                log::trace!("[TMUX] Unhandled core notification variant");
                None
            }
        }
    }

    /// Convert multiple core notifications to frontend notifications
    pub fn convert_all(
        notifications: Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
    ) -> Vec<TmuxNotification> {
        notifications
            .into_iter()
            .filter_map(Self::convert)
            .collect()
    }
}

/// The zoomed pane a `%layout-change` announces: tmux and par-mux both set
/// the `Z` window flag (tmux as part of e.g. `*Z`) and send the zoomed pane
/// alone as the visible layout.
fn zoomed_pane(raw_flags: &str, visible_layout: &str) -> Option<TmuxPaneId> {
    if !raw_flags.contains('Z') {
        return None;
    }
    let visible = crate::types::TmuxLayout::parse(visible_layout)?;
    match visible.pane_ids().as_slice() {
        [only] => Some(*only),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoom_flag_names_the_visible_layouts_only_pane() {
        assert_eq!(zoomed_pane("Z", "0000,80x24,0,0,3"), Some(3));
        assert_eq!(zoomed_pane("*Z", "b25d,80x24,0,0,7"), Some(7), "tmux form");
        assert_eq!(zoomed_pane("", "0000,80x24,0,0,3"), None, "no flag");
        assert_eq!(
            zoomed_pane("Z", "0000,80x24,0,0{40x24,0,0,1,39x24,41,0,2}"),
            None,
            "a multi-pane visible layout is not a zoom"
        );
    }

    #[test]
    fn a_zoomed_layout_change_carries_the_true_tree_and_the_zoomed_pane() {
        use par_term_emu_core_rust::tmux_control::TmuxNotification as Core;
        let converted = ParserBridge::convert(Core::LayoutChange {
            window_id: "@1".into(),
            window_layout: "0000,80x24,0,0{40x24,0,0,1,39x24,41,0,2}".into(),
            window_visible_layout: "0000,80x24,0,0,2".into(),
            window_raw_flags: "Z".into(),
        });
        match converted {
            Some(TmuxNotification::LayoutChange {
                window_id,
                layout,
                zoomed,
            }) => {
                assert_eq!(window_id, 1);
                assert!(layout.contains('{'), "the true tree is kept: {layout}");
                assert_eq!(zoomed, Some(2));
            }
            other => panic!("expected LayoutChange, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_pane_id() {
        assert_eq!(ParsedId::parse("%5").as_pane(), Some(5));
        assert_eq!(ParsedId::parse("%123").as_pane(), Some(123));
        assert!(ParsedId::parse("@5").as_pane().is_none());
    }

    #[test]
    fn test_parse_window_id() {
        assert_eq!(ParsedId::parse("@5").as_window(), Some(5));
        assert_eq!(ParsedId::parse("@123").as_window(), Some(123));
        assert!(ParsedId::parse("%5").as_window().is_none());
    }

    #[test]
    fn test_parse_raw() {
        match ParsedId::parse("invalid") {
            ParsedId::Raw(s) => assert_eq!(s, "invalid"),
            _ => panic!("Expected Raw variant"),
        }
    }
}
