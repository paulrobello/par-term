//! par-mux agent-telemetry pushes: `%agent-telemetry-changed {pane} {agent}`.
//!
//! Core 0.64's host-probe sweep broadcasts this identity-only push when a
//! pane's host-attributed project actually changes — the values live on
//! the roster, not the push, so a client's only correct reaction is to
//! re-fetch `list-agents` (the wire shape is the same two-token identity
//! grammar as `%agent-released`, verified against the v0.66.1 tag).
//!
//! Consumed string-level so the code compiles against a core that
//! predates the push: the published pin parses it as `Unknown { line }`,
//! a newer core as the named `AgentTelemetryChanged` variant that `emit`
//! serializes back to the same wire line. An older daemon never sends
//! it, so the feature is dormant there.

use crate::app::window_state::WindowState;
use par_term_emu_core_rust::tmux_control::TmuxNotification as CoreNotification;
use par_term_mux::AgentEntry;
use par_term_tmux::TmuxPaneId;

/// One telemetry push, parsed from its wire line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentTelemetry {
    pub(crate) pane: TmuxPaneId,
    pub(crate) agent: String,
}

/// Parse an `%agent-telemetry-changed %N <agent>` wire line. Identity
/// only: exactly the two tokens after the kind, or the line is not one.
/// Anything else (including a malformed pane id) is `None`.
pub(crate) fn parse_agent_telemetry_line(line: &str) -> Option<AgentTelemetry> {
    let mut fields = line.split_whitespace();
    if fields.next()? != "%agent-telemetry-changed" {
        return None;
    }
    let pane = fields.next()?.strip_prefix('%')?.parse().ok()?;
    let agent = fields.next()?.to_string();
    if fields.next().is_some() {
        return None;
    }
    Some(AgentTelemetry { pane, agent })
}

/// The telemetry push a core notification carries, if any: the raw line
/// of an `Unknown` (published pin), else the named variant re-serialized
/// by the core's own emitter (newer core). `emit` yields an empty string
/// for every variant it does not serialize, which parses to `None`.
pub(crate) fn agent_telemetry_of(notification: &CoreNotification) -> Option<AgentTelemetry> {
    if let CoreNotification::Unknown { line } = notification {
        return parse_agent_telemetry_line(line);
    }
    match notification.notification_type() {
        "agent-telemetry-changed" => {
            parse_agent_telemetry_line(&par_term_emu_core_rust::mux::emit(notification))
        }
        _ => None,
    }
}

impl WindowState {
    /// Apply `%agent-telemetry-changed` pushes: re-fetch the roster
    /// wholesale. The push carries no state, so there is nothing to
    /// merge — the fresh `list-agents` reply is the fact. Same manners
    /// rule as the rest of this drain: a redraw flag at most, never a
    /// focus steal. A failed or refused re-query keeps the current
    /// roster — a push is a hint, not evidence.
    pub(super) fn apply_agent_telemetry(&mut self, pushes: &[AgentTelemetry]) -> bool {
        if pushes.is_empty() {
            return false;
        }
        crate::debug_info!(
            "MUX",
            "agent telemetry push for {} pane(s); re-fetching roster",
            pushes.len()
        );
        let fetched = self
            .tmux_state
            .transport
            .as_ref()
            .and_then(|transport| transport.send_command("list-agents").ok())
            .map(|lines| {
                lines
                    .iter()
                    .filter(|line| !line.trim().is_empty())
                    .filter_map(|line| AgentEntry::parse_list_line(line))
                    .collect::<Vec<_>>()
            });
        match fetched {
            Some(agents) => {
                self.tmux_state.agent_roster.fill_from_list(agents);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_lines_parse_identity_only() {
        let t = parse_agent_telemetry_line("%agent-telemetry-changed %5 claude").unwrap();
        assert_eq!(t.pane, 5);
        assert_eq!(t.agent, "claude");
        // Tolerant of the trailing newline a raw wire line carries.
        assert!(parse_agent_telemetry_line("%agent-telemetry-changed %0 pi\n").is_some());
    }

    #[test]
    fn non_telemetry_and_malformed_lines_are_rejected() {
        assert!(parse_agent_telemetry_line("%agent-state-changed %5 claude").is_none());
        assert!(parse_agent_telemetry_line("%agent-telemetry-changed 5 claude").is_none());
        assert!(parse_agent_telemetry_line("%agent-telemetry-changed %5").is_none());
        assert!(
            parse_agent_telemetry_line("%agent-telemetry-changed %5 claude extra").is_none(),
            "the push is identity-only; a third token is not one"
        );
        assert!(parse_agent_telemetry_line("").is_none());
    }

    #[test]
    fn unknown_variants_yield_their_telemetry() {
        let t = agent_telemetry_of(&CoreNotification::Unknown {
            line: "%agent-telemetry-changed %3 omp".to_string(),
        });
        assert_eq!(t.map(|t| t.pane), Some(3));
        assert!(
            agent_telemetry_of(&CoreNotification::Unknown {
                line: "%sessions-changed".to_string()
            })
            .is_none()
        );
    }
}
