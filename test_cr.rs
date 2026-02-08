// Quick test to check carriage return handling
use par_term_emu_core_rust::pty_session::PtySession;
use std::thread;
use std::time::Duration;

fn main() {
    let mut pty = PtySession::new(80, 24, 1000);

    // Spawn a shell
    pty.spawn_shell().expect("Failed to spawn shell");

    // Give shell time to start
    thread::sleep(Duration::from_millis(200));

    // Write some text
    pty.write_str("hello").expect("Failed to write");
    thread::sleep(Duration::from_millis(100));

    // Get cursor position after typing "hello"
    let (col1, row1) = pty.cursor_position();
    println!("After 'hello': cursor at col={}, row={}", col1, row1);

    // Send carriage return
    pty.write(b"\r").expect("Failed to write CR");
    thread::sleep(Duration::from_millis(100));

    // Get cursor position after carriage return
    let (col2, row2) = pty.cursor_position();
    println!("After '\\r': cursor at col={}, row={}", col2, row2);

    if col2 == 0 {
        println!("✓ Carriage return works correctly!");
    } else {
        println!("✗ BUG: Carriage return did NOT move cursor to column 0!");
        println!("  Expected col=0, got col={}", col2);
    }

    // Send line feed
    pty.write(b"\n").expect("Failed to write LF");
    thread::sleep(Duration::from_millis(100));

    let (col3, row3) = pty.cursor_position();
    println!("After '\\n': cursor at col={}, row={}", col3, row3);

    if row3 > row2 {
        println!("✓ Line feed works correctly!");
    } else {
        println!("✗ BUG: Line feed did NOT move cursor down!");
    }
}
