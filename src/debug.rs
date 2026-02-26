/// Comprehensive debugging infrastructure for par-term
///
/// Two logging systems are unified into a single log file:
///
/// 1. **Custom debug macros** (`crate::debug_info!()`, etc.)
///    - Controlled by `DEBUG_LEVEL` environment variable (0-4)
///    - Best for high-frequency rendering/input logging with category tags
///
/// 2. **Standard `log` crate** (`log::info!()`, etc.)
///    - Controlled by `RUST_LOG` environment variable
///    - Used by most application code and third-party crates
///
/// Both write to `<temp_dir>/par_term_debug.log` (respects `$TMPDIR` on Unix, `%TEMP%` on Windows).
/// The log file is always created so that errors are captured even in GUI-only contexts
/// (macOS app bundles, Windows GUI apps) where stderr is invisible.
/// The log file is created with 0600 permissions on Unix and symlink-checked to prevent attacks.
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
}

impl DebugLogger {
    fn new() -> Self {
        let level = DebugLevel::from_env();

        let log_path = std::env::temp_dir().join("par_term_debug.log");

        // Security: refuse to open symlinks (prevents symlink attacks)
        if log_path
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_file(&log_path);
        }

        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&log_path)
            .ok();

        // Security: restrict log file to owner-only access on Unix
        #[cfg(unix)]
        if file.is_some() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&log_path, std::fs::Permissions::from_mode(0o600));
        }

        let mut logger = DebugLogger { level, file };
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
    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("SystemTime::now() is always after UNIX_EPOCH");
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

// ============================================================================
// log crate bridge — routes log::info!() etc. to the debug log file
// ============================================================================

/// Bridge that implements the `log` crate's `Log` trait, routing all log
/// output to the par-term debug log file. Optionally mirrors to stderr
/// when `RUST_LOG` is set (for terminal debugging).
struct LogCrateBridge {
    /// Maximum level to accept (parsed from RUST_LOG, default: Info)
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
        self.max_level
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
    // CLI/config override takes precedence, then RUST_LOG, then default
    let max_level = level_override.unwrap_or(bridge.max_level);

    // Install as the global logger
    if log::set_boxed_logger(Box::new(bridge)).is_ok() {
        log::set_max_level(max_level);
    }
}

/// Update the log level at runtime (e.g., from settings UI).
/// This only changes `log::max_level()` — the bridge itself always writes
/// whatever passes the filter.
pub fn set_log_level(level: log::LevelFilter) {
    log::set_max_level(level);
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
