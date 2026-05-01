//! ShellCommand action handler: fire-and-forget and capture_output paths.

use crate::app::window_state::WindowState;
use crate::app::window_state::WorkflowContext;
use std::sync::Arc;

impl WindowState {
    /// Execute a ShellCommand custom action.
    ///
    /// Spawns a background thread to avoid blocking the main event loop.
    /// When `capture_output` is true, stdout/stderr are captured and stored
    /// in `self.last_workflow_context`.
    pub(crate) fn execute_shell_command_action(
        &mut self,
        command: String,
        args: Vec<String>,
        notify_on_success: bool,
        timeout_secs: u64,
        title: String,
        capture_output: bool,
    ) -> bool {
        let ctx_arc = Arc::clone(&self.last_workflow_context);

        log::info!(
            "Executing shell command '{}' (timeout={}s): {} {}",
            title,
            timeout_secs,
            command,
            args.join(" ")
        );

        // DEFERRED: Command allowlist for restricted deployments.
        //
        // In enterprise/kiosk environments it may be desirable to restrict
        // which executables can be invoked through custom shell-command
        // actions, in addition to the per-action configuration already
        // enforced by the keybinding system. A suggested approach:
        //
        //   1. Add an `allowed_commands: Option<Vec<String>>` field to
        //      `Config` (absent means "allow all").
        //   2. Before spawning, resolve `command` to its canonical path
        //      with `std::fs::canonicalize` and check it against the list.
        //   3. Return `false` (with a toast notification) if the canonical
        //      path is not on the allowlist.
        //
        // Intentionally deferred: requires a policy decision about how the
        // allowlist is managed (config file, environment variable, MDM
        // profile, etc.) that is out of scope for the current sprint.
        // Track as a GitHub issue with the "enterprise" and "security" labels
        // before implementing. Relates to SEC-002 (bypassable command denylist)
        // and ARC-011 (optional feature flags for enterprise deployment).

        // Spawn a background thread to avoid blocking the main event loop
        std::thread::spawn(move || {
            let timeout = std::time::Duration::from_secs(timeout_secs);
            let start = std::time::Instant::now();

            if capture_output {
                // Collect stdout+stderr, cap at 64KB
                let output_result = std::process::Command::new(&command).args(&args).output();

                match output_result {
                    Ok(output) => {
                        let exit_code = output.status.code().unwrap_or(-1);
                        let mut combined = String::new();
                        combined.push_str(&String::from_utf8_lossy(&output.stdout));
                        combined.push_str(&String::from_utf8_lossy(&output.stderr));
                        if combined.len() > 65536 {
                            combined.truncate(65536);
                        }
                        let ctx = WorkflowContext {
                            last_exit_code: Some(exit_code),
                            last_output: if combined.is_empty() {
                                None
                            } else {
                                Some(combined)
                            },
                        };
                        if let Ok(mut guard) = ctx_arc.lock() {
                            *guard = Some(ctx);
                        }
                        if output.status.success() {
                            log::info!("Shell command '{}' completed successfully", title);
                            if notify_on_success {
                                log::info!("Command '{}' output available", title);
                            }
                        } else {
                            log::error!(
                                "Shell command '{}' failed with exit code {}",
                                title,
                                exit_code
                            );
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to spawn shell command '{}': {}", title, e);
                    }
                }
            } else {
                // Use spawn to run the command and wait with timeout
                let child_result = std::process::Command::new(&command).args(&args).spawn();

                match child_result {
                    Ok(mut child) => {
                        // Poll for completion with timeout
                        loop {
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    let elapsed = start.elapsed();
                                    if status.success() {
                                        log::info!(
                                            "Shell command '{}' completed successfully in {:.2}s",
                                            title,
                                            elapsed.as_secs_f64()
                                        );
                                        if notify_on_success {
                                            log::info!(
                                                "Command '{}' output available (check terminal or logs)",
                                                title
                                            );
                                        }
                                    } else {
                                        log::error!(
                                            "Shell command '{}' failed with status: {} after {:.2}s",
                                            title,
                                            status,
                                            elapsed.as_secs_f64()
                                        );
                                    }
                                    break;
                                }
                                Ok(None) => {
                                    // Still running, check timeout
                                    if start.elapsed() > timeout {
                                        log::error!(
                                            "Shell command '{}' timed out after {}s, terminating",
                                            title,
                                            timeout_secs
                                        );
                                        let _ = child.kill();
                                        let _ = child.wait();
                                        break;
                                    }
                                    // Small sleep to avoid busy-waiting
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                }
                                Err(e) => {
                                    log::error!(
                                        "Shell command '{}' error checking status: {}",
                                        title,
                                        e
                                    );
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to spawn shell command '{}': {}", title, e);
                    }
                }
            }
        });

        // Return immediately - command is running in background
        true
    }
}
