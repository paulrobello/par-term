//! Tests for the session logger.

use super::core::{REDACTION_MARKER, SessionLogger};
use super::writers::{contains_password_prompt, html_escape, strip_ansi_escapes};
use crate::config::SessionLogFormat;
use tempfile::TempDir;

#[test]
fn test_strip_ansi_escapes() {
    // Simple text
    assert_eq!(strip_ansi_escapes(b"hello world"), "hello world");

    // CSI sequence (color)
    assert_eq!(strip_ansi_escapes(b"\x1b[32mgreen\x1b[0m"), "green");

    // OSC sequence (title)
    assert_eq!(strip_ansi_escapes(b"\x1b]0;title\x07text"), "text");

    // Multiple sequences
    assert_eq!(
        strip_ansi_escapes(b"\x1b[1;32mBold Green\x1b[0m Normal"),
        "Bold Green Normal"
    );
}

#[test]
fn test_html_escape() {
    assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    assert_eq!(html_escape("a & b"), "a &amp; b");
    assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
}

#[test]
fn test_session_logger_plain() {
    let temp_dir = TempDir::new().unwrap();
    let mut logger = SessionLogger::new(
        SessionLogFormat::Plain,
        temp_dir.path(),
        (80, 24),
        Some("Test Session".to_string()),
    )
    .unwrap();

    logger.start().unwrap();
    logger.record_output(b"Hello, World!\n");
    logger.record_output(b"\x1b[32mGreen text\x1b[0m\n");
    let path = logger.stop().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Hello, World!"));
    assert!(content.contains("Green text"));
    assert!(!content.contains("\x1b")); // No escape sequences
}

#[test]
fn test_session_logger_asciicast() {
    let temp_dir = TempDir::new().unwrap();
    let mut logger = SessionLogger::new(
        SessionLogFormat::Asciicast,
        temp_dir.path(),
        (80, 24),
        Some("Test Session".to_string()),
    )
    .unwrap();

    logger.start().unwrap();
    logger.record_output(b"Hello\n");
    std::thread::sleep(std::time::Duration::from_millis(10));
    logger.record_output(b"World\n");
    let path = logger.stop().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // First line should be header
    assert!(lines[0].contains("\"version\":2"));
    assert!(lines[0].contains("\"width\":80"));
    assert!(lines[0].contains("\"height\":24"));

    // Should have output events
    assert!(lines.len() >= 3);
}

#[test]
fn test_password_prompt_detection() {
    assert!(contains_password_prompt("Password:"));
    assert!(contains_password_prompt("[sudo] password for user:"));
    assert!(contains_password_prompt("Enter passphrase for key:"));
    assert!(contains_password_prompt("Enter PIN:"));
    assert!(contains_password_prompt("some output\nPassword:"));
    assert!(contains_password_prompt("Verification code:"));
    assert!(contains_password_prompt("(current) UNIX password:"));
    // Case insensitive
    assert!(contains_password_prompt("PASSWORD:"));
    assert!(contains_password_prompt("Enter Password:"));
    // Should not match normal output
    assert!(!contains_password_prompt("user@host:~$"));
    assert!(!contains_password_prompt("Hello, World!"));
    assert!(!contains_password_prompt("ls -la"));
}

#[test]
fn test_password_prompt_with_ansi_escapes() {
    // Password prompt with ANSI color codes
    let data = b"\x1b[1;31m[sudo] password for user:\x1b[0m ";
    let stripped = strip_ansi_escapes(data);
    assert!(contains_password_prompt(&stripped));
}

#[test]
fn test_input_redaction_on_password_prompt() {
    let temp_dir = TempDir::new().unwrap();
    let mut logger = SessionLogger::new(
        SessionLogFormat::Asciicast,
        temp_dir.path(),
        (80, 24),
        Some("Test Session".to_string()),
    )
    .unwrap();

    logger.start().unwrap();

    // Normal output and input (should be recorded)
    logger.record_output(b"user@host:~$ ");
    logger.record_input(b"sudo ls\r");

    // Password prompt output
    logger.record_output(b"[sudo] password for user: ");

    // Password input (should be redacted)
    logger.record_input(b"s");
    logger.record_input(b"e");
    logger.record_input(b"c");
    logger.record_input(b"r");
    logger.record_input(b"e");
    logger.record_input(b"t");
    // Enter completes the password
    logger.record_input(b"\r");

    // Normal output after password
    logger.record_output(b"\nfile1  file2  file3\n");
    logger.record_input(b"echo done\r");

    let path = logger.stop().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();

    // The actual password ("secret") should NOT appear in the log
    // Check that no input event contains the password characters individually
    assert!(
        !content.contains("secret"),
        "Password text 'secret' should not appear in the log"
    );
    // The redaction marker SHOULD appear
    assert!(
        content.contains(REDACTION_MARKER),
        "Redaction marker should appear in the log"
    );
    // Normal input should still appear
    assert!(
        content.contains("sudo ls"),
        "Normal input before prompt should be recorded"
    );
    assert!(
        content.contains("echo done"),
        "Normal input after password should be recorded"
    );
}

#[test]
fn test_input_redaction_via_echo_suppressed() {
    let temp_dir = TempDir::new().unwrap();
    let mut logger = SessionLogger::new(
        SessionLogFormat::Asciicast,
        temp_dir.path(),
        (80, 24),
        Some("Test Session".to_string()),
    )
    .unwrap();

    logger.start().unwrap();

    // Normal input
    logger.record_input(b"ls\r");

    // Externally signal echo off
    logger.set_echo_suppressed(true);
    logger.record_input(b"mysecret");
    logger.record_input(b"\r");

    // Echo back on
    logger.set_echo_suppressed(false);
    logger.record_input(b"whoami\r");

    let path = logger.stop().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();

    assert!(!content.contains("mysecret"), "Secret should not appear");
    assert!(
        content.contains(REDACTION_MARKER),
        "Redaction marker should appear"
    );
    assert!(content.contains("whoami"), "Normal input should appear");
}

#[test]
fn test_redaction_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let mut logger = SessionLogger::new(
        SessionLogFormat::Asciicast,
        temp_dir.path(),
        (80, 24),
        Some("Test Session".to_string()),
    )
    .unwrap();

    logger.set_redact_passwords(false);
    logger.start().unwrap();

    // Even with a password prompt, input should NOT be redacted
    logger.record_output(b"Password: ");
    logger.record_input(b"mysecret\r");

    let path = logger.stop().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    // With redaction disabled, the password IS recorded (user's choice)
    assert!(
        content.contains("mysecret"),
        "With redaction disabled, input should be recorded"
    );
    assert!(
        !content.contains(REDACTION_MARKER),
        "No redaction marker when disabled"
    );
}

#[test]
fn test_redaction_marker_emitted_once_per_period() {
    let temp_dir = TempDir::new().unwrap();
    let mut logger = SessionLogger::new(
        SessionLogFormat::Asciicast,
        temp_dir.path(),
        (80, 24),
        Some("Test Session".to_string()),
    )
    .unwrap();

    logger.start().unwrap();
    logger.record_output(b"Password: ");

    // Multiple input events during password entry
    logger.record_input(b"a");
    logger.record_input(b"b");
    logger.record_input(b"c");
    logger.record_input(b"\r");

    let path = logger.stop().unwrap();

    let content = std::fs::read_to_string(&path).unwrap();

    // Count occurrences of the redaction marker
    let marker_count = content.matches(REDACTION_MARKER).count();
    assert_eq!(
        marker_count, 1,
        "Redaction marker should appear exactly once per password entry"
    );
}
