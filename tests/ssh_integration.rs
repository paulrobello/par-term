//! Integration tests for the SSH subsystem.

use par_term::profile::Profile;
use par_term::ssh::config_parser::parse_ssh_config_str;
use par_term::ssh::history::parse_ssh_command;
use par_term::ssh::known_hosts::parse_known_hosts_str;
use par_term::ssh::types::{SshHost, SshHostSource};

#[test]
fn test_ssh_config_roundtrip() {
    let config = r#"
Host production
    HostName prod.example.com
    User deploy
    Port 22
    IdentityFile ~/.ssh/id_prod
    ProxyJump bastion

Host staging
    HostName staging.example.com
    User staging
"#;
    let hosts = parse_ssh_config_str(config);
    assert_eq!(hosts.len(), 2);

    let prod = &hosts[0];
    assert_eq!(prod.alias, "production");
    let args = prod.ssh_args();
    assert!(args.contains(&"-J".to_string()));
    assert!(args.contains(&"bastion".to_string()));
    assert!(args.iter().any(|a| a.contains("deploy@")));
}

#[test]
fn test_profile_ssh_command_args() {
    let mut profile = Profile::new("SSH Test");
    profile.ssh_host = Some("server.example.com".to_string());
    profile.ssh_user = Some("admin".to_string());
    profile.ssh_port = Some(2222);
    profile.ssh_identity_file = Some("/home/user/.ssh/id_work".to_string());
    profile.ssh_extra_args = Some("-o StrictHostKeyChecking=no".to_string());

    let args = profile.ssh_command_args().unwrap();
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"2222".to_string()));
    assert!(args.contains(&"-i".to_string()));
    assert!(args.iter().any(|a| a.contains("admin@server.example.com")));
    assert!(args.contains(&"-o".to_string()));
    assert!(args.contains(&"StrictHostKeyChecking=no".to_string()));
}

#[test]
fn test_host_source_display() {
    assert_eq!(SshHostSource::Config.to_string(), "SSH Config");
    assert_eq!(SshHostSource::KnownHosts.to_string(), "Known Hosts");
    assert_eq!(SshHostSource::History.to_string(), "History");
    assert_eq!(SshHostSource::Mdns.to_string(), "mDNS");
}

#[test]
fn test_connection_string_formats() {
    // Just hostname
    let host = SshHost {
        alias: "test".to_string(),
        hostname: Some("example.com".to_string()),
        user: None,
        port: None,
        identity_file: None,
        proxy_jump: None,
        source: SshHostSource::Config,
    };
    assert_eq!(host.connection_string(), "example.com");

    // User@host:port
    let host2 = SshHost {
        alias: "test".to_string(),
        hostname: Some("example.com".to_string()),
        user: Some("admin".to_string()),
        port: Some(2222),
        identity_file: None,
        proxy_jump: None,
        source: SshHostSource::Config,
    };
    assert_eq!(host2.connection_string(), "admin@example.com:2222");
}

#[test]
fn test_known_hosts_and_config_integration() {
    // Test that known_hosts parsing returns hosts with correct source
    let known = parse_known_hosts_str("example.com ssh-rsa AAAA...\n");
    assert_eq!(known.len(), 1);
    assert_eq!(known[0].source, SshHostSource::KnownHosts);

    // Test that config parsing returns hosts with correct source
    let config = parse_ssh_config_str("Host test\n    HostName test.com\n");
    assert_eq!(config.len(), 1);
    assert_eq!(config[0].source, SshHostSource::Config);
}

#[test]
fn test_history_parse_integration() {
    let host = parse_ssh_command("ssh -p 2222 deploy@server.example.com").unwrap();
    assert_eq!(host.source, SshHostSource::History);
    assert_eq!(host.connection_string(), "deploy@server.example.com:2222");

    // Verify ssh_args() produces valid command args
    let args = host.ssh_args();
    assert_eq!(args[0], "-p");
    assert_eq!(args[1], "2222");
    assert_eq!(args[2], "deploy@server.example.com");
}

#[test]
fn test_profile_without_ssh_host() {
    let profile = Profile::new("Regular Profile");
    assert!(profile.ssh_command_args().is_none());
}

#[test]
fn test_profile_ssh_default_port() {
    let mut profile = Profile::new("SSH Default Port");
    profile.ssh_host = Some("server.example.com".to_string());
    profile.ssh_port = Some(22);

    let args = profile.ssh_command_args().unwrap();
    // Default port 22 should NOT include -p flag
    assert!(!args.contains(&"-p".to_string()));
    assert_eq!(args, vec!["server.example.com"]);
}
