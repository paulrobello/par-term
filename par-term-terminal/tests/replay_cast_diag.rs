//! Diagnostic replay harness for the stale-pixel hunt (card 01a198054b5d).
//!
//! Feeds a par-term session recording (asciicast v2, `auto_log_sessions`) through a
//! fresh [`TerminalManager`] grid and prints the resulting screen, so the recorded
//! byte stream can be compared against what the live pane's buffer held. Gated on
//! `REPLAY_CAST` (path to the .cast file); skipped when unset so normal test runs
//! never touch the filesystem.
//!
//! Run: REPLAY_CAST=/path/to/session.cast cargo test -p par-term-terminal \
//!        --test replay_cast_diag -- --nocapture

use std::io::BufRead;

#[test]
fn replay_recorded_bytes_into_a_fresh_grid() {
    let Ok(path) = std::env::var("REPLAY_CAST") else {
        eprintln!("REPLAY_CAST not set; skipping");
        return;
    };
    let file = std::fs::File::open(&path).expect("open recording");
    let reader = std::io::BufReader::new(file);

    // Header line carries the grid the session started at; resizes are not
    // recorded, so a session that resized mid-flight diverges here — that
    // limitation is noted, not modeled.
    let mut lines = reader.lines();
    let header = lines.next().expect("header line").expect("read header");
    let header: serde_json::Value = serde_json::from_str(&header).expect("header json");
    let cols = header["width"].as_u64().expect("width") as usize;
    let rows = header["height"].as_u64().expect("height") as usize;

    // `REPLAY_SNAPSHOT_EVERY` (optional): dump a checksum line per event so
    // intermediate states (a half-rendered window that healed seconds later)
    // can be located in the byte stream; `REPLAY_AT` (optional) stops the
    // replay at that event index and dumps the full grid there.
    let snapshot_every: usize = std::env::var("REPLAY_SNAPSHOT_EVERY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let replay_at: Option<usize> = std::env::var("REPLAY_AT").ok().and_then(|v| v.parse().ok());

    let manager =
        par_term_terminal::TerminalManager::new(cols, rows).expect("fresh grid at recording size");
    let mut events = 0usize;
    let mut total_bytes = 0usize;
    for line in lines {
        let line = line.expect("read event");
        if line.trim().is_empty() {
            continue;
        }
        let event: serde_json::Value = serde_json::from_str(&line).expect("event json");
        if event[1].as_str() != Some("o") {
            continue;
        }
        let data = event[2].as_str().expect("output string");
        manager.process_data(data.as_bytes());
        events += 1;
        total_bytes += data.len();
        if snapshot_every > 0 && events.is_multiple_of(snapshot_every) {
            println!(
                "SNAPSHOT {events} bytes={total_bytes} gridhash={:x}",
                grid_hash(&manager)
            );
        }
        if replay_at.is_some_and(|at| events >= at) {
            eprintln!(
                "stopped at event {events} ({} bytes total), dumping grid",
                total_bytes
            );
            dump_grid(&manager);
            return;
        }
    }

    eprintln!("replayed {events} output events ({total_bytes} bytes) into {cols}x{rows}");
    dump_grid(&manager);
}

/// Cheap whole-grid hash so snapshot lines can be diffed without dumping the
/// full screen per event (card 01a198054b5d).
fn grid_hash(manager: &par_term_terminal::TerminalManager) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::Hash;
    let cells = manager
        .try_get_cells_with_scrollback(0, None, false)
        .expect("uncontended in test");
    let text: String = cells
        .iter()
        .filter(|c| !c.wide_char_spacer)
        .map(|c| c.grapheme.clone())
        .collect();
    text.hash(&mut hasher);
    std::hash::Hasher::finish(&hasher)
}

fn dump_grid(manager: &par_term_terminal::TerminalManager) {
    let (cols, rows) = manager.dimensions();
    let cells = manager
        .try_get_cells_with_scrollback(0, None, false)
        .expect("uncontended in test");
    for row in 0..rows {
        let line: String = cells[row * cols..(row + 1) * cols]
            .iter()
            .filter(|c| !c.wide_char_spacer)
            .map(|c| c.grapheme.clone())
            .collect();
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            println!("{row:3}|{trimmed}|");
        }
    }
}
