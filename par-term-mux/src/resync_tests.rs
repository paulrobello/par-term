//! Tests for the resync/replay parse surface. Sibling module of
//! `resync.rs` (split out when the file crossed the 800-line gate).

#![cfg(test)]

use super::resync::*;
use crate::MuxSessionClient;
use crate::resync_presence::{PanePresence, pane_presence};

fn lines(reply: &[&str]) -> Vec<String> {
    reply.iter().map(|l| l.to_string()).collect()
}

/// Criterion 1 (card "Mux reconnect: replay daemon event ring via
/// `events --since`"): an events reply parses into the head watermark
/// and one event per row, the wire line riding whole (a pane can print
/// any text; the row split is fixed at three fields).
#[test]
fn parses_an_events_reply_into_head_and_event_rows() {
    let replay = parse_events_reply(&lines(&[
        "head 3",
        "1 1730000000000 %window-add @2",
        "2 1730000000001 %pane-exited %3 7",
        "3 1730000000002 %agent-state-changed %3 claude working hook",
    ]))
    .expect("reply parses");
    assert_eq!(replay.head, 3);
    assert_eq!(
        replay.events,
        vec![
            ReplayedEvent {
                seq: 1,
                at_ms: 1730000000000,
                line: "%window-add @2".to_string(),
            },
            ReplayedEvent {
                seq: 2,
                at_ms: 1730000000001,
                line: "%pane-exited %3 7".to_string(),
            },
            ReplayedEvent {
                seq: 3,
                at_ms: 1730000000002,
                line: "%agent-state-changed %3 claude working hook".to_string(),
            },
        ]
    );
}

/// `--since` past the head (a fresh daemon behind a watermark from a
/// dead one) is an empty, healthy reply naming the head as the resume
/// point — not an error.
#[test]
fn an_empty_ring_is_a_head_only_reply() {
    let replay = parse_events_reply(&lines(&["head 0"])).expect("head-only reply parses");
    assert_eq!(replay.head, 0);
    assert!(replay.events.is_empty());
}

/// A reply without the header is an error — an older daemon's
/// `unknown command: events` body must never read as "nothing
/// happened", or the caller would re-baseline its watermark at 0 and
/// lose the resume point on the next attach.
#[test]
fn a_reply_without_a_head_header_is_an_error() {
    assert!(parse_events_reply(&lines(&["unknown command: events"])).is_err());
    assert!(parse_events_reply(&lines(&["%pane-exited %3 7"])).is_err());
    assert!(parse_events_reply(&lines(&[])).is_err());
    // A malformed head value is the same class of failure.
    assert!(parse_events_reply(&lines(&["head x"])).is_err());
}

/// Malformed event rows (missing the timestamp or the wire line) fail
/// loudly rather than fabricating an event.
#[test]
fn malformed_event_rows_are_errors() {
    assert!(parse_events_reply(&lines(&["head 2", "1 1730000000000"])).is_err());
    assert!(parse_events_reply(&lines(&["head 2", "1 1730000000000 "])).is_err());
    assert!(parse_events_reply(&lines(&["head 2", "x 1730000000000 %a"])).is_err());
    // A wire line that is itself text starting with a token the header
    // match would take must stay an event row: only the exact `head`
    // token opens the header.
    let replay = parse_events_reply(&lines(&["head 2", "1 1730000000000 head 99"]))
        .expect("a wire line that begins with `head` is event text");
    assert_eq!(replay.events.len(), 1);
    assert_eq!(replay.events[0].line, "head 99");
}

#[test]
fn pane_info_reply_yields_the_foreground_command() {
    // "c2xlZXA=" is base64("sleep").
    assert_eq!(
        pane_foreground_command(&lines(&["%3 @1 80x24 cmd=c2xlZXA="]), 3).as_deref(),
        Some("sleep")
    );
}

#[test]
fn pane_info_reply_without_a_command_is_unknown() {
    // A daemon that predates the cmd token, an unknown-command error,
    // a reply for a different pane, an empty reply, and bad base64 all
    // read as "no foreground command known".
    assert_eq!(pane_foreground_command(&lines(&["%3 @1 80x24"]), 3), None);
    assert_eq!(
        pane_foreground_command(&lines(&["unknown command: pane-info"]), 3),
        None
    );
    assert_eq!(
        pane_foreground_command(&lines(&["%4 @1 80x24 cmd=c2xlZXA="]), 3),
        None
    );
    assert_eq!(pane_foreground_command(&[], 3), None);
    assert_eq!(
        pane_foreground_command(&lines(&["%3 @1 80x24 cmd=!!!"]), 3),
        None
    );
    assert_eq!(
        pane_foreground_command(&lines(&["%3 @1 80x24 cmd="]), 3),
        None
    );
}

/// Core 0.72 presence tokens parse into their fields; the other tail
/// tokens (`cmd=`, `future=`, `last_input_at=`, `viewers=`) ride the
/// tolerate-unknown path.
#[test]
fn pane_presence_parses_the_tail_tokens() {
    let presence = pane_presence(
            &lines(&[
                "%3 @1 80x24 cmd=c2xlZXA= future=value viewer_count=2 last_viewed_at=1730000002000 cpu_ms=42000 last_input_at=1730000001000",
            ]),
            3,
        )
        .unwrap();
    assert_eq!(presence.viewer_count, Some(2));
    assert_eq!(presence.last_viewed_at, Some(1_730_000_002_000));
    assert_eq!(presence.cpu_ms, Some(42_000));

    // A bare reply (pre-0.72 daemon shape) parses with every field None.
    let bare = pane_presence(&lines(&["%3 @1 80x24 cmd=c2xlZXA="]), 3).unwrap();
    assert_eq!(bare, PanePresence::default());
}

/// ENH-045: the `reflow=` token parses; its absence (a pre-0.84
/// daemon or the daemon's reflow config off) leaves the field None —
/// the daemon-off default mirrors keep legacy reflow under.
#[test]
fn reflow_token_parses_and_absence_reads_none() {
    let on = pane_presence(
        &lines(&["%5 @2 80x24 cmd=c2xlZXA= reflow=1 viewer_count=1"]),
        5,
    )
    .unwrap();
    assert_eq!(on.xterm_resize_reflow, Some(true));

    let explicit_off = pane_presence(&lines(&["%5 @2 80x24 reflow=0"]), 5).unwrap();
    assert_eq!(explicit_off.xterm_resize_reflow, Some(false));

    let absent = pane_presence(&lines(&["%5 @2 80x24 viewer_count=1"]), 5).unwrap();
    assert_eq!(absent.xterm_resize_reflow, None);
}

/// A malformed `reflow=` value degrades to None with a warn while the
/// sibling tokens keep theirs.
#[test]
fn malformed_reflow_token_degrades_to_none() {
    let presence = pane_presence(&lines(&["%7 @1 80x24 reflow=maybe viewer_count=3"]), 7).unwrap();
    assert_eq!(presence.xterm_resize_reflow, None);
    assert_eq!(presence.viewer_count, Some(3));
}

/// A malformed value degrades to None with a warn while the rest of
/// the tail keeps its values — the row is never dropped.
#[test]
fn malformed_presence_tokens_degrade_to_none() {
    let presence = pane_presence(
        &lines(&["%9 @1 80x24 viewer_count=nope last_viewed_at=1730000002000 cpu_ms=nope"]),
        9,
    )
    .unwrap();
    assert_eq!(presence.viewer_count, None);
    assert_eq!(presence.last_viewed_at, Some(1_730_000_002_000));
    assert_eq!(presence.cpu_ms, None);
    let bad_timestamp = pane_presence(
        &lines(&["%9 @1 80x24 viewer_count=0 last_viewed_at=nope cpu_ms=0"]),
        9,
    )
    .unwrap();
    assert_eq!(bad_timestamp.last_viewed_at, None);
    assert_eq!(bad_timestamp.viewer_count, Some(0));
    assert_eq!(bad_timestamp.cpu_ms, Some(0));

    // A reply for another pane, an error body, and an empty reply all
    // read as "no presence known".
    assert_eq!(
        pane_presence(&lines(&["%4 @1 80x24 viewer_count=1"]), 9),
        None
    );
    assert_eq!(
        pane_presence(&lines(&["unknown command: pane-info"]), 9),
        None
    );
    assert_eq!(pane_presence(&[], 9), None);
}

#[test]
fn parses_session_and_window_reply_lines() {
    let (id, name) = parse_id_name_line("$3: dev", '$').expect("session line parses");
    assert_eq!((id, name.as_str()), (3, "dev"));

    let (id, name) = parse_id_name_line("@7: editor", '@').expect("window line parses");
    assert_eq!((id, name.as_str()), (7, "editor"));
}

#[test]
fn rejects_malformed_reply_lines() {
    assert!(parse_id_name_line("3: dev", '$').is_err());
    assert!(parse_id_name_line("$3:dev", '$').is_err());
    assert!(parse_id_name_line("$x: dev", '$').is_err());
}

#[test]
fn parses_workspace_grouped_session_rows() {
    let row = parse_session_row("+0: main: $0: roster").expect("row parses");
    assert_eq!(row.workspace_id, 0);
    assert_eq!(row.workspace_name, "main");
    assert_eq!(row.id, 0);
    assert_eq!(row.name, "roster");

    // Non-zero ids on both levels.
    let row = parse_session_row("+2: other: $5: dev").expect("row parses");
    assert_eq!(row.workspace_id, 2);
    assert_eq!(row.workspace_name, "other");
    assert_eq!(row.id, 5);
    assert_eq!(row.name, "dev");
}

#[test]
fn workspace_names_carrying_marker_text_do_not_confuse_the_parse() {
    // The documented rule: the session id is the LAST `$<digits>:`
    // marker in the row, so a WORKSPACE name imitating one (it precedes
    // the real marker) cannot steal the parse.
    let row = parse_session_row("+0: $7: weird: $1: dev").expect("row parses");
    assert_eq!(row.workspace_id, 0);
    assert_eq!(row.workspace_name, "$7: weird");
    assert_eq!(row.id, 1);
    assert_eq!(row.name, "dev");
}

#[test]
fn parses_a_client_snapshot_reply() {
    let snap = ClientSnapshot::parse(&lines(&[
        "snapshot 1",
        "workspace +0 * main",
        "workspace +1 - lab",
        "session $0 - +0 work",
        "session $1 * +1 spaced name",
        "window @0 * main",
        "pane %0 title with spaces",
        "pane %1",
        "agent %0 claude working hook",
        "future-row-kind x y z",
    ]))
    .expect("snapshot parses");
    assert_eq!(snap.workspaces.len(), 2);
    assert_eq!(snap.workspaces[0].id, 0);
    assert!(snap.workspaces[0].active);
    assert_eq!(snap.workspaces[1].name, "lab");
    assert_eq!(snap.sessions.len(), 2);
    assert_eq!(snap.sessions[0].id, 0);
    assert_eq!(snap.sessions[0].workspace_id, 0);
    assert!(!snap.sessions[0].displayed);
    // The name is the rest of the line, so spaces survive.
    assert_eq!(snap.sessions[1].name, "spaced name");
    assert!(snap.sessions[1].displayed);
}

#[test]
fn snapshot_rows_the_reader_does_not_need_are_ignored() {
    // window/pane/agent/worktree rows and a future header-less
    // garbage line do not break the two rosters the status read
    // needs.
    let snap = ClientSnapshot::parse(&lines(&[
        "snapshot 1",
        "window @0 * x",
        "agent %0 a s src",
        "worktree +0 repo - main /r/x",
    ]))
    .expect("snapshot parses");
    assert!(snap.workspaces.is_empty());
    assert!(snap.sessions.is_empty());
    assert!(snap.git.is_empty());
}

/// Criterion 1 + 4 (card "Adopt client-snapshot git rows and
/// #{pane_title}"): `git` rows parse into the snapshot, a body
/// without them reads as no badges, and a malformed git row is
/// skipped like an unknown kind — never an error.
#[test]
fn parses_git_rows_tolerates_their_absence_and_skips_malformed_ones() {
    let snap = ClientSnapshot::parse(&lines(&[
        "snapshot 1",
        "workspace +0 * main",
        "workspace +1 - lab",
        "git +0 main - -",
        "git +1 feat 2 3",
        "git +2 dev - 1",
        "future-row-kind x y z",
    ]))
    .expect("snapshot parses");
    assert_eq!(
        snap.git,
        vec![
            GitStatusSummary {
                id: 0,
                branch: "main".to_string(),
                ahead: 0,
                behind: 0
            },
            GitStatusSummary {
                id: 1,
                branch: "feat".to_string(),
                ahead: 2,
                behind: 3
            },
            GitStatusSummary {
                id: 2,
                branch: "dev".to_string(),
                ahead: 0,
                behind: 1
            },
        ]
    );

    // The same body without git rows: no badges, and the rosters
    // still parse.
    let snap = ClientSnapshot::parse(&lines(&[
        "snapshot 1",
        "workspace +0 * main",
        "workspace +1 - lab",
        "future-row-kind x y z",
    ]))
    .expect("snapshot parses");
    assert!(snap.git.is_empty());
    assert_eq!(snap.workspaces.len(), 2);

    // Malformed git rows skip like unknown kinds: a `%`-prefixed id,
    // a non-count field, and a missing field are all tolerated.
    let snap = ClientSnapshot::parse(&lines(&[
        "snapshot 1",
        "git %0 main - -",
        "git +0 main x -",
        "git +0 main -",
    ]))
    .expect("snapshot parses");
    assert!(snap.git.is_empty());
}

#[test]
fn snapshot_replies_without_a_v1_header_are_rejected() {
    assert!(ClientSnapshot::parse(&lines(&["unknown command: client-snapshot"])).is_err());
    assert!(ClientSnapshot::parse(&lines(&["snapshot 2", "workspace +0 * m"])).is_err());
    assert!(ClientSnapshot::parse(&lines(&[])).is_err());
}

#[test]
fn the_snapshot_gate_needs_the_v1_token_on_the_client_snapshot_row() {
    assert!(MuxSessionClient::snapshot_supported(&lines(&[
        "capture-pane escape",
        "client-snapshot v1",
        "refresh-client cell-pixels chrome",
        "features replay-held-state",
    ])));
    // No token, an unversioned row, or the token on another command
    // all leave the gate shut.
    assert!(!MuxSessionClient::snapshot_supported(&lines(&[
        "client-snapshot",
        "pane-info cmd v1",
    ])));
    assert!(!MuxSessionClient::snapshot_supported(&lines(&[])));
}

#[test]
fn rejects_malformed_session_rows() {
    // Pre-workspace `$N: name` shape, a bare id, and a missing session
    // marker all fail loudly — the daemon never sends them.
    assert!(parse_session_row("$0: roster").is_err());
    assert!(parse_session_row("+0").is_err());
    assert!(parse_session_row("+0: main: dev").is_err());
}

/// A 4-byte RGBA pixel, base64-encoded.
fn pixel_b64() -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode([0x80u8, 0x40, 0x20, 0xFF])
}

#[test]
fn reply_without_marker_is_all_screen() {
    let reply = lines(&["restore", "lines"]);
    let (screen, graphics) = split_graphics_seed(&reply);
    assert_eq!(screen, vec!["restore".to_string(), "lines".to_string()]);
    assert!(graphics.is_empty());
}

#[test]
fn graphics_section_is_extracted_after_last_marker() {
    let b64 = pixel_b64();
    let reply = lines(&[
        "%graphics-begin", // a pane printed a fake marker; the LAST one wins
        "%gfx 7 1 2 4 2 <not-base64-but-ignored-screen>", // ignored: before the real section
        "%graphics-end",
        "screen continues here",
        "%graphics-begin",
        &format!("%gfx 7 1 2 4 2 {b64}"),
        "%gfx 8 0 0 1 1 AAAA",
        "%graphics-end",
    ]);
    let (screen, graphics) = split_graphics_seed(&reply);
    assert_eq!(
        screen,
        vec![
            "%graphics-begin".to_string(),
            "%gfx 7 1 2 4 2 <not-base64-but-ignored-screen>".to_string(),
            "%graphics-end".to_string(),
            "screen continues here".to_string(),
        ]
    );
    assert_eq!(graphics.len(), 2);
    assert_eq!(graphics[0].id, 7);
    assert_eq!((graphics[0].col, graphics[0].row), (1, 2));
    assert_eq!((graphics[0].width, graphics[0].height), (4, 2));
    assert_eq!(graphics[0].rgba, vec![0x80, 0x40, 0x20, 0xFF]);
    assert_eq!(graphics[1].id, 8);
}

#[test]
fn malformed_gfx_lines_are_skipped() {
    let b64 = pixel_b64();
    let reply = lines(&[
        "%graphics-begin",
        "%gfx",                                 // no fields
        "%gfx 7 1 2 4 2",                       // missing payload
        &format!("%gfx 9 0 0 1 1 {b64} extra"), // trailing field
        &format!("%gfx 10 0 0 1 1 {b64}"),      // the one good row
    ]);
    let (_, graphics) = split_graphics_seed(&reply);
    assert_eq!(graphics.len(), 1);
    assert_eq!(graphics[0].id, 10);
}

#[test]
fn unclosed_section_still_parses_rows() {
    // A pane could print `%graphics-begin` without a closing marker;
    // rows up to EOF parse rather than the whole tail becoming screen.
    let b64 = pixel_b64();
    let reply = lines(&["%graphics-begin", &format!("%gfx 3 0 0 1 1 {b64}")]);
    let (screen, graphics) = split_graphics_seed(&reply);
    assert!(screen.is_empty());
    assert_eq!(graphics.len(), 1);
    assert_eq!(graphics[0].id, 3);
}
