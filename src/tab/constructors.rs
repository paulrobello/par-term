//! Tab constructors: `Tab::new`, `Tab::new_from_profile`, and `Tab::new_internal`.
//!
//! Split from `tab/mod.rs` to keep that file under 500 lines. The `Tab` struct
//! definition and `Drop` impl remain in `mod.rs`; all constructor logic lives here.

use super::{Tab, TabInitParams};
use crate::config::Config;
use crate::pane::PaneManager;
use crate::profile::Profile;
use crate::session_logger::{SessionLogger, create_shared_logger};
use crate::tab::activity_state::TabActivityMonitor;
use crate::tab::initial_text::build_initial_text_payload;
use crate::tab::profile_state::TabProfileState;
use crate::tab::scripting_state::TabScriptingState;
use crate::tab::setup::{
    apply_login_shell_flag, build_shell_env, create_base_terminal, get_shell_command,
};
use crate::tab::tmux_state::TabTmuxState;
use crate::terminal::TerminalManager;
use par_term_config::TabId;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

impl Tab {
    /// Shared constructor body called by both `Tab::new()` and `Tab::new_from_profile()`.
    ///
    /// Both public constructors:
    /// 1. Create and configure a `TerminalManager` (divergent: shell command, env, login_shell)
    /// 2. Call this method to handle the identical steps:
    ///    - Coprocess auto-start loop
    ///    - Session logging setup
    ///    - `Arc<RwLock<>>` wrapping
    ///    - Initial text scheduling (only when `params.runtime` is `Some`)
    ///    - `Tab` struct construction with all shared default fields
    ///
    /// # Arguments
    /// * `params` — Constructor-specific values (title, working_directory, etc.)
    /// * `terminal` — Fully configured `TerminalManager` with PTY already spawned
    /// * `config` — Global config (used for coprocesses and session logging)
    /// * `session_title` — Human-readable title written to the session log file header
    pub(super) fn new_internal(
        params: TabInitParams,
        terminal: TerminalManager,
        config: &Config,
        session_title: String,
    ) -> anyhow::Result<Self> {
        // Sync triggers from config into the core TriggerRegistry
        let trigger_security = terminal.sync_triggers(&config.triggers);

        // Auto-start configured coprocesses via the PtySession's built-in manager
        let mut coprocess_ids = Vec::with_capacity(config.coprocesses.len());
        for coproc_config in &config.coprocesses {
            if coproc_config.auto_start {
                let core_config = par_term_emu_core_rust::coprocess::CoprocessConfig {
                    command: coproc_config.command.clone(),
                    args: coproc_config.args.clone(),
                    cwd: None,
                    env: crate::terminal::coprocess_env(),
                    copy_terminal_output: coproc_config.copy_terminal_output,
                    restart_policy: coproc_config.restart_policy.to_core(),
                    restart_delay_ms: coproc_config.restart_delay_ms,
                };
                match terminal.start_coprocess(core_config) {
                    Ok(id) => {
                        log::info!(
                            "Auto-started coprocess '{}' (id={})",
                            coproc_config.name,
                            id
                        );
                        coprocess_ids.push(Some(id));
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to auto-start coprocess '{}': {}",
                            coproc_config.name,
                            e
                        );
                        coprocess_ids.push(None);
                    }
                }
            } else {
                coprocess_ids.push(None);
            }
        }

        // Create shared session logger
        let session_logger = create_shared_logger();

        // Set up session logging if enabled
        if config.auto_log_sessions {
            let logs_dir = config.logs_dir();

            // SEC-010: Ensure the logs directory exists with owner-only permissions
            // (0o700) so session logs are not world-listable.
            if let Err(e) = std::fs::create_dir_all(&logs_dir) {
                log::warn!("Failed to create logs directory {:?}: {}", logs_dir, e);
            } else {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ =
                        std::fs::set_permissions(&logs_dir, std::fs::Permissions::from_mode(0o700));
                }
            }

            let title_with_ts = Some(format!(
                "{} - {}",
                session_title,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            match SessionLogger::new(
                config.session_log_format,
                &logs_dir,
                (config.cols, config.rows),
                title_with_ts,
            ) {
                Ok(mut logger) => {
                    logger.set_redact_passwords(config.session_log_redact_passwords);
                    if let Err(e) = logger.start() {
                        log::warn!("Failed to start session logging: {}", e);
                    } else {
                        log::info!("Session logging started: {:?}", logger.output_path());

                        // Set up output callback to record PTY output
                        let logger_clone = Arc::clone(&session_logger);
                        terminal.set_output_callback(move |data: &[u8]| {
                            if let Some(ref mut logger) = *logger_clone.lock() {
                                logger.record_output(data);
                            }
                        });

                        *session_logger.lock() = Some(logger);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to create session logger: {}", e);
                }
            }
        }

        let terminal = Arc::new(RwLock::new(terminal));

        // Send initial text after optional delay (only when a runtime is provided)
        if let Some(runtime) = params.runtime
            && let Some(payload) =
                build_initial_text_payload(&config.initial_text, config.initial_text_send_newline)
        {
            let delay_ms = config.initial_text_delay_ms;
            let terminal_clone = Arc::clone(&terminal);
            runtime.spawn(async move {
                if delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }

                let term = terminal_clone.write().await;
                if let Err(err) = term.write(&payload) {
                    log::warn!("Failed to send initial text: {}", err);
                }
            });
        }

        // Always create a PaneManager with one primary pane that wraps `tab.terminal`
        // (R-32). This eliminates the fallback scroll_state/mouse/bell/cache fields
        // that were used when pane_manager was None (single-pane mode).
        let is_active = Arc::new(AtomicBool::new(false));
        let pane_manager = PaneManager::new_with_existing_terminal(
            Arc::clone(&terminal),
            params.working_directory.clone(),
            Arc::clone(&is_active),
        );

        Ok(Self {
            id: params.id,
            terminal,
            pane_manager: Some(pane_manager),
            title: params.title,
            refresh_task: None,
            working_directory: params.working_directory,
            custom_color: None,
            has_default_title: params.has_default_title,
            user_named: params.user_named,
            activity: TabActivityMonitor::default(),
            session_logger,
            tmux: TabTmuxState::default(),
            detected_hostname: None,
            detected_cwd: None,
            custom_icon: None,
            profile: TabProfileState::default(),
            scripting: TabScriptingState {
                coprocess_ids,
                trigger_prompt_before_run: trigger_security,
                ..TabScriptingState::default()
            },
            was_alt_screen: false,
            is_active,
            shutdown_fast: false,
            is_hidden: false,
        })
    }

    /// Create a new tab with a terminal session
    ///
    /// # Arguments
    /// * `id` - Unique tab identifier
    /// * `tab_number` - Display number for the tab (1-indexed)
    /// * `config` - Terminal configuration
    /// * `runtime` - Tokio runtime for async operations
    /// * `working_directory` - Optional working directory to start in
    /// * `grid_size` - Optional (cols, rows) override. When provided, uses these
    ///   dimensions instead of config.cols/rows. This ensures the shell starts
    ///   with the correct dimensions when the renderer has already calculated
    ///   the grid size accounting for tab bar height.
    pub fn new(
        id: TabId,
        tab_number: usize,
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
        grid_size: Option<(usize, usize)>,
    ) -> anyhow::Result<Self> {
        // Create and configure terminal
        let (mut terminal, _, _) = create_base_terminal(config, grid_size)?;

        // Determine working directory:
        // 1. If explicitly provided (e.g., from tab_inherit_cwd), use that
        // 2. Otherwise, use the configured startup directory based on mode
        let effective_startup_dir = config.get_effective_startup_directory();
        let work_dir = working_directory
            .as_deref()
            .or(effective_startup_dir.as_deref());

        // Get shell command and apply login shell flag
        let (shell_cmd, mut shell_args) = get_shell_command(config);
        apply_login_shell_flag(&mut shell_args, config);

        let shell_args_deref = shell_args.as_deref();
        let shell_env = build_shell_env(config.shell_env.as_ref());
        terminal.spawn_custom_shell_with_dir(
            &shell_cmd,
            shell_args_deref,
            work_dir,
            shell_env.as_ref(),
        )?;

        // Generate initial title based on current tab count, not unique ID
        let title = format!("Tab {}", tab_number);

        Self::new_internal(
            TabInitParams {
                id,
                title,
                has_default_title: true,
                user_named: false,
                working_directory: working_directory.or_else(|| config.working_directory.clone()),
                runtime: Some(runtime),
            },
            terminal,
            config,
            format!("Tab {}", tab_number),
        )
    }

    /// Create a new tab from a profile configuration
    ///
    /// The profile can override:
    /// - Working directory
    /// - Command and arguments (instead of default shell)
    /// - Tab name
    ///
    /// If a profile specifies a command, it always runs from the profile's working
    /// directory (or config default if unset).
    ///
    /// # Arguments
    /// * `id` - Unique tab identifier
    /// * `config` - Terminal configuration
    /// * `_runtime` - Tokio runtime (unused: profile tabs don't send initial text)
    /// * `profile` - Profile configuration to use
    /// * `grid_size` - Optional (cols, rows) override for initial terminal size
    ///
    /// # Unique logic vs `Tab::new()`
    /// This constructor's divergent logic (not shared with `Tab::new()` via `new_internal`):
    /// - SSH command detection (`profile.ssh_command_args()`)
    /// - Per-profile `login_shell` override (takes precedence over `config.login_shell`)
    /// - Per-profile `SHELL` env-var injection when `profile.shell` is set
    /// - Title derived from `profile.tab_name` → `profile.name` (not "Tab N")
    /// - Profile tabs do NOT send `config.initial_text` on startup
    pub fn new_from_profile(
        id: TabId,
        config: &Config,
        _runtime: Arc<Runtime>,
        profile: &Profile,
        grid_size: Option<(usize, usize)>,
    ) -> anyhow::Result<Self> {
        // Create and configure terminal
        let (mut terminal, _, _) = create_base_terminal(config, grid_size)?;

        // Determine working directory: profile overrides config startup directory
        let effective_startup_dir = config.get_effective_startup_directory();
        let work_dir = profile
            .working_directory
            .as_deref()
            .or(effective_startup_dir.as_deref());

        // Determine command and args with priority:
        // 0. profile.ssh_host → build ssh command with user/port/identity args
        // 1. profile.command → use as-is (non-shell commands like tmux, ssh)
        // 2. profile.shell → use as shell, apply login_shell logic
        // 3. neither → fall back to global config shell / $SHELL
        let is_ssh_profile = profile.ssh_host.is_some();
        let (shell_cmd, mut shell_args) = if let Some(ssh_args) = profile.ssh_command_args() {
            ("ssh".to_string(), Some(ssh_args))
        } else if let Some(ref cmd) = profile.command {
            (cmd.clone(), profile.command_args.clone())
        } else if let Some(ref shell) = profile.shell {
            (shell.clone(), None)
        } else {
            get_shell_command(config)
        };

        // Apply login shell flag when using a shell (not a custom command or SSH profile).
        // Per-profile login_shell overrides global config.login_shell.
        if profile.command.is_none() && !is_ssh_profile {
            let use_login_shell = profile.login_shell.unwrap_or(config.login_shell);
            if use_login_shell {
                let args = shell_args.get_or_insert_with(Vec::new);
                #[cfg(not(target_os = "windows"))]
                if !args.iter().any(|a| a == "-l" || a == "--login") {
                    args.insert(0, "-l".to_string());
                }
            }
        }

        let shell_args_deref = shell_args.as_deref();
        let mut shell_env = build_shell_env(config.shell_env.as_ref());

        // When a profile specifies a shell, set the SHELL env var so child
        // processes (and $SHELL) reflect the selected shell, not the login shell.
        if profile.command.is_none()
            && let Some(ref shell_path) = profile.shell
            && let Some(ref mut env) = shell_env
        {
            env.insert("SHELL".to_string(), shell_path.clone());
        }

        terminal.spawn_custom_shell_with_dir(
            &shell_cmd,
            shell_args_deref,
            work_dir,
            shell_env.as_ref(),
        )?;

        // Generate title: use profile tab_name or profile name
        let title = profile
            .tab_name
            .clone()
            .unwrap_or_else(|| profile.name.clone());

        let working_directory = profile
            .working_directory
            .clone()
            .or_else(|| config.working_directory.clone());

        // Session log title uses profile name (Tab::new uses "Tab N")
        let session_title = profile.name.clone();

        Self::new_internal(
            TabInitParams {
                id,
                title,
                has_default_title: false, // Profile-created tabs have explicit names
                user_named: profile.tab_name.is_some(),
                working_directory,
                runtime: None, // Profile tabs don't send initial_text
            },
            terminal,
            config,
            session_title,
        )
    }
}

/// Minimal stub for use in unit tests (no PTY, no runtime).
#[cfg(test)]
impl Tab {
    pub(crate) fn new_stub(id: TabId, tab_number: usize) -> Self {
        use crate::session_logger::create_shared_logger;

        // Create a dummy TerminalManager without spawning a shell
        let terminal =
            TerminalManager::new_with_scrollback(80, 24, 100).expect("stub terminal creation");
        let terminal = Arc::new(RwLock::new(terminal));
        let is_active = Arc::new(AtomicBool::new(false));
        let pane_manager = PaneManager::new_with_existing_terminal(
            Arc::clone(&terminal),
            None,
            Arc::clone(&is_active),
        );
        Self {
            id,
            terminal,
            pane_manager: Some(pane_manager),
            title: format!("Tab {}", tab_number),
            refresh_task: None,
            working_directory: None,
            custom_color: None,
            has_default_title: true,
            user_named: false,
            activity: TabActivityMonitor::default(),
            session_logger: create_shared_logger(),
            tmux: TabTmuxState::default(),
            detected_hostname: None,
            detected_cwd: None,
            custom_icon: None,
            profile: TabProfileState::default(),
            scripting: TabScriptingState::default(),
            was_alt_screen: false,
            is_active,
            shutdown_fast: false,
            is_hidden: false,
        }
    }
}
