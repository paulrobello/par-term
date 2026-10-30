//! Tests for the agent shader confirmation gate (SEC-003 residual).

use super::{partition_agent_shader_updates, shader_change_needs_confirmation};
use crate::app::window_state::shader_state::{PendingShaderChange, ShaderState};
use serde_json::json;
use std::collections::HashMap;

fn updates_of(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn validate_ok(_name: &str) -> Result<(), String> {
    Ok(())
}

fn validate_reject(name: &str) -> Result<(), String> {
    if name == "good.glsl" {
        Ok(())
    } else {
        Err("outside the shaders directory".to_string())
    }
}

/// Re-sending the active value and `null` are confirmation-free; any other
/// non-null value activates new GPU code and needs confirmation.
#[test]
fn needs_confirmation_matrix() {
    let current = Some("active.glsl".to_string());

    assert!(!shader_change_needs_confirmation(&json!(null), &current));
    assert!(!shader_change_needs_confirmation(&json!(null), &None));
    assert!(!shader_change_needs_confirmation(
        &json!("active.glsl"),
        &current
    ));
    assert!(shader_change_needs_confirmation(
        &json!("other.glsl"),
        &current
    ));
    assert!(shader_change_needs_confirmation(
        &json!("active.glsl"),
        &None
    ));
    // Wrong JSON type follows the apply path's own error, not confirmation.
    assert!(!shader_change_needs_confirmation(&json!(7), &current));
}

#[test]
fn differing_valid_shader_value_is_parked_for_confirmation() {
    let updates = updates_of(&[("custom_shader", json!("new.glsl"))]);
    let (apply_now, pending, notices) =
        partition_agent_shader_updates(&updates, &None, &None, validate_ok);

    assert!(
        !apply_now.contains_key("custom_shader"),
        "must not apply immediately"
    );
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].key, "custom_shader");
    assert_eq!(pending[0].value, "new.glsl");
    assert_eq!(notices.len(), 1);
    assert!(
        notices[0].contains("pending user confirmation"),
        "notice must tell the agent the value is not activated: {notices:?}"
    );
}

#[test]
fn differing_invalid_shader_value_stays_a_rejection() {
    let updates = updates_of(&[("cursor_shader", json!("evil.glsl"))]);
    let (apply_now, pending, notices) =
        partition_agent_shader_updates(&updates, &None, &None, validate_reject);

    assert!(
        apply_now.contains_key("cursor_shader"),
        "invalid value stays in the batch for the apply path's own rejection"
    );
    assert!(
        pending.is_empty(),
        "invalid values must not reach the dialog"
    );
    assert!(
        notices.is_empty(),
        "no duplicate notice beside the apply-path error"
    );
}

#[test]
fn same_value_and_null_stay_in_the_apply_batch() {
    let current = Some("active.glsl".to_string());
    let updates = updates_of(&[
        ("custom_shader", json!("active.glsl")),
        ("cursor_shader", json!(null)),
        ("font_size", json!(13.0)),
    ]);
    let (apply_now, pending, notices) =
        partition_agent_shader_updates(&updates, &current, &current, validate_reject);

    assert!(pending.is_empty(), "no-op shader values must not prompt");
    assert!(notices.is_empty());
    assert!(apply_now.contains_key("custom_shader"));
    assert!(apply_now.contains_key("cursor_shader"));
    assert!(apply_now.contains_key("font_size"));
}

#[test]
fn shader_queue_dedupes_and_answers_the_displayed_change() {
    let mut state = ShaderState::new(std::path::PathBuf::from("/tmp/par-term-test-shaders"));
    assert!(!state.has_pending_shader_changes());

    let first = PendingShaderChange {
        key: "custom_shader".to_string(),
        value: "a.glsl".to_string(),
    };
    let second = PendingShaderChange {
        key: "cursor_shader".to_string(),
        value: "b.glsl".to_string(),
    };
    assert!(state.queue_shader_change(first.clone()));
    assert!(
        !state.queue_shader_change(first.clone()),
        "identical re-send must not stack dialogs"
    );
    assert!(state.queue_shader_change(second.clone()));
    assert_eq!(state.pending_shader_changes.len(), 2);

    // The dialog displays the last queued change and records the answer
    // bound to that exact change.
    state.shader_change_decision = Some((second.clone(), true));
    let (change, approved) = state.take_shader_decision().expect("decision pending");
    assert!(approved);
    assert_eq!(change, second);
    assert_eq!(state.pending_shader_changes.len(), 1);

    state.shader_change_decision = Some((first.clone(), false));
    let (change, approved) = state.take_shader_decision().expect("decision pending");
    assert!(!approved);
    assert_eq!(change, first);
    assert!(!state.has_pending_shader_changes());

    // A decision whose change is no longer pending (already answered or
    // superseded) is dropped instead of answering something else.
    state.shader_change_decision = Some((first, true));
    assert!(state.take_shader_decision().is_none());
    assert!(state.shader_change_decision.is_none());
}
