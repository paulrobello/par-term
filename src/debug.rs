/// Comprehensive debugging infrastructure for par-term
///
/// # Dual Logging Systems (ARC-008)
///
/// par-term intentionally runs **two parallel logging systems** that both funnel into the
/// same log file. This coexistence is by design, not accident:
///
/// 1. **Custom debug macros** (`crate::debug_info!()`, `crate::debug_log!()`, etc.)
///    - Controlled by `DEBUG_LEVEL` environment variable (0-4)
///    - Best for high-frequency rendering/input logging with category tags
///    - Categories (e.g., `"RENDER"`, `"TAB"`, `"SHADER"`) allow selective filtering
///    - Zero overhead when `DEBUG_LEVEL=0` (the default)
///
/// 2. **Standard `log` crate** (`log::info!()`, `log::warn!()`, etc.)
///    - Level set by `--log-level` CLI flag, config `log_level`, or `RUST_LOG`
///    - Used by application lifecycle code (startup, config load, errors)
///    - Required for third-party crates (wgpu, tokio, etc.) that emit via `log`
///
/// ## Why Both Systems Coexist
///
/// The custom macros predate widespread `tracing` adoption and were purpose-built for
/// GPU-loop debugging where `RUST_LOG=debug` would produce millions of lines per second.
/// The category/level system lets a developer write `DEBUG_LEVEL=3` and see only
/// rendering events without drowning in tokio internals.
///
/// The `log` crate is kept because third-party dependencies (wgpu, tokio, egui) emit
/// through it exclusively, and because lifecycle events (startup, config) benefit from
/// the standard `env_logger`/`RUST_LOG` filtering UX.
///
/// ## Migration Path (TODO ARC-008)
///
/// Long-term, both systems should be unified under `tracing` (the modern Rust async-aware
/// tracing framework). Migration path:
///   1. Replace `crate::debug_info!()` macros with `tracing::trace!(category = "RENDER", ...)`
///   2. Replace `log::info!()` calls with `tracing::info!()`
///   3. Bridge `log` crate output to tracing with `tracing_log::LogTracer`
///   4. Use `tracing_subscriber` with `EnvFilter` for unified level/category control
///   5. Keep the file sink; replace the custom `DebugLogger` with a tracing file appender
///
/// This is a non-trivial migration touching ~500 call sites. Do not attempt without
/// a dedicated effort. Until then, the dual system is the accepted state.
///
/// Both write to `<temp_dir>/par_term_debug.log` (respects `$TMPDIR` on Unix, `%TEMP%` on Windows).
/// The log file is always created so that errors are captured even in GUI-only contexts
/// (macOS app bundles, Windows GUI apps) where stderr is invisible.
/// The log file is created with 0600 permissions on Unix (set at creation, not chmod'ed
/// afterwards) and symlink-checked to prevent attacks. If the path already exists and is
/// owned by another user, logging is disabled rather than writing where they can read it.
///
/// When `RUST_LOG` is set, `log` crate output is also mirrored to stderr for terminal debugging.
use parking_lot::Mutex;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Debug level configuration for custom debug macros
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugLevel {
    Off = 0,
    Error = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl DebugLevel {
    fn from_env() -> Self {
        match std::env::var("DEBUG_LEVEL") {
            Ok(val) => match val.trim().parse::<u8>() {
                Ok(0) => DebugLevel::Off,
                Ok(1) => DebugLevel::Error,
                Ok(2) => DebugLevel::Info,
                Ok(3) => DebugLevel::Debug,
                Ok(4) => DebugLevel::Trace,
                _ => DebugLevel::Off,
            },
            Err(_) => DebugLevel::Off,
        }
    }
}

/// Global debug logger that handles both custom debug macros and `log` crate output.
struct DebugLogger {
    /// Level for custom debug macros (controlled by DEBUG_LEVEL)
    level: DebugLevel,
    /// Log file handle (always opened)
    file: Option<std::fs::File>,
    /// Mirror custom debug macros to stderr (true when `RUST_LOG` is set), so
    /// `make run-debug` (which tees stderr to the log file) captures them live.
    mirror_stderr: bool,
}

/// Roll the previous session's log aside to `<log>.1`.
///
/// The log is opened with `truncate(true)`, so without this the crash report the
/// panic hook writes (see `src/session/crash_guard.rs`) is erased by the next
/// launch — which, after a crash, is usually seconds later.
///
/// Rename rather than copy: it is atomic, needs no read of a log that may be
/// large, and leaves the `.1` file with the 0600 mode it was created under.
/// Only one generation is kept; a log worth more than that belongs in a tee.
///
/// Skipped, leaving any existing `.1` intact, when the log is
///
/// - absent, or empty — `make run-debug` pipes through `tee`, which truncates
///   the path before par-term starts, so rotating there would clobber a real
///   previous log with nothing;
/// - not a regular file — a symlink is unlinked by [`open_log_file`] first, and
///   anything else is not ours to move;
/// - owned by another user — [`open_log_file`] refuses to write such a path
///   (SEC-016), and renaming it away would quietly convert that refusal into
///   "log anyway, into a fresh file".
fn rotate_log_file(log_path: &std::path::Path) {
    let Ok(meta) = log_path.symlink_metadata() else {
        return;
    };
    if !meta.is_file() || meta.len() == 0 {
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: `getuid` has no preconditions, takes no arguments and always
        // succeeds.
        if meta.uid() != unsafe { libc::getuid() } {
            return;
        }
    }

    let mut rolled = log_path.as_os_str().to_owned();
    rolled.push(".1");
    let _ = std::fs::rename(log_path, std::path::PathBuf::from(rolled));
}

/// Open (creating or truncating) the debug log with owner-only permissions.
///
/// The previous session's log is first rolled aside by [`rotate_log_file`], so
/// truncation here costs the log of the run before last, not the last one.
///
/// Returns `None` — disabling file logging rather than leaking — when the path
/// cannot be opened safely.
///
/// SEC-010: any existing symlink is unlinked first, and on Unix `O_NOFOLLOW` closes
/// the TOCTOU race between that unlink and the open. On non-Unix platforms the
/// check-then-open approach is the only available option.
///
/// SEC-016: 0600 is requested as the *creation* mode rather than chmod'ed after the
/// open. A post-open chmod leaves the file world-readable for the length of the
/// write window and, in the case that actually matters, fails outright — silently —
/// when the path was pre-created by someone else, because chmod requires ownership.
/// `temp_dir()` is `/tmp` on Linux: shared and world-writable.
fn open_log_file(log_path: &std::path::Path) -> Option<std::fs::File> {
    if log_path
        .symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_file(log_path);
    }

    rotate_log_file(log_path);

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        // O_NOFOLLOW (0x20000 on Linux / 0x100 on macOS) causes open() to fail with
        // ELOOP if the final path component is a symlink, regardless of who created it.
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(log_path)
            .ok()
            .filter(|f| {
                // The creation mode above only applies to a file *this* process
                // creates. If the path was pre-created mode 0666 by another user, the
                // open still succeeds and they keep read access to everything written
                // here, so refuse to log rather than leak. Checked on the open
                // descriptor, so there is no TOCTOU window.
                // SAFETY: `getuid` has no preconditions, takes no arguments and
                // always succeeds.
                let our_uid = unsafe { libc::getuid() };
                match f.metadata() {
                    Ok(meta) if meta.uid() == our_uid => true,
                    Ok(meta) => {
                        eprintln!(
                            "par-term: refusing to write {} — it is owned by uid {}, not \
                             this user. Debug file logging is disabled.",
                            log_path.display(),
                            meta.uid()
                        );
                        false
                    }
                    Err(_) => false,
                }
            })
    }

    #[cfg(not(unix))]
    {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(log_path)
            .ok()
    }
}

impl DebugLogger {
    fn new() -> Self {
        let level = DebugLevel::from_env();

        let file = open_log_file(&log_path());

        let mirror_stderr = std::env::var("RUST_LOG").is_ok();
        let mut logger = DebugLogger {
            level,
            file,
            mirror_stderr,
        };
        logger.write_raw(&format!(
            "\n{}\npar-term log session started at {} (debug_level={:?}, rust_log={})\n{}\n",
            "=".repeat(80),
            get_timestamp(),
            level,
            std::env::var("RUST_LOG").unwrap_or_else(|_| "unset".to_string()),
            "=".repeat(80)
        ));
        logger
    }

    fn write_raw(&mut self, msg: &str) {
        if let Some(ref mut file) = self.file {
            let _ = file.write_all(msg.as_bytes());
            let _ = file.flush();
        }
    }

    /// Write a custom debug macro message (respects DEBUG_LEVEL)
    fn log(&mut self, level: DebugLevel, category: &str, msg: &str) {
        if level <= self.level {
            let timestamp = get_timestamp();
            let level_str = match level {
                DebugLevel::Error => "ERROR",
                DebugLevel::Info => "INFO ",
                DebugLevel::Debug => "DEBUG",
                DebugLevel::Trace => "TRACE",
                DebugLevel::Off => return,
            };
            self.write_raw(&format!(
                "[{}] [{}] [{}] {}\n",
                timestamp, level_str, category, msg
            ));
            // Mirror to stderr during debug runs (RUST_LOG set), so `make run-debug`
            // — which tees stderr to the log file — captures custom-macro output too.
            if self.mirror_stderr {
                eprintln!("[{}] {}: {}", level_str.trim_end(), category, msg);
            }
        }
    }

    /// Write a `log` crate record (always writes to file)
    fn log_record(&mut self, record: &log::Record) {
        let timestamp = get_timestamp();
        let level_str = match record.level() {
            log::Level::Error => "ERROR",
            log::Level::Warn => "WARN ",
            log::Level::Info => "INFO ",
            log::Level::Debug => "DEBUG",
            log::Level::Trace => "TRACE",
        };
        self.write_raw(&format!(
            "[{}] [{}] [{}] {}\n",
            timestamp,
            level_str,
            record.target(),
            record.args()
        ));
    }
}

static LOGGER: OnceLock<Mutex<DebugLogger>> = OnceLock::new();

fn get_logger() -> &'static Mutex<DebugLogger> {
    LOGGER.get_or_init(|| Mutex::new(DebugLogger::new()))
}

fn get_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("SystemTime::now() is always after UNIX_EPOCH");
    format!("{}.{:06}", now.as_secs(), now.subsec_micros())
}

/// Get the path to the debug log file.
pub fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("par_term_debug.log")
}

/// Check if debugging is enabled at given level (for custom debug macros)
pub fn is_enabled(level: DebugLevel) -> bool {
    let logger = get_logger().lock();
    level <= logger.level
}

/// Log a message at specified level (for custom debug macros)
pub fn log(level: DebugLevel, category: &str, msg: &str) {
    let mut logger = get_logger().lock();
    logger.log(level, category, msg);
}

/// Log formatted message (for custom debug macros)
pub fn logf(level: DebugLevel, category: &str, args: fmt::Arguments) {
    if is_enabled(level) {
        log(level, category, &format!("{}", args));
    }
}

/// Write one line to the debug log without ever blocking. Returns whether it
/// was written.
///
/// Every other entry point takes the logger mutex unconditionally, and
/// `parking_lot::Mutex` is not reentrant: a panic raised while that lock is held
/// hangs any hook that then tries to log — the failure mode documented in
/// `src/session/crash_guard.rs`. This drops the line instead of hanging. It also
/// never initializes the logger, because re-entering `OnceLock::get_or_init` on
/// the same thread is its own deadlock.
///
/// Two deliberate differences from [`logf`]:
///
/// - `DEBUG_LEVEL` does not filter the message. The callers this exists for are
///   reporting a fault, not tracing, and at the default `DEBUG_LEVEL=0` a
///   filtered write would produce nothing at all.
/// - Nothing is mirrored to stderr. A panic hook's stderr output is already
///   written by the default hook it chains to.
pub fn try_logf(level: DebugLevel, category: &str, args: fmt::Arguments) -> bool {
    let level_str = match level {
        DebugLevel::Error => "ERROR",
        DebugLevel::Info => "INFO ",
        DebugLevel::Debug => "DEBUG",
        DebugLevel::Trace => "TRACE",
        DebugLevel::Off => return false,
    };
    let Some(logger) = LOGGER.get() else {
        return false;
    };
    let Some(mut logger) = logger.try_lock() else {
        return false;
    };
    logger.write_raw(&format!(
        "[{}] [{}] [{}] {}\n",
        get_timestamp(),
        level_str,
        category,
        args
    ));
    true
}

// ============================================================================
// log crate bridge — routes log::info!() etc. to the debug log file
// ============================================================================

/// Bridge that implements the `log` crate's `Log` trait, routing all log
/// output to the par-term debug log file. Optionally mirrors to stderr
/// when `RUST_LOG` is set (for terminal debugging).
struct LogCrateBridge {
    /// Install-time level derivation (parsed from RUST_LOG, default: Info). The
    /// live acceptance level is [`BRIDGE_MAX_LEVEL`]; this field only feeds
    /// [`init_log_bridge`]'s override decision.
    max_level: log::LevelFilter,
    /// Whether to also write to stderr (true when RUST_LOG is explicitly set)
    mirror_stderr: bool,
    /// Module-level filters (module_prefix, max_level) for noisy crates
    module_filters: Vec<(&'static str, log::LevelFilter)>,
}

impl LogCrateBridge {
    fn new() -> Self {
        let rust_log_set = std::env::var("RUST_LOG").is_ok();
        let max_level = if rust_log_set {
            // Parse RUST_LOG for the default level (simplified: just use the first token)
            match std::env::var("RUST_LOG")
                .unwrap_or_default()
                .to_lowercase()
                .as_str()
            {
                "trace" => log::LevelFilter::Trace,
                "debug" => log::LevelFilter::Debug,
                "info" => log::LevelFilter::Info,
                "warn" => log::LevelFilter::Warn,
                "error" => log::LevelFilter::Error,
                "off" => log::LevelFilter::Off,
                _ => log::LevelFilter::Info, // default if RUST_LOG has module-specific syntax
            }
        } else {
            // No RUST_LOG: capture info and above to the log file
            log::LevelFilter::Info
        };

        LogCrateBridge {
            max_level,
            mirror_stderr: rust_log_set,
            module_filters: vec![
                ("wgpu_core", log::LevelFilter::Warn),
                ("wgpu_hal", log::LevelFilter::Warn),
                ("naga", log::LevelFilter::Warn),
                ("rodio", log::LevelFilter::Error),
                ("cpal", log::LevelFilter::Error),
            ],
        }
    }

    fn level_for_module(&self, target: &str) -> log::LevelFilter {
        for (prefix, filter) in &self.module_filters {
            if target.starts_with(prefix) {
                return *filter;
            }
        }
        bridge_max_level()
    }
}

impl log::Log for LogCrateBridge {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level_for_module(metadata.target())
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // Write to the debug log file
        let mut logger = get_logger().lock();
        logger.log_record(record);
        drop(logger);

        // Mirror to stderr when RUST_LOG is set (for terminal debugging)
        if self.mirror_stderr {
            eprintln!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

/// The bridge's live acceptance level. A process-wide atomic rather than a
/// field because the bridge is boxed into the global logger on install, while
/// `set_log_level` (settings UI, config application) must still be able to move
/// the level afterwards. `log::set_max_level` only gates the macros; every
/// record that passes it is re-checked against this value, so the two must move
/// together or raised records are silently dropped at the bridge.
static BRIDGE_MAX_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(3);

fn store_bridge_max_level(level: log::LevelFilter) {
    let code = match level {
        log::LevelFilter::Off => 0,
        log::LevelFilter::Error => 1,
        log::LevelFilter::Warn => 2,
        log::LevelFilter::Info => 3,
        log::LevelFilter::Debug => 4,
        log::LevelFilter::Trace => 5,
    };
    BRIDGE_MAX_LEVEL.store(code, std::sync::atomic::Ordering::Relaxed);
}

fn bridge_max_level() -> log::LevelFilter {
    match BRIDGE_MAX_LEVEL.load(std::sync::atomic::Ordering::Relaxed) {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    }
}

/// Initialize the `log` crate bridge. Call this once from main() instead of env_logger::init().
/// Routes all `log::info!()` etc. calls to the par-term debug log file.
/// When `RUST_LOG` is set, also mirrors to stderr for terminal debugging.
///
/// `level_override` allows CLI or config to set the level. If `None`, uses
/// `RUST_LOG` env var (or defaults to `Info`).
pub fn init_log_bridge(level_override: Option<log::LevelFilter>) {
    // Force logger initialization (opens the log file)
    let _ = get_logger();

    let bridge = LogCrateBridge::new();
    // CLI/config override takes precedence, then RUST_LOG, then default. The
    // override must reach the bridge's own gate as well: set_max_level below
    // only gates the macros, and the bridge re-checks every record against
    // BRIDGE_MAX_LEVEL — left at the RUST_LOG-derived level (Info when RUST_LOG
    // is unset), it silently drops what the override let through.
    let max_level = level_override.unwrap_or(bridge.max_level);
    store_bridge_max_level(max_level);

    // Install as the global logger
    if log::set_boxed_logger(Box::new(bridge)).is_ok() {
        log::set_max_level(max_level);
    }
}

/// Update the log level at runtime (e.g., from settings UI).
/// Moves both the macros' global gate and the bridge's per-record gate —
/// raising only `log::max_level()` would be undone at the bridge's own
/// `enabled()` check.
pub fn set_log_level(level: log::LevelFilter) {
    store_bridge_max_level(level);
    log::set_max_level(level);
}

// ============================================================================
// try_lock failure telemetry
// ============================================================================

/// Total number of `try_lock()` calls that returned `Err` (lock contended) across all
/// call sites in the application.  Incremented via [`record_try_lock_failure`].
///
/// Deliberately a module-level static so it is zero-cost when the counter is never
/// read — the increment itself is a single `fetch_add(Relaxed)`.
pub static TRY_LOCK_FAILURE_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// The value of [`TRY_LOCK_FAILURE_COUNT`] at the time of the last periodic telemetry
/// log.  Used by [`maybe_log_try_lock_telemetry`] to avoid emitting redundant log lines.
static TRY_LOCK_LAST_REPORTED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Record one `try_lock()` failure.
///
/// Increments [`TRY_LOCK_FAILURE_COUNT`] and emits a `CONCURRENCY` debug-log entry
/// (visible at `DEBUG_LEVEL >= 3`).
///
/// # Arguments
/// * `site` - A short, human-readable label identifying the call site
///   (e.g., `"resize"`, `"theme_change"`, `"focus_event"`).
#[inline]
pub fn record_try_lock_failure(site: &str) {
    let total = TRY_LOCK_FAILURE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    logf(
        DebugLevel::Debug,
        "CONCURRENCY",
        format_args!("try_lock() miss at '{}' (lifetime total: {})", site, total),
    );
}

/// Return the current lifetime total of `try_lock()` failures.
///
/// Intended for periodic telemetry reporting (e.g., once per second in
/// `about_to_wait` in `crate::app::handler::window_state_impl`).
#[inline]
pub fn try_lock_failure_count() -> u64 {
    TRY_LOCK_FAILURE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Emit a periodic telemetry summary if new `try_lock()` failures have been recorded
/// since the last call.
///
/// This is designed to be called from `about_to_wait` (once per event-loop iteration)
/// so that lock-contention pressure is surfaced in the debug log without generating
/// per-failure log spam at higher rates.  The log entry is only written when at least
/// one new failure occurred since the previous call.
pub fn maybe_log_try_lock_telemetry() {
    let current = TRY_LOCK_FAILURE_COUNT.load(std::sync::atomic::Ordering::Relaxed);
    let last = TRY_LOCK_LAST_REPORTED.load(std::sync::atomic::Ordering::Relaxed);
    if current > last {
        // Use compare_exchange to ensure only one caller logs when there are
        // multiple windows.  The "loser" simply skips this cycle.
        if TRY_LOCK_LAST_REPORTED
            .compare_exchange(
                last,
                current,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            let new_since_last = current - last;
            logf(
                DebugLevel::Info,
                "CONCURRENCY",
                format_args!(
                    "try_lock telemetry: {} new failure(s) this interval, {} lifetime total",
                    new_since_last, current
                ),
            );
        }
    }
}

// ============================================================================
// Custom debug macros (unchanged, controlled by DEBUG_LEVEL)
// ============================================================================

#[macro_export]
macro_rules! debug_error {
    ($category:expr, $($arg:tt)*) => {
        $crate::debug::logf($crate::debug::DebugLevel::Error, $category, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug_info {
    ($category:expr, $($arg:tt)*) => {
        $crate::debug::logf($crate::debug::DebugLevel::Info, $category, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug_log {
    ($category:expr, $($arg:tt)*) => {
        $crate::debug::logf($crate::debug::DebugLevel::Debug, $category, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug_trace {
    ($category:expr, $($arg:tt)*) => {
        $crate::debug::logf($crate::debug::DebugLevel::Trace, $category, format_args!($($arg)*))
    };
}

// ============================================================================
// Combined debug-log macros (R-18)
//
// Several call sites emit both a `crate::debug_error!` (routed to the custom
// debug log file, controlled by DEBUG_LEVEL) AND a `log::warn!` / `log::error!`
// (routed through the standard `log` crate bridge, controlled by RUST_LOG).
//
// These two-line patterns are collapsed into a single `debug_and_log!` call:
//
//   debug_and_log!(WARN, "CATEGORY", "message {}", var);
//   debug_and_log!(ERROR, "CATEGORY", "message {}", var);
//
// The macro writes to the custom debug log at the `Error` level **and** emits
// the same message through `log::warn!` or `log::error!` respectively.
// ============================================================================

/// Emit a message to both the custom debug log (at `Error` level) and the
/// standard `log` crate at `log::warn!` level.
///
/// # Example
/// ```ignore
/// debug_and_log!(WARN, "TMUX", "Failed to attach session '{}': {}", name, e);
/// ```
#[macro_export]
macro_rules! debug_and_log_warn {
    ($category:expr, $($arg:tt)*) => {{
        $crate::debug::logf(
            $crate::debug::DebugLevel::Error,
            $category,
            format_args!($($arg)*),
        );
        log::warn!($($arg)*);
    }};
}

/// Emit a message to both the custom debug log (at `Error` level) and the
/// standard `log` crate at `log::error!` level.
///
/// # Example
/// ```ignore
/// debug_and_log_error!("SCRIPT", "Script '{}' failed: {}", name, e);
/// ```
#[macro_export]
macro_rules! debug_and_log_error {
    ($category:expr, $($arg:tt)*) => {{
        $crate::debug::logf(
            $crate::debug::DebugLevel::Error,
            $category,
            format_args!($($arg)*),
        );
        log::error!($($arg)*);
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SEC-016: the log must be owner-only from the moment it is created. A chmod
    /// after the open leaves a world-readable window and fails silently when the
    /// path was pre-created by another user.
    #[cfg(unix)]
    #[test]
    fn log_file_is_created_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("par_term_debug.log");

        let file = open_log_file(&path).expect("log file opens");
        drop(file);

        let mode = std::fs::metadata(&path)
            .expect("log file created")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "debug log must not be group/world readable"
        );
    }

    /// SEC-010 regression: a symlink planted at the log path must not be written
    /// through, whoever owns it.
    #[cfg(unix)]
    #[test]
    fn log_file_open_does_not_follow_a_planted_symlink() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("par_term_debug.log");
        let target = dir.path().join("victim");

        std::fs::write(&target, "original").expect("seed symlink target");
        std::os::unix::fs::symlink(&target, &path).expect("plant symlink");

        let mut file = open_log_file(&path).expect("log file opens");
        std::io::Write::write_all(&mut file, b"leaked").expect("write");
        drop(file);

        assert_eq!(
            std::fs::read_to_string(&target).expect("read target"),
            "original",
            "the log must not be written through the planted symlink"
        );
    }

    #[test]
    fn log_file_is_truncated_on_reopen() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("par_term_debug.log");

        std::fs::write(&path, "stale session output").expect("seed log");

        let file = open_log_file(&path).expect("log file opens");
        drop(file);

        assert_eq!(std::fs::read_to_string(&path).expect("read log"), "");
    }

    /// The previous session's log — which after a crash holds the panic report —
    /// must survive the next launch's truncation.
    #[test]
    fn previous_log_is_rolled_aside_on_reopen() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("par_term_debug.log");
        let rolled = dir.path().join("par_term_debug.log.1");

        std::fs::write(&path, "PANIC report from the run that crashed").expect("seed log");

        let file = open_log_file(&path).expect("log file opens");
        drop(file);

        assert_eq!(
            std::fs::read_to_string(&rolled).expect("previous log rolled aside"),
            "PANIC report from the run that crashed"
        );
    }

    /// `make run-debug` pipes through `tee`, which truncates the log before
    /// par-term opens it. Rotating an empty log there would replace a real
    /// previous log with nothing.
    #[test]
    fn an_empty_log_is_not_rotated() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("par_term_debug.log");
        let rolled = dir.path().join("par_term_debug.log.1");

        std::fs::write(&rolled, "the log worth keeping").expect("seed rolled log");
        std::fs::write(&path, "").expect("seed empty log");

        let file = open_log_file(&path).expect("log file opens");
        drop(file);

        assert_eq!(
            std::fs::read_to_string(&rolled).expect("read rolled log"),
            "the log worth keeping",
            "an empty log must not overwrite the previous rotation"
        );
    }

    /// The panic hook's requirement: report a fault without taking the logger
    /// mutex, and without being filtered out at the default `DEBUG_LEVEL=0`.
    #[test]
    fn try_logf_neither_blocks_nor_initializes_the_logger() {
        // Only one direction is assertable, and both of the tempting stronger
        // claims are wrong under `cargo test`'s default parallelism. `LOGGER` is
        // a `OnceLock`, so a concurrent test can install it between the call and
        // a later reading. And an installed logger does *not* imply a write:
        // `try_logf` uses `try_lock` and drops the line when another thread
        // holds the mutex, which is the whole reason it exists for the panic
        // hook. So: a write implies a logger, and nothing else.
        let wrote = try_logf(DebugLevel::Error, "TEST", format_args!("no deadlock"));
        assert!(
            !wrote || LOGGER.get().is_some(),
            "try_logf reported a write with no logger installed"
        );

        assert!(
            !try_logf(DebugLevel::Off, "TEST", format_args!("ignored")),
            "DebugLevel::Off has no line format and must never be written"
        );

        // Held lock: the panic-hook case. Must return false, not hang.
        if let Some(logger) = LOGGER.get() {
            let held = logger.lock();
            assert!(
                !try_logf(DebugLevel::Error, "TEST", format_args!("contended")),
                "try_logf must drop the line while the logger mutex is held"
            );
            drop(held);
        }
    }

    /// The hazard [`try_logf`] exists to remove, exercised through the caller it
    /// was wired into: the panic hook's report step must return while the logger
    /// mutex is held rather than deadlocking on it.
    ///
    /// Driven through `crash_guard::report_fields` — the report path with the
    /// `PanicHookInfo` destructuring lifted off — rather than a real panic. A
    /// real one is not reproducible from a test: the hook is process-global and
    /// latched, it binds to whichever thread installs it first, libtest installs
    /// a hook of its own, and `PanicHookInfo` cannot be constructed.
    ///
    /// The contended half only bites once some earlier test has initialized
    /// `LOGGER`, and nothing here forces that, because initializing it rotates
    /// and truncates the developer's real debug log. The watchdog turns a
    /// reintroduced blocking call — a `log::error!` in that path reaches this
    /// same mutex through [`LogCrateBridge`] — into a failure rather than a run
    /// that hangs forever.
    #[test]
    fn the_panic_report_never_blocks_on_the_logger() {
        use crate::session::crash_guard::{PanicReport, SaveOutcome, report_fields};

        fn fields(outcome: Option<SaveOutcome>) -> PanicReport<'static> {
            PanicReport {
                thread_name: "test",
                file: "src/debug.rs",
                line: 1,
                column: 1,
                payload: "provoked",
                outcome,
            }
        }

        // Uncontended: every outcome the hook can report runs to completion.
        // The hook must stay infallible whichever branch it takes.
        for outcome in [
            None,
            Some(SaveOutcome::Saved),
            Some(SaveOutcome::NoSnapshot),
            Some(SaveOutcome::AlreadySaved),
            Some(SaveOutcome::WriteFailed),
        ] {
            report_fields(fields(outcome));
        }

        let Some(logger) = LOGGER.get() else {
            return;
        };
        let held = logger.lock();

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            report_fields(fields(Some(SaveOutcome::Saved)));
            let _ = tx.send(());
        });
        let finished = rx.recv_timeout(std::time::Duration::from_secs(10)).is_ok();
        drop(held);

        assert!(
            finished,
            "the panic hook's report step blocked on the logger mutex"
        );
    }

    /// Card 01a0c0aef20e7442bf26171b8bc1c7b2: a `--log-level debug` override
    /// raised only `log::set_max_level` while the bridge kept accepting at its
    /// RUST_LOG-derived level, so DEBUG records passed the macro gate and were
    /// then dropped by the bridge's own `enabled()` check. Every level source —
    /// the init-time override and runtime `set_log_level` — must reach
    /// `BRIDGE_MAX_LEVEL`, not just the global gate.
    ///
    /// Exercises the composition `init_log_bridge` uses, minus the two steps a
    /// test must not take: installing the global logger and initializing the
    /// file logger (which truncates the developer's real debug log).
    #[test]
    fn level_overrides_reach_the_bridge_gate() {
        fn meta<'a>(level: log::Level, target: &'a str) -> log::Metadata<'a> {
            log::Metadata::builder().level(level).target(target).build()
        }

        // What init_log_bridge computes for a Debug CLI override: the derived
        // RUST_LOG level replaced by the override.
        let bridge = LogCrateBridge::new();
        store_bridge_max_level(log::LevelFilter::Debug);

        assert_eq!(bridge_max_level(), log::LevelFilter::Debug);
        assert!(
            log::Log::enabled(&bridge, &meta(log::Level::Debug, "par_term::app")),
            "--log-level debug must let log::debug! records through the bridge"
        );

        // The runtime path (settings UI, config application at app start) moves
        // the same gate, in both directions.
        set_log_level(log::LevelFilter::Warn);
        assert!(!log::Log::enabled(
            &bridge,
            &meta(log::Level::Debug, "par_term::app")
        ));
        assert!(!log::Log::enabled(
            &bridge,
            &meta(log::Level::Info, "par_term::app")
        ));
        assert!(log::Log::enabled(
            &bridge,
            &meta(log::Level::Warn, "par_term::app")
        ));

        // Noisy-crate caps are independent of the level and must survive it.
        assert!(
            !log::Log::enabled(&bridge, &meta(log::Level::Info, "wgpu_core::device")),
            "module caps must still clamp wgpu_core below the acceptance level"
        );
    }
}
