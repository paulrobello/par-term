//! Tests for the pure chat-history scans used by `process_agent_messages_tick`.
//!
//! `old_*` below are verbatim transcriptions of the inline loops these
//! helpers replaced; every generated history must give the same answer.

use super::{
    is_recoverable_tool_failure, scan_recoverable_tool_failure, shader_prompt_missing_config_update,
};
use crate::ai_inspector::chat::ChatMessage;

fn old_scan(messages: &[ChatMessage], initially_detected: bool) -> bool {
    let mut detected = initially_detected;
    if !detected {
        let mut seen_user_boundary = false;
        for msg in messages.iter().rev() {
            if matches!(msg, ChatMessage::User { .. }) {
                seen_user_boundary = true;
                break;
            }
            if let ChatMessage::ToolCall { title, status, .. } = msg {
                let title_l = title.to_ascii_lowercase();
                let status_l = status.to_ascii_lowercase();
                let is_failed = status_l.contains("fail") || status_l.contains("error");
                let is_recoverable_tool = title_l.contains("skill")
                    || title_l == "write"
                    || title_l.starts_with("write ")
                    || title_l.contains(" write ");
                if is_failed && is_recoverable_tool {
                    detected = true;
                    break;
                }
            }
        }
        if !seen_user_boundary {
            detected = false;
        }
    }
    detected
}

fn new_scan(messages: &[ChatMessage], initially_detected: bool) -> bool {
    let mut detected = initially_detected;
    if !detected {
        detected = scan_recoverable_tool_failure(messages);
    }
    detected
}

fn old_shader(messages: &[ChatMessage]) -> bool {
    let mut saw_user_boundary = false;
    let mut saw_config_update_for_prompt = false;
    for msg in messages.iter().rev() {
        match msg {
            ChatMessage::User { .. } => {
                saw_user_boundary = true;
                break;
            }
            ChatMessage::ToolCall { title, .. } => {
                let title_l = title.to_ascii_lowercase();
                if title_l.contains("config_update") {
                    saw_config_update_for_prompt = true;
                    break;
                }
            }
            _ => {}
        }
    }
    saw_user_boundary && !saw_config_update_for_prompt
}

fn tool(title: &str, status: &str) -> ChatMessage {
    ChatMessage::ToolCall {
        tool_call_id: "id".into(),
        title: title.into(),
        kind: "other".into(),
        status: status.into(),
    }
}

fn user() -> ChatMessage {
    ChatMessage::User {
        text: "do it".into(),
        pending: false,
    }
}

/// Message alphabet for the generated histories.
fn alphabet() -> Vec<ChatMessage> {
    vec![
        user(),
        ChatMessage::Agent("ok".into()),
        tool("Skill", "failed"),
        tool("Write", "error"),
        tool("write file.rs", "Failed"),
        tool("Read", "failed"),
        tool("Skill", "completed"),
        tool("mcp__par-term-config__config_update", "completed"),
        tool("CONFIG_UPDATE", "pending"),
        ChatMessage::System("note".into()),
    ]
}

/// Every history of length 0..=4 over the alphabet (11,111 histories).
fn histories() -> Vec<Vec<ChatMessage>> {
    let alpha = alphabet();
    let mut out: Vec<Vec<ChatMessage>> = vec![Vec::new()];
    let mut frontier: Vec<Vec<ChatMessage>> = vec![Vec::new()];
    for _ in 0..4 {
        let mut next = Vec::new();
        for h in &frontier {
            for m in &alpha {
                let mut h2 = h.clone();
                h2.push(m.clone());
                next.push(h2);
            }
        }
        out.extend(next.iter().cloned());
        frontier = next;
    }
    out
}

#[test]
fn skill_failure_scan_matches_the_inline_loop() {
    let all = histories();
    assert_eq!(all.len(), 11_111);
    for h in &all {
        for initially in [false, true] {
            assert_eq!(new_scan(h, initially), old_scan(h, initially), "{h:?}");
        }
    }
}

/// Pins today's behavior: a fresh scan never sets the flag (the failure
/// break happens before the User boundary is seen). See the helper's doc.
#[test]
fn skill_failure_scan_never_sets_the_flag() {
    assert!(!scan_recoverable_tool_failure(&[
        user(),
        tool("Skill", "failed")
    ]));
    assert!(
        histories()
            .iter()
            .all(|h| !scan_recoverable_tool_failure(h))
    );
}

#[test]
fn shader_config_update_check_matches_the_inline_loop() {
    for h in &histories() {
        assert_eq!(
            shader_prompt_missing_config_update(h),
            old_shader(h),
            "{h:?}"
        );
    }
}

#[test]
fn shader_config_update_check_cases() {
    assert!(!shader_prompt_missing_config_update(&[]));
    assert!(shader_prompt_missing_config_update(&[user()]));
    assert!(shader_prompt_missing_config_update(&[
        user(),
        ChatMessage::Agent("done".into())
    ]));
    assert!(!shader_prompt_missing_config_update(&[
        user(),
        tool("config_update", "completed")
    ]));
    assert!(shader_prompt_missing_config_update(&[
        tool("config_update", "completed"),
        user()
    ]));
}

#[test]
fn recoverable_tool_failure_predicate() {
    for (title, status, want) in [
        ("Skill", "failed", true),
        ("use skill foo", "ERROR", true),
        ("Write", "failed", true),
        ("write a.txt", "failed", true),
        ("please write now", "error: io", true),
        ("Writer", "failed", false),
        ("rewrite", "failed", false),
        ("Read", "failed", false),
        ("Skill", "completed", false),
        ("Write", "pending", false),
    ] {
        assert_eq!(
            is_recoverable_tool_failure(title, status),
            want,
            "{title:?} / {status:?}"
        );
    }
}
