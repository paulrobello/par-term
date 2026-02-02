//! Parser bridge for tmux control mode
//!
//! This module bridges the core library's `TmuxControlParser` with the frontend's
//! notification types. It converts core library notifications to the frontend's
//! `TmuxNotification` enum and handles pane ID format conversion.

use super::session::TmuxNotification;
use super::types::{TmuxPaneId, TmuxWindowId};

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

            CoreNotification::WindowAdd { window_id } => ParsedId::parse(&window_id)
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
                ..
            } => ParsedId::parse(&window_id)
                .as_window()
                .map(|id| TmuxNotification::LayoutChange {
                    window_id: id,
                    layout: window_layout,
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
                crate::debug_trace!("TMUX", "Unknown notification: {}", line);
                None
            }

            // Terminal output (non-control mode data) - should not happen in gateway mode
            // but if it does, treat as error
            CoreNotification::TerminalOutput { data } => {
                crate::debug_trace!(
                    "TMUX",
                    "Unexpected terminal output in control mode: {} bytes",
                    data.len()
                );
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

#[cfg(test)]
mod tests {
    use super::*;

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
