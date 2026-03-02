//! Transcript file writing helpers for the ACP harness.
//!
//! Provides a thread-safe, append-only transcript file that mirrors all
//! harness output so sessions can be reviewed after the fact.

use std::fmt;
use std::io::Write as _;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Transcript slot
// ---------------------------------------------------------------------------

static TRANSCRIPT_FILE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();

/// Return the global transcript file slot (initialised lazily).
pub fn transcript_slot() -> &'static Mutex<Option<std::fs::File>> {
    TRANSCRIPT_FILE.get_or_init(|| Mutex::new(None))
}

/// Open (or create) the transcript file at `path` and store it in the global
/// slot. Creates parent directories as needed.
pub fn init_transcript(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut guard = transcript_slot()
        .lock()
        .map_err(|_| "Transcript mutex poisoned")?;
    *guard = Some(file);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tee writer
// ---------------------------------------------------------------------------

/// Write a line to stdout and, if a transcript file is open, to it as well.
pub fn println_tee(args: fmt::Arguments<'_>) {
    let mut line = String::new();
    let _ = fmt::write(&mut line, args);

    {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let _ = writeln!(out, "{line}");
    }

    if let Ok(mut guard) = transcript_slot().lock()
        && let Some(file) = guard.as_mut()
    {
        let _ = writeln!(file, "{line}");
        let _ = file.flush();
    }
}
