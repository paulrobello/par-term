//! The agent roster wire tier (A2b task 1): the `list-agents` query and
//! the `%agent-state-changed` push payload.
//!
//! The daemon owns agent state (the Phase 5 ruling: hook-fed or
//! scrape-labelled, never inferred); these types are what a client READS,
//! not a collector. Wire shapes:
//!
//! - `list-agents` replies one line per pane a hook has CLAIMED or a rule
//!   has MATCHED: `%N <agent> <state> <source>`, source `hook` or
//!   `scrape`. Panes without either are absent outright — `unknown` means
//!   no report ever happened, never "idle".
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
    /// Why a blocked agent is waiting. The wire cannot carry this yet —
    /// the daemon drops the `message` field pi/omp send until core card
    /// 01a0c77378997ba3bd3016b8db8df98a lands, and claude/codex/grok send
    /// none even then — so every parse today yields `None`. The field
    /// exists so the fill/push paths and their readers are already shaped
    /// for the day it arrives.
    pub reason: Option<String>,
}

impl AgentEntry {
    /// Parse a `list-agents` row: `%N <agent> <state> <source>`. The state
    /// is everything between the agent and the trailing source token, so a
    /// multi-word state survives the same way it does through the push
    /// parser.
    fn parse_list_line(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        let pane = parse_pane_id(parts.next()?)?;
        let agent = parts.next()?.to_string();
        let tail: Vec<&str> = parts.collect();
        // At least a state and the trailing source token.
        if tail.len() < 2 {
            return None;
        }
        let source = AgentSource::parse(tail[tail.len() - 1])?;
        Some(Self {
            pane,
            agent,
            state: tail[..tail.len() - 1].join(" "),
            source,
            reason: None,
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
        assert_eq!(entry.reason, None, "the wire carries no reason yet");
    }

    #[test]
    fn multi_word_state_survives_the_list_parse() {
        let entry = AgentEntry::parse_list_line("%4 claude waiting for input scrape").unwrap();
        assert_eq!(entry.state, "waiting for input");
        assert_eq!(entry.source, AgentSource::Scrape);
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
