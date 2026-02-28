/// Tests for the script command dispatcher helpers: VT stripping,
/// command tokenisation, and rate-limit logic in `ScriptManager`.
use par_term::scripting::manager::ScriptManager;
use par_term::scripting::protocol::strip_vt_sequences;

// ── strip_vt_sequences ───────────────────────────────────────────────────────

#[test]
fn test_strip_vt_plain_text_unchanged() {
    assert_eq!(strip_vt_sequences("hello world"), "hello world");
}

#[test]
fn test_strip_vt_csi_removed() {
    // ESC [ 32 m (SGR green) … ESC [ 0 m
    assert_eq!(strip_vt_sequences("\x1b[32mgreen\x1b[0m"), "green");
}

#[test]
fn test_strip_vt_osc_removed() {
    // OSC 0 ; title BEL — common window-title escape
    assert_eq!(strip_vt_sequences("\x1b]0;title\x07text"), "text");
}

#[test]
fn test_strip_vt_osc_st_terminated() {
    // OSC sequence terminated by ST (ESC \) instead of BEL
    assert_eq!(strip_vt_sequences("\x1b]0;title\x1b\\after"), "after");
}

#[test]
fn test_strip_vt_dcs_removed() {
    // DCS sequence: ESC P ... ST
    assert_eq!(strip_vt_sequences("\x1bPdcs-data\x1b\\text"), "text");
}

#[test]
fn test_strip_vt_apc_removed() {
    // APC sequence: ESC _ ... ST
    assert_eq!(strip_vt_sequences("\x1b_apc-data\x1b\\after"), "after");
}

#[test]
fn test_strip_vt_bare_esc_sequence_skipped() {
    // ESC M (reverse index) — two-byte sequence, one char skipped
    assert_eq!(strip_vt_sequences("\x1bMtext"), "text");
}

#[test]
fn test_strip_vt_newlines_preserved() {
    assert_eq!(strip_vt_sequences("line1\nline2"), "line1\nline2");
}

#[test]
fn test_strip_vt_mixed() {
    let input = "\x1b[1;32mBold Green\x1b[0m Normal";
    assert_eq!(strip_vt_sequences(input), "Bold Green Normal");
}

#[test]
fn test_strip_vt_empty_string() {
    assert_eq!(strip_vt_sequences(""), "");
}

#[test]
fn test_strip_vt_only_escape_sequence() {
    assert_eq!(strip_vt_sequences("\x1b[2J"), "");
}

// ── ScriptManager rate limiting ──────────────────────────────────────────────

#[test]
fn test_write_text_rate_allows_first_call() {
    let mut mgr = ScriptManager::new();
    // First call should always be allowed
    assert!(mgr.check_write_text_rate(1, 10));
}

#[test]
fn test_write_text_rate_blocks_immediate_second_call() {
    let mut mgr = ScriptManager::new();
    assert!(mgr.check_write_text_rate(1, 10)); // allowed
    // Immediate second call should be blocked (< 100ms interval for 10/s)
    assert!(!mgr.check_write_text_rate(1, 10));
}

#[test]
fn test_run_command_rate_allows_first_call() {
    let mut mgr = ScriptManager::new();
    assert!(mgr.check_run_command_rate(1, 1));
}

#[test]
fn test_run_command_rate_blocks_immediate_second_call() {
    let mut mgr = ScriptManager::new();
    assert!(mgr.check_run_command_rate(1, 1)); // allowed
    assert!(!mgr.check_run_command_rate(1, 1)); // blocked immediately
}

#[test]
fn test_rate_limits_are_independent_per_script() {
    let mut mgr = ScriptManager::new();
    // Script 1 fires
    assert!(mgr.check_write_text_rate(1, 10));
    // Script 2 should still be allowed (independent counter)
    assert!(mgr.check_write_text_rate(2, 10));
    // Script 1 should now be blocked
    assert!(!mgr.check_write_text_rate(1, 10));
    // Script 2 should also be blocked
    assert!(!mgr.check_write_text_rate(2, 10));
}

#[test]
fn test_rate_default_zero_uses_default_rate() {
    let mut mgr = ScriptManager::new();
    // 0 means "use default" — first call always passes
    assert!(mgr.check_write_text_rate(1, 0));
    // Immediate second call is blocked
    assert!(!mgr.check_write_text_rate(1, 0));
}

#[test]
fn test_stop_script_clears_rate_state() {
    let mut mgr = ScriptManager::new();
    // Prime the rate limiter
    assert!(mgr.check_write_text_rate(42, 10));
    assert!(!mgr.check_write_text_rate(42, 10)); // now blocked

    // Stopping the script clears its rate state
    mgr.stop_script(42);

    // After stop+clear, a new id 42 gets a fresh entry
    assert!(mgr.check_write_text_rate(42, 10));
}
