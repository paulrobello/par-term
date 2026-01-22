use parking_lot::Mutex;
/// Comprehensive debugging infrastructure for par-term
///
/// Controlled by DEBUG_LEVEL environment variable:
/// - 0 or unset: No debugging
/// - 1: Errors only
/// - 2: Info level (app events, graphics operations)
/// - 3: Debug level (rendering, calculations)
/// - 4: Trace level (every operation, detailed info)
///
/// All output goes to /tmp/par_term_debug.log on Unix/macOS,
/// or %TEMP%\par_term_debug.log on Windows.
/// This avoids breaking TUI apps by keeping debug output separate from stdout/stderr.
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Debug level configuration
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

/// Global debug logger
struct DebugLogger {
    level: DebugLevel,
    file: Option<std::fs::File>,
}

impl DebugLogger {
    fn new() -> Self {
        let level = DebugLevel::from_env();

        let file = if level != DebugLevel::Off {
            #[cfg(unix)]
            let log_path = std::path::PathBuf::from("/tmp/par_term_debug.log");
            #[cfg(windows)]
            let log_path = std::env::temp_dir().join("par_term_debug.log");

            match OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(&log_path)
            {
                Ok(f) => {
                    // Write header
                    let mut logger = DebugLogger {
                        level,
                        file: Some(f),
                    };
                    logger.write_raw(&format!(
                        "\n{}\npar-term debug session started at {} (level={:?})\n{}\n",
                        "=".repeat(80),
                        get_timestamp(),
                        level,
                        "=".repeat(80)
                    ));
                    return logger;
                }
                Err(_e) => {
                    // Silently fail if log file can't be opened
                    // This prevents debug output from interfering with TUI applications
                    None
                }
            }
        } else {
            None
        };

        DebugLogger { level, file }
    }

    fn write_raw(&mut self, msg: &str) {
        if let Some(ref mut file) = self.file {
            let _ = file.write_all(msg.as_bytes());
            let _ = file.flush();
        }
    }

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
}

static LOGGER: OnceLock<Mutex<DebugLogger>> = OnceLock::new();

fn get_logger() -> &'static Mutex<DebugLogger> {
    LOGGER.get_or_init(|| Mutex::new(DebugLogger::new()))
}

fn get_timestamp() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{}.{:06}", now.as_secs(), now.subsec_micros())
}

/// Check if debugging is enabled at given level
pub fn is_enabled(level: DebugLevel) -> bool {
    let logger = get_logger().lock();
    level <= logger.level
}

/// Log a message at specified level
pub fn log(level: DebugLevel, category: &str, msg: &str) {
    let mut logger = get_logger().lock();
    logger.log(level, category, msg);
}

/// Log formatted message
pub fn logf(level: DebugLevel, category: &str, args: fmt::Arguments) {
    if is_enabled(level) {
        log(level, category, &format!("{}", args));
    }
}

// Convenience macros for logging
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
