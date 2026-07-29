//! Integration tests for the scripting system.
//!
//! Covers: script configuration, integration with manager/process,
//! command dispatch, observer bridge, and protocol serialization.

/// The Python interpreter to drive from tests.
///
/// Resolved rather than hardcoded to `python3`: the official Windows
/// distribution installs `python.exe` and `py.exe` but no `python3`, so a
/// literal `"python3"` fails there even when Python is correctly installed.
/// Uses the same resolution as production code.
pub fn python_cmd() -> &'static str {
    par_term::scripting::manager::python_interpreter()
        .expect("these tests require a Python interpreter (python3/python/py) on PATH")
}

/// How long to wait for a spawned script to produce the output a test expects.
///
/// Generous on purpose. These tests used to sleep a fixed 500 ms, which is
/// shorter than a cold Python start on a Windows CI runner — interpreter
/// startup plus Defender scanning the interpreter and the stdlib it imports.
/// That made the whole scripting suite pass or fail depending on whether
/// something else had already warmed Python, which is what made Windows CI
/// look flaky. Waiting for the output itself is both faster in the common case
/// and immune to how slow the runner is.
pub const SCRIPT_OUTPUT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Interval between polls while waiting for script output.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(25);

/// Drain `read` repeatedly until `enough` accepts what has accumulated, or
/// [`SCRIPT_OUTPUT_TIMEOUT`] expires.
///
/// Accumulates across polls because the `read_commands`/`read_errors` pair
/// drains its buffer — a script's output can arrive split over several reads,
/// and a single late read would lose the earlier half.
///
/// Returns whatever was collected; on timeout that is the partial output, so
/// the caller's assertion reports the real shortfall rather than a timeout.
pub fn drain_until<T>(mut read: impl FnMut() -> Vec<T>, enough: impl Fn(&[T]) -> bool) -> Vec<T> {
    let deadline = std::time::Instant::now() + SCRIPT_OUTPUT_TIMEOUT;
    let mut collected = Vec::new();
    loop {
        collected.extend(read());
        if enough(&collected) || std::time::Instant::now() >= deadline {
            return collected;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Wait for `cond` to hold, up to [`SCRIPT_OUTPUT_TIMEOUT`].
///
/// Returns whether it held, so callers assert on the result.
pub fn wait_until(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = std::time::Instant::now() + SCRIPT_OUTPUT_TIMEOUT;
    loop {
        if cond() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

mod script_auto_start_tests;
mod script_command_dispatch_tests;
mod script_integration_tests;
mod script_manager_tests;
mod script_observer_tests;
mod script_process_tests;
mod script_protocol_tests;
mod scripting_config_tests;
