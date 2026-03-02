//! URL and file path opening/action utilities.
//!
//! # Error Handling Convention
//!
//! Public functions in this module return `Result<(), String>` (simple string
//! errors for UI display) rather than `anyhow::Error`. New helper functions
//! added to this module should follow the same `Result<T, String>` pattern so
//! callers can surface the error message directly to the user without conversion.

/// Ensure a URL has a scheme prefix, adding `https://` if missing.
///
/// # Examples
/// - `"www.example.com"` -> `"https://www.example.com"`
/// - `"https://example.com"` -> `"https://example.com"` (unchanged)
pub fn ensure_url_scheme(url: &str) -> String {
    if !url.contains("://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    }
}

/// Expand a link handler command template by replacing `{url}` with the given URL.
///
/// Returns the command split into program + arguments, ready for spawning.
/// The command template is parsed using shell-word splitting BEFORE URL substitution
/// so that the URL remains a single argument regardless of its content (preventing
/// argument injection via crafted URLs containing spaces or shell metacharacters).
///
/// Returns an error if the expanded command is empty (whitespace-only or blank).
pub fn expand_link_handler(command: &str, url: &str) -> Result<Vec<String>, String> {
    // Parse the command template into tokens FIRST, before substitution.
    // This ensures that {url} occupies exactly one token position,
    // and the substituted URL cannot inject additional arguments.
    let tokens = shell_words::split(command)
        .map_err(|e| format!("Failed to parse link handler command: {}", e))?;
    if tokens.is_empty() {
        return Err("Link handler command is empty after expansion".to_string());
    }
    // Replace {url} placeholder within each token (the URL stays as one argument)
    let parts: Vec<String> = tokens
        .into_iter()
        .map(|token| token.replace("{url}", url))
        .collect();
    Ok(parts)
}

/// Open a URL in the configured browser or system default
pub fn open_url(url: &str, link_handler_command: &str) -> Result<(), String> {
    let url_with_scheme = ensure_url_scheme(url);

    if link_handler_command.is_empty() {
        // Use system default
        open::that(&url_with_scheme).map_err(|e| format!("Failed to open URL: {}", e))
    } else {
        // Use custom command with {url} placeholder
        let parts = expand_link_handler(link_handler_command, &url_with_scheme)?;
        std::process::Command::new(&parts[0])
            .args(&parts[1..])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to run link handler '{}': {}", parts[0], e))
    }
}

/// Open a file path in the configured editor, or a directory in the file manager
///
/// # Arguments
/// * `path` - The file or directory path to open
/// * `line` - Optional line number to jump to (ignored for directories)
/// * `column` - Optional column number to jump to (ignored for directories)
/// * `editor_mode` - How to select the editor (Custom, EnvironmentVariable, or SystemDefault)
/// * `editor_cmd` - Editor command template with placeholders: `{file}`, `{line}`, `{col}`.
///   Only used when mode is `Custom`.
/// * `cwd` - Optional working directory for resolving relative paths
///
/// # Security Note
///
/// The `path` argument originates from terminal output (e.g. a URL or filename detected
/// in the scrollback buffer). It is **user-supplied and not sanitized beyond shell escaping**.
/// The function applies [`shell_escape`] to all substituted values before constructing the
/// shell command, which prevents typical shell metacharacter injection (backticks, `$()`,
/// semicolons, etc.) via a maliciously crafted filename.
///
/// **Trust assumption**: this function trusts that the path was identified by the URL/semantic
/// detector from the user's own terminal session. It does not validate that the path points to
/// a benign file — opening a path in an editor is the intended action. If this assumption
/// changes (e.g. paths arrive from an untrusted external source), additional validation should
/// be applied before calling this function.
pub fn open_file_in_editor(
    path: &str,
    line: Option<usize>,
    column: Option<usize>,
    editor_mode: crate::config::SemanticHistoryEditorMode,
    editor_cmd: &str,
    cwd: Option<&str>,
) -> Result<(), String> {
    // Expand ~ to home directory
    let resolved_path = if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            path.replacen("~", &home.to_string_lossy(), 1)
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };

    // Resolve relative paths using CWD
    let resolved_path = if resolved_path.starts_with("./") || resolved_path.starts_with("../") {
        if let Some(working_dir) = cwd {
            // Expand ~ in CWD as well
            let expanded_cwd = if working_dir.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    working_dir.replacen("~", &home.to_string_lossy(), 1)
                } else {
                    working_dir.to_string()
                }
            } else {
                working_dir.to_string()
            };

            let cwd_path = std::path::Path::new(&expanded_cwd);
            let full_path = cwd_path.join(&resolved_path);
            crate::debug_info!(
                "SEMANTIC",
                "Resolved relative path: {:?} + {:?} = {:?}",
                expanded_cwd,
                resolved_path,
                full_path
            );
            // Canonicalize to resolve . and .. components
            full_path
                .canonicalize()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| full_path.to_string_lossy().to_string())
        } else {
            resolved_path.clone()
        }
    } else {
        resolved_path.clone()
    };

    // Verify the path exists
    let path_obj = std::path::Path::new(&resolved_path);
    if !path_obj.exists() {
        return Err(format!("Path not found: {}", resolved_path));
    }

    // If it's a directory, always open in the system file manager
    if path_obj.is_dir() {
        crate::debug_info!(
            "SEMANTIC",
            "Opening directory in file manager: {}",
            resolved_path
        );
        return open::that(&resolved_path).map_err(|e| format!("Failed to open directory: {}", e));
    }

    // Determine the editor command based on mode
    use crate::config::SemanticHistoryEditorMode;
    let cmd = match editor_mode {
        SemanticHistoryEditorMode::Custom => {
            if editor_cmd.is_empty() {
                // Custom mode but no command configured - fall back to system default
                crate::debug_info!(
                    "SEMANTIC",
                    "Custom mode but no editor configured, using system default for: {}",
                    resolved_path
                );
                return open::that(&resolved_path)
                    .map_err(|e| format!("Failed to open file: {}", e));
            }
            crate::debug_info!("SEMANTIC", "Using custom editor: {:?}", editor_cmd);
            editor_cmd.to_string()
        }
        SemanticHistoryEditorMode::EnvironmentVariable => {
            // Try $EDITOR, then $VISUAL, then fall back to system default
            let env_editor = std::env::var("EDITOR")
                .or_else(|_| std::env::var("VISUAL"))
                .ok();
            crate::debug_info!(
                "SEMANTIC",
                "Environment variable mode: EDITOR={:?}, VISUAL={:?}",
                std::env::var("EDITOR").ok(),
                std::env::var("VISUAL").ok()
            );
            if let Some(editor) = env_editor {
                editor
            } else {
                crate::debug_info!(
                    "SEMANTIC",
                    "No $EDITOR/$VISUAL set, using system default for: {}",
                    resolved_path
                );
                return open::that(&resolved_path)
                    .map_err(|e| format!("Failed to open file: {}", e));
            }
        }
        SemanticHistoryEditorMode::SystemDefault => {
            crate::debug_info!(
                "SEMANTIC",
                "System default mode, opening with default app: {}",
                resolved_path
            );
            return open::that(&resolved_path).map_err(|e| format!("Failed to open file: {}", e));
        }
    };

    // Replace placeholders in command template.
    // Shell-escape ALL substituted values to prevent command injection via
    // malicious filenames containing shell metacharacters (backticks, $(), etc.).
    let escaped_path = shell_escape(&resolved_path);
    let line_str = line
        .map(|l| l.to_string())
        .unwrap_or_else(|| "1".to_string());
    let col_str = column
        .map(|c| c.to_string())
        .unwrap_or_else(|| "1".to_string());
    // Line/col are numeric strings so shell_escape is defensive, but apply it
    // consistently to guard against any future changes to how these are sourced.
    let escaped_line = shell_escape(&line_str);
    let escaped_col = shell_escape(&col_str);

    let full_cmd = cmd
        .replace("{file}", &escaped_path)
        .replace("{line}", &escaped_line)
        .replace("{col}", &escaped_col);

    // If the template didn't have placeholders, append the file path
    let full_cmd = if !cmd.contains("{file}") {
        format!("{} {}", full_cmd, escaped_path)
    } else {
        full_cmd
    };

    // Execute the command
    crate::debug_info!(
        "SEMANTIC",
        "Executing editor command: {:?} for file: {} (line: {:?}, col: {:?})",
        full_cmd,
        resolved_path,
        line,
        column
    );

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", &full_cmd])
            .spawn()
            .map_err(|e| format!("Failed to launch editor: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Use login shell to ensure user's PATH is available
        // Try user's default shell first, fall back to sh
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        std::process::Command::new(&shell)
            .args(["-lc", &full_cmd])
            .spawn()
            .map_err(|e| format!("Failed to launch editor with {}: {}", shell, e))?;
    }

    Ok(())
}

/// Simple shell escape for file paths (wraps in single quotes)
pub fn shell_escape(s: &str) -> String {
    // Replace single quotes with escaped version and wrap in single quotes
    format!("'{}'", s.replace('\'', "'\\''"))
}
