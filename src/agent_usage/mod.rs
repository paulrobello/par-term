//! Agent usage panel: a display surface over a directory of JSON usage
//! records written by external collectors.
//!
//! The file contract is omarchy's (ported verbatim — see
//! `docs/features/AGENT_USAGE.md` once Task 6 lands): one `<agent-id>.json`
//! per agent under the records directory, parsed by [`records`], watched and
//! snapshotted by `store` (Task 2), and rendered by the status-bar widget and
//! popup panel (Tasks 3–4). par-term itself ships no collectors.

pub(crate) mod records;
