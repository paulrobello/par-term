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

mod script_auto_start_tests;
mod script_command_dispatch_tests;
mod script_integration_tests;
mod script_manager_tests;
mod script_observer_tests;
mod script_process_tests;
mod script_protocol_tests;
mod scripting_config_tests;
