// Test to examine terminal grid content
use par_term_emu_core_rust::pty_session::PtySession;
use std::thread;
use std::time::Duration;

fn print_grid_row(pty: &PtySession, row: usize, cols: usize) {
    let terminal = pty.terminal();
    let term = terminal.lock();
    let grid = term.active_grid();

    let mut line = String::new();
    for col in 0..cols {
        if let Some(cell) = grid.get(col, row) {
            let grapheme = cell.get_grapheme();
            let ch = grapheme.chars().next().unwrap_or('\0');
            if ch == '\0' || ch.is_control() {
                line.push(' ');
            } else {
                line.push_str(&grapheme);
            }
        } else {
            line.push(' ');
        }
    }
    println!("Row {}: |{}|", row, line);
}

fn main() {
    let mut pty = PtySession::new(80, 24, 1000);

    // Spawn shell
    pty.spawn_shell().expect("Failed to spawn shell");
    thread::sleep(Duration::from_millis(500));

    println!("\n=== Initial state ===");
    let (col, row) = pty.cursor_position();
    println!("Cursor: col={}, row={}", col, row);
    for r in 0..5 {
        print_grid_row(&pty, r, 80);
    }

    // Clear any existing content by sending Ctrl+C and waiting
    pty.write(b"\x03").expect("Failed to write Ctrl+C");
    thread::sleep(Duration::from_millis(200));

    println!("\n=== After Ctrl+C ===");
    let (col, row) = pty.cursor_position();
    println!("Cursor: col={}, row={}", col, row);
    for r in 0..5 {
        print_grid_row(&pty, r, 80);
    }

    // Type "test"
    pty.write_str("test").expect("Failed to write");
    thread::sleep(Duration::from_millis(200));

    println!("\n=== After typing 'test' ===");
    let (col, row) = pty.cursor_position();
    println!("Cursor: col={}, row={}", col, row);
    for r in 0..5 {
        print_grid_row(&pty, r, 80);
    }

    // Send Enter (carriage return)
    pty.write(b"\r").expect("Failed to write Enter");
    thread::sleep(Duration::from_millis(200));

    println!("\n=== After pressing Enter ===");
    let (col, row) = pty.cursor_position();
    println!("Cursor: col={}, row={}", col, row);
    for r in 0..5 {
        print_grid_row(&pty, r, 80);
    }

    // Type "abc"
    pty.write_str("abc").expect("Failed to write");
    thread::sleep(Duration::from_millis(200));

    println!("\n=== After typing 'abc' ===");
    let (col, row) = pty.cursor_position();
    println!("Cursor: col={}, row={}", col, row);
    for r in 0..5 {
        print_grid_row(&pty, r, 80);
    }

    println!("\n=== Analysis ===");
    println!("If 'abc' starts at column 0, rendering is correct.");
    println!("If 'abc' starts at same column where 'test' ended, there's a bug.");
}
