//! Shell and terminal setup utilities for tab creation.
//!
//! This module contains platform-specific environment building, shell command
//! detection, and terminal configuration helpers used when spawning new tabs.

use crate::config::Config;
use crate::terminal::TerminalManager;

/// Configure a terminal with settings from config (theme, clipboard limits, cursor style, unicode)
pub(crate) fn configure_terminal_from_config(terminal: &mut TerminalManager, config: &Config) {
    // Set theme from config
    terminal.set_theme(config.load_theme());

    // Apply clipboard history limits from config
    terminal.set_max_clipboard_sync_events(config.clipboard_max_sync_events);
    terminal.set_max_clipboard_event_bytes(config.clipboard_max_event_bytes);

    // Set answerback string for ENQ response (if configured)
    if !config.answerback_string.is_empty() {
        terminal.set_answerback_string(Some(config.answerback_string.clone()));
    }

    // Apply Unicode width configuration
    let width_config =
        par_term_emu_core_rust::WidthConfig::new(config.unicode_version, config.ambiguous_width);
    terminal.set_width_config(width_config);

    // Apply Unicode normalization form
    terminal.set_normalization_form(config.normalization_form);

    // Initialize cursor style from config
    use crate::config::CursorStyle as ConfigCursorStyle;
    use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
    let term_style = if config.cursor_blink {
        match config.cursor_style {
            ConfigCursorStyle::Block => TermCursorStyle::BlinkingBlock,
            ConfigCursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
            ConfigCursorStyle::Beam => TermCursorStyle::BlinkingBar,
        }
    } else {
        match config.cursor_style {
            ConfigCursorStyle::Block => TermCursorStyle::SteadyBlock,
            ConfigCursorStyle::Underline => TermCursorStyle::SteadyUnderline,
            ConfigCursorStyle::Beam => TermCursorStyle::SteadyBar,
        }
    };
    terminal.set_cursor_style(term_style);
}

/// Get the platform-specific PATH separator
#[cfg(target_os = "windows")]
const PATH_SEPARATOR: char = ';';
#[cfg(not(target_os = "windows"))]
const PATH_SEPARATOR: char = ':';

/// Build environment variables with an augmented PATH
///
/// When launched from Finder on macOS (or similar on other platforms), the PATH may be minimal.
/// This function augments the PATH with common directories where user tools are installed.
pub(crate) fn build_shell_env(
    config_env: Option<&std::collections::HashMap<String, String>>,
) -> Option<std::collections::HashMap<String, String>> {
    // Advertise as iTerm.app for maximum compatibility with tools that check
    // TERM_PROGRAM for feature detection (progress bars, hyperlinks, clipboard, etc.)
    // par-term supports all the relevant iTerm2 protocols (OSC 8, 9;4, 52, 1337).
    let mut env = std::collections::HashMap::new();
    env.insert("TERM_PROGRAM".to_string(), "iTerm.app".to_string());
    env.insert("TERM_PROGRAM_VERSION".to_string(), "3.6.6".to_string());
    env.insert("LC_TERMINAL".to_string(), "iTerm2".to_string());
    env.insert("LC_TERMINAL_VERSION".to_string(), "3.6.6".to_string());
    // par-term identity marker for shell integration scripts to detect
    env.insert("__PAR_TERM".to_string(), "1".to_string());

    // ITERM_SESSION_ID: used by Claude Code and other tools for OSC 52 clipboard detection
    // Format: w{window}t{tab}p{pane}:{UUID}
    let session_uuid = uuid::Uuid::new_v4();
    env.insert(
        "ITERM_SESSION_ID".to_string(),
        format!("w0t0p0:{session_uuid}"),
    );

    // Merge user-configured shell_env (user values take precedence)
    if let Some(config) = config_env {
        for (key, value) in config {
            env.insert(key.clone(), value.clone());
        }
    }

    // Build augmented PATH with platform-specific extra directories
    let current_path = std::env::var("PATH").unwrap_or_default();
    let extra_paths = build_platform_extra_paths();
    let new_paths: Vec<String> = extra_paths
        .into_iter()
        .filter(|p| !p.is_empty() && !current_path.contains(p) && std::path::Path::new(p).exists())
        .collect();

    let augmented_path = if new_paths.is_empty() {
        current_path
    } else {
        format!(
            "{}{}{}",
            new_paths.join(&PATH_SEPARATOR.to_string()),
            PATH_SEPARATOR,
            current_path
        )
    };
    env.insert("PATH".to_string(), augmented_path);

    Some(env)
}

/// Build the list of extra PATH directories for the current platform
#[cfg(target_os = "windows")]
fn build_platform_extra_paths() -> Vec<String> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Cargo bin
        paths.push(
            home.join(".cargo")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
        // Scoop
        paths.push(
            home.join("scoop")
                .join("shims")
                .to_string_lossy()
                .to_string(),
        );
        // Go bin
        paths.push(home.join("go").join("bin").to_string_lossy().to_string());
    }

    // Chocolatey
    paths.push(r"C:\ProgramData\chocolatey\bin".to_string());

    // Common program locations
    if let Some(local_app_data) = dirs::data_local_dir() {
        // Python (common location)
        paths.push(
            local_app_data
                .join("Programs")
                .join("Python")
                .join("Python312")
                .join("Scripts")
                .to_string_lossy()
                .to_string(),
        );
        paths.push(
            local_app_data
                .join("Programs")
                .join("Python")
                .join("Python311")
                .join("Scripts")
                .to_string_lossy()
                .to_string(),
        );
    }

    paths
}

/// Build the list of extra PATH directories for Unix platforms (macOS/Linux)
#[cfg(not(target_os = "windows"))]
fn build_platform_extra_paths() -> Vec<String> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // User's home .local/bin (common for pip, pipx, etc.)
        paths.push(
            home.join(".local")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
        // Cargo bin
        paths.push(
            home.join(".cargo")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
        // Go bin
        paths.push(home.join("go").join("bin").to_string_lossy().to_string());
        // Nix user profile
        paths.push(
            home.join(".nix-profile")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
    }

    // Nix system profile
    paths.push("/nix/var/nix/profiles/default/bin".to_string());

    // macOS-specific paths
    #[cfg(target_os = "macos")]
    {
        // Homebrew on Apple Silicon
        paths.push("/opt/homebrew/bin".to_string());
        paths.push("/opt/homebrew/sbin".to_string());
        // Homebrew on Intel Mac
        paths.push("/usr/local/bin".to_string());
        paths.push("/usr/local/sbin".to_string());
        // MacPorts
        paths.push("/opt/local/bin".to_string());
    }

    // Linux-specific paths
    #[cfg(target_os = "linux")]
    {
        // Common system paths that might be missing
        paths.push("/usr/local/bin".to_string());
        // Snap
        paths.push("/snap/bin".to_string());
        // Flatpak exports
        if let Some(home) = dirs::home_dir() {
            paths.push(
                home.join(".local")
                    .join("share")
                    .join("flatpak")
                    .join("exports")
                    .join("bin")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        paths.push("/var/lib/flatpak/exports/bin".to_string());
    }

    paths
}

/// Determine the shell command and arguments to use based on config
pub(crate) fn get_shell_command(config: &Config) -> (String, Option<Vec<String>>) {
    if let Some(ref custom) = config.custom_shell {
        (custom.clone(), config.shell_args.clone())
    } else {
        #[cfg(target_os = "windows")]
        {
            ("powershell.exe".to_string(), None)
        }
        #[cfg(not(target_os = "windows"))]
        {
            (
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
                None,
            )
        }
    }
}

/// Apply login shell flag if configured (Unix only)
#[cfg(not(target_os = "windows"))]
pub(crate) fn apply_login_shell_flag(shell_args: &mut Option<Vec<String>>, config: &Config) {
    if config.login_shell {
        let args = shell_args.get_or_insert_with(Vec::new);
        if !args.iter().any(|a| a == "-l" || a == "--login") {
            args.insert(0, "-l".to_string());
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn apply_login_shell_flag(_shell_args: &mut Option<Vec<String>>, _config: &Config) {
    // No-op on Windows
}

/// Create and configure a new TerminalManager based on grid size and config.
/// Returns (terminal, cols, rows).
pub(crate) fn create_base_terminal(
    config: &Config,
    grid_size: Option<(usize, usize)>,
) -> anyhow::Result<(TerminalManager, usize, usize)> {
    // Use provided grid size if available, otherwise fall back to config
    let (cols, rows) = grid_size.unwrap_or((config.cols, config.rows));

    // Create terminal with scrollback from config
    let mut terminal = TerminalManager::new_with_scrollback(cols, rows, config.scrollback_lines)?;

    // Apply common terminal configuration
    configure_terminal_from_config(&mut terminal, config);

    Ok((terminal, cols, rows))
}
