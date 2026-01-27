use par_term::terminal::TerminalManager;

#[test]
fn test_terminal_creation() {
    let result = TerminalManager::new(80, 24);
    assert!(result.is_ok());
    let terminal = result.unwrap();
    assert_eq!(terminal.dimensions(), (80, 24));
}

#[test]
fn test_terminal_custom_dimensions() {
    let terminal = TerminalManager::new(100, 30).unwrap();
    assert_eq!(terminal.dimensions(), (100, 30));
}

#[test]
#[ignore] // Ignored because spawning a shell causes tests to hang waiting for input
fn test_terminal_spawn_shell() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    let result = terminal.spawn_shell();
    assert!(result.is_ok());
    assert!(terminal.is_running());
}

#[test]
#[ignore] // PTY required for write operations
fn test_terminal_write_string() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    terminal.spawn_shell().unwrap();
    let result = terminal.write_str("Hello, world!");
    assert!(result.is_ok());
}

#[test]
#[ignore] // PTY required for write operations
fn test_terminal_write_bytes() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    terminal.spawn_shell().unwrap();
    let result = terminal.write(b"Hello, world!");
    assert!(result.is_ok());
}

#[test]
fn test_terminal_content() {
    let terminal = TerminalManager::new(80, 24).unwrap();
    // Can get content even without spawning shell
    let content = terminal.content().unwrap();
    // Terminal should return content (even if empty)
    // Just check that it doesn't panic
    let _ = content.len();
}

#[test]
#[ignore] // PTY required for write operations
fn test_terminal_ansi_sequences() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    terminal.spawn_shell().unwrap();
    // Write some ANSI escape sequences
    let result = terminal.write(b"\x1b[1;32mGreen\x1b[0m");
    assert!(result.is_ok());
}

#[test]
fn test_terminal_resize() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    assert_eq!(terminal.dimensions(), (80, 24));

    let result = terminal.resize(100, 30);
    assert!(result.is_ok());
    assert_eq!(terminal.dimensions(), (100, 30));
}

#[test]
fn test_terminal_pty_running() {
    let terminal = TerminalManager::new(80, 24).unwrap();
    // Before spawning shell, PTY should not be running
    assert!(!terminal.is_running());
}

#[test]
fn test_terminal_scrollback() {
    let terminal = TerminalManager::new(80, 24).unwrap();
    // Should be able to get scrollback (even if empty)
    let scrollback = terminal.scrollback();
    // Just check that it doesn't panic
    let _ = scrollback.len();
}

#[test]
#[ignore] // PTY required for write operations
fn test_terminal_multiple_writes() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    terminal.spawn_shell().unwrap();

    terminal.write_str("Line 1\r\n").unwrap();
    terminal.write_str("Line 2\r\n").unwrap();
    terminal.write_str("Line 3\r\n").unwrap();

    let content = terminal.content().unwrap();
    assert!(!content.is_empty());
}

#[test]
#[ignore] // PTY required for write operations
fn test_terminal_control_characters() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    terminal.spawn_shell().unwrap();

    // Test various control characters
    terminal.write(b"\r").unwrap(); // Carriage return
    terminal.write(b"\n").unwrap(); // Line feed
    terminal.write(b"\t").unwrap(); // Tab
    terminal.write(b"\x1b").unwrap(); // Escape

    // Should not panic
    let result = terminal.content();
    assert!(result.is_ok());
}

#[test]
fn test_terminal_large_dimensions() {
    let result = TerminalManager::new(200, 100);
    assert!(result.is_ok());
    let terminal = result.unwrap();
    assert_eq!(terminal.dimensions(), (200, 100));
}

#[test]
fn test_terminal_minimal_dimensions() {
    let result = TerminalManager::new(10, 5);
    assert!(result.is_ok());
    let terminal = result.unwrap();
    assert_eq!(terminal.dimensions(), (10, 5));
}

// ========================================================================
// DECSCUSR (Set Cursor Style) Tests
// ========================================================================

#[test]
fn test_cursor_style_default() {
    let terminal = TerminalManager::new(80, 24).unwrap();
    use par_term_emu_core_rust::cursor::CursorStyle;
    // Default should be blinking block (DECSCUSR 0/1)
    assert_eq!(terminal.cursor_style(), CursorStyle::BlinkingBlock);
}

#[test]
fn test_cursor_style_set_directly() {
    let mut terminal = TerminalManager::new(80, 24).unwrap();
    use par_term_emu_core_rust::cursor::CursorStyle;

    // Test setting steady block
    terminal.set_cursor_style(CursorStyle::SteadyBlock);
    assert_eq!(terminal.cursor_style(), CursorStyle::SteadyBlock);

    // Test setting blinking underline
    terminal.set_cursor_style(CursorStyle::BlinkingUnderline);
    assert_eq!(terminal.cursor_style(), CursorStyle::BlinkingUnderline);

    // Test setting steady bar
    terminal.set_cursor_style(CursorStyle::SteadyBar);
    assert_eq!(terminal.cursor_style(), CursorStyle::SteadyBar);
}

#[test]
fn test_decscusr_escape_sequences() {
    // Test that the terminal correctly parses DECSCUSR sequences
    // These sequences are: CSI Ps SP q where Ps is:
    // 0/1 = blinking block, 2 = steady block
    // 3 = blinking underline, 4 = steady underline
    // 5 = blinking bar, 6 = steady bar

    let terminal = TerminalManager::new(80, 24).unwrap();
    use par_term_emu_core_rust::cursor::CursorStyle;

    // Get access to the underlying terminal to process escape sequences
    let term_arc = terminal.terminal();
    let mut term = term_arc.lock();

    // Test DECSCUSR 0 (default - blinking block)
    term.process(b"\x1b[0 q");
    assert_eq!(term.cursor().style, CursorStyle::BlinkingBlock);

    // Test DECSCUSR 1 (blinking block)
    term.process(b"\x1b[1 q");
    assert_eq!(term.cursor().style, CursorStyle::BlinkingBlock);

    // Test DECSCUSR 2 (steady block)
    term.process(b"\x1b[2 q");
    assert_eq!(term.cursor().style, CursorStyle::SteadyBlock);

    // Test DECSCUSR 3 (blinking underline)
    term.process(b"\x1b[3 q");
    assert_eq!(term.cursor().style, CursorStyle::BlinkingUnderline);

    // Test DECSCUSR 4 (steady underline)
    term.process(b"\x1b[4 q");
    assert_eq!(term.cursor().style, CursorStyle::SteadyUnderline);

    // Test DECSCUSR 5 (blinking bar)
    term.process(b"\x1b[5 q");
    assert_eq!(term.cursor().style, CursorStyle::BlinkingBar);

    // Test DECSCUSR 6 (steady bar)
    term.process(b"\x1b[6 q");
    assert_eq!(term.cursor().style, CursorStyle::SteadyBar);

    // Test reset to default (DECSCUSR with no parameter or 0)
    term.process(b"\x1b[ q");
    assert_eq!(term.cursor().style, CursorStyle::BlinkingBlock);
}

#[test]
fn test_cursor_style_is_blinking() {
    // Helper test to verify which styles should blink
    use par_term_emu_core_rust::cursor::CursorStyle;

    fn is_blinking(style: CursorStyle) -> bool {
        matches!(
            style,
            CursorStyle::BlinkingBlock | CursorStyle::BlinkingUnderline | CursorStyle::BlinkingBar
        )
    }

    // Blinking styles (odd DECSCUSR values: 1, 3, 5)
    assert!(is_blinking(CursorStyle::BlinkingBlock));
    assert!(is_blinking(CursorStyle::BlinkingUnderline));
    assert!(is_blinking(CursorStyle::BlinkingBar));

    // Steady styles (even DECSCUSR values: 2, 4, 6)
    assert!(!is_blinking(CursorStyle::SteadyBlock));
    assert!(!is_blinking(CursorStyle::SteadyUnderline));
    assert!(!is_blinking(CursorStyle::SteadyBar));
}
