//! The agent roster wire tier (A2b task 1): the `list-agents` query and
//! the `%agent-state-changed` push payload.
//!
//! The daemon owns agent state (the Phase 5 ruling: hook-fed or
//! scrape-labelled, never inferred); these types are what a client READS,
//! not a collector. Wire shapes:
//!
//! - `list-agents` replies one line per pane a hook has CLAIMED or a rule
//!   has MATCHED: `%N <agent> <state> <source>` (fixed positions; source
//!   `hook` or `scrape`), then zero or more whitespace-free trailing
//!   tokens — `key=value` per the ARC-060 grammar (`reason=<standard
//!   base64>`, `telemetry=`, `host_telemetry=`, …) or free reason words
//!   from a pre-ARC-060 daemon. Panes without either are absent outright
//!   — `unknown` means no report ever happened, never "idle".
//! - `%agent-state-changed` pushes `{pane} {agent} {state} source={s}`;
//!   core's parser extracts the fields (the pane id keeps its `%` sigil,
//!   the state may be multi-word, `source` is empty when the line carried
//!   no `source=` token).
//!
//! Malformed shapes are skipped and logged, never guessed into entries:
//! a wrong roster entry would render a false state with a claim's
//! confidence.

use crate::client::MuxSessionClient;
use par_term_tmux::TmuxPaneId;
use std::io;

/// Who asserted an agent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSource {
    /// The agent claimed the state about itself (a hook report).
    Hook,
    /// A pattern guessed the state from pane content (a scrape match).
    Scrape,
}

impl AgentSource {
    /// Parse the wire token. Anything else is `None` so the caller skips
    /// the line rather than defaulting — defaulting would relabel a guess
    /// as a claim (or vice versa).
    fn parse(token: &str) -> Option<Self> {
        match token {
            "hook" => Some(Self::Hook),
            "scrape" => Some(Self::Scrape),
            _ => None,
        }
    }
}

/// One pane's rostered agent state — a `list-agents` row and a
/// `%agent-state-changed` payload in one shape, so the app-side cache
/// upserts from either without conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEntry {
    /// The pane the agent runs in (the `%N` id, parsed).
    pub pane: TmuxPaneId,
    /// The agent label (`claude`, `kimi`, …) as the daemon recorded it.
    pub agent: String,
    /// The reported state (`working` / `blocked` / `idle`), verbatim —
    /// par-term renders what it is told and infers nothing.
    pub state: String,
    /// Provenance: a hook CLAIM or a scrape GUESS. Surfaces render them
    /// differently; a guess never shows with a claim's confidence.
    pub source: AgentSource,
    /// Why a blocked agent is waiting. `list-agents` rows carry it as
    /// `reason=<standard base64>` (the ARC-060 grammar; decoded here) or,
    /// from a pre-ARC-060 daemon, as free trailing words. The
    /// `%agent-state-changed` push carries no reason — core's
    /// `AgentStateChanged` has no such field — so a push-refreshed entry
    /// holds `None` until the next roster refetch.
    pub reason: Option<String>,
}

impl AgentEntry {
    /// Parse a `list-agents` row: `%N <agent> <state> <source>` with
    /// fixed positions (the state is a single validated token on the
    /// wire), then zero or more trailing tokens: `key=value` tokens
    /// (`reason=<standard base64>`, `telemetry=`, `host_telemetry=`, …)
    /// per the ARC-060 grammar, or — from a pre-ARC-060 daemon — free
    /// reason words. Fixed positions mean no trailing token can be
    /// mistaken for a column; unknown `key=value` tokens are tolerated
    /// and ignored, and only `reason=` is consumed.
    fn parse_list_line(line: &str) -> Option<Self> {
        use base64::Engine as _;
        use base64::engine::general_purpose::STANDARD;

        let mut parts = line.split_whitespace();
        let pane = parse_pane_id(parts.next()?)?;
        let agent = parts.next()?.to_string();
        let state = parts.next()?.to_string();
        let source = AgentSource::parse(parts.next()?)?;
        let mut reason = None;
        let mut words: Vec<&str> = Vec::new();
        for token in parts {
            if let Some(encoded) = token.strip_prefix("reason=") {
                reason = match STANDARD.decode(encoded) {
                    Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
                    Err(e) => {
                        log::warn!("[MUX] undecodable reason token {token:?}: {e}");
                        None
                    }
                };
            } else if !token.contains('=') {
                words.push(token);
            }
        }
        if reason.is_none() && !words.is_empty() {
            reason = Some(words.join(" "));
        }
        Some(Self {
            pane,
            agent,
            state,
            source,
            reason,
        })
    }

    /// Build the entry a `%agent-state-changed` push carries, from the
    /// fields core's parser already extracted. Returns `None` for shapes
    /// that cannot be trusted as-is: an unparsable pane id, or a
    /// missing/unknown source — an unattributed state must not be
    /// relabelled `hook` by the reader.
    pub fn from_push(pane_id: &str, agent: &str, state: &str, source: &str) -> Option<Self> {
        Some(Self {
            pane: parse_pane_id(pane_id)?,
            agent: agent.to_string(),
            state: state.to_string(),
            source: AgentSource::parse(source)?,
            reason: None,
        })
    }
}

impl MuxSessionClient {
    /// The roster: one [`AgentEntry`] per line of the `list-agents`
    /// reply. Malformed lines are skipped with a warn — one bad row must
    /// not blank the roster when every other row is still true — and an
    /// empty reply is a valid empty roster, not an error.
    pub fn list_agents(&mut self) -> io::Result<Vec<AgentEntry>> {
        let lines = self.send("list-agents")?;
        Ok(lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| match AgentEntry::parse_list_line(line) {
                Some(entry) => Some(entry),
                None => {
                    log::warn!("[MUX] skipping malformed list-agents line: {line:?}");
                    None
                }
            })
            .collect())
    }
}

/// `%N` → `N` (pane ids carry their sigil on the wire).
fn parse_pane_id(token: &str) -> Option<TmuxPaneId> {
    token.strip_prefix('%')?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn parses_list_lines_with_source() {
        let entry = AgentEntry::parse_list_line("%3 kimi working hook").unwrap();
        assert_eq!(
            (
                entry.pane,
                entry.agent.as_str(),
                entry.state.as_str(),
                entry.source
            ),
            (3, "kimi", "working", AgentSource::Hook)
        );
        assert_eq!(entry.reason, None, "no reason token on this row");
    }

    #[test]
    fn trailing_reason_words_survive_the_list_parse() {
        // Pre-ARC-060 daemon shape: free-text reason words after the
        // source. Positions are fixed, so they cannot collide with a
        // column the way the old last-token source parse did.
        let entry =
            AgentEntry::parse_list_line("%4 claude blocked scrape waiting for input").unwrap();
        assert_eq!(entry.state, "blocked");
        assert_eq!(entry.source, AgentSource::Scrape);
        assert_eq!(entry.reason.as_deref(), Some("waiting for input"));
    }

    #[test]
    fn reason_token_decodes_base64() {
        // ARC-060 grammar: `reason=<standard base64>`, one
        // whitespace-free token — a reason containing `telemetry=` or
        // ending in `hook` still cannot collide with a column.
        let reason = "waiting on telemetry=x hook";
        let encoded = base64::engine::general_purpose::STANDARD.encode(reason.as_bytes());
        let entry =
            AgentEntry::parse_list_line(&format!("%5 pi blocked hook reason={encoded}")).unwrap();
        assert_eq!(entry.state, "blocked");
        assert_eq!(entry.source, AgentSource::Hook);
        assert_eq!(entry.reason.as_deref(), Some(reason));
    }

    #[test]
    fn telemetry_tokens_do_not_drop_the_row() {
        let entry = AgentEntry::parse_list_line(
            "%6 claude working hook telemetry=eyJhIjoxfQ== host_telemetry=aG9zdA==",
        )
        .unwrap();
        assert_eq!(entry.state, "working");
        assert_eq!(entry.source, AgentSource::Hook);
        assert_eq!(entry.reason, None);
    }

    #[test]
    fn malformed_list_lines_are_rejected_not_guessed() {
        assert!(AgentEntry::parse_list_line("%3 kimi working").is_none()); // no source
        assert!(AgentEntry::parse_list_line("%3 kimi working daemon").is_none()); // unknown source
        assert!(AgentEntry::parse_list_line("3 kimi working hook").is_none()); // no sigil
        assert!(AgentEntry::parse_list_line("%x kimi working hook").is_none()); // bad id
    }

    #[test]
    fn push_entries_parse_and_reject_the_same_way() {
        let entry = AgentEntry::from_push("%0", "claude", "blocked", "hook").unwrap();
        assert_eq!((entry.pane, entry.state.as_str()), (0, "blocked"));
        // An empty source (no `source=` token on the wire) must not
        // silently become a hook claim.
        assert!(AgentEntry::from_push("%0", "claude", "blocked", "").is_none());
    }
}
