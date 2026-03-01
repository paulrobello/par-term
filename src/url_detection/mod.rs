/// URL and file path detection and handling utilities
//
// # Error Handling Convention
//
// Public functions in this module return `Result<(), String>` (simple string
// errors for UI display) rather than `anyhow::Error`. New helper functions
// added to this module should follow the same `Result<T, String>` pattern so
// callers can surface the error message directly to the user without conversion.
use regex::Regex;
use std::sync::OnceLock;

/// URL pattern that matches common URL schemes
static URL_REGEX: OnceLock<Regex> = OnceLock::new();

/// File path pattern that matches Unix-style file paths
static FILE_PATH_REGEX: OnceLock<Regex> = OnceLock::new();

/// Get the compiled URL regex pattern
fn url_regex() -> &'static Regex {
    URL_REGEX.get_or_init(|| {
        // Matches URLs with common schemes (http, https, ftp, etc.)
        // Also matches URLs without schemes that start with www.
        Regex::new(
            r"(?x)
            \b(?:
                # URLs with explicit schemes
                (?:https?|ftps?|file|git|ssh)://[^\s<>{}|\\^`\[\]]+
                |
                # URLs starting with www.
                www\.[^\s<>{}|\\^`\[\]]+
            )\b
            ",
        )
        .expect("Failed to compile URL regex")
    })
}

/// Get the compiled file path regex pattern
fn file_path_regex() -> &'static Regex {
    FILE_PATH_REGEX.get_or_init(|| {
        // Matches file paths at the START of a logical token:
        // - Absolute paths starting with / (must follow whitespace or start of line)
        // - Relative paths starting with ./ or ../
        // - Home-relative paths starting with ~/
        //
        // Absolute paths use a lookbehind to require whitespace or start-of-string
        // before the leading /, preventing false matches inside relative paths
        // like ./a/b/c where /b/c would otherwise also match.
        //
        // Optionally followed by :line_number or :line_number:col_number
        // Also supports other line number formats from iTerm2:
        // - [line, col] or [line,col]
        // - (line, col) or (line,col)
        // - (line)
        Regex::new(
            r#"(?x)
            (?:
                # Home-relative paths (~/...)
                ~/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+
                |
                # Relative paths starting with ./ or ../
                \.\.?/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+
                |
                # Absolute paths: must be at start of string or after whitespace
                # Require at least two path components to reduce false positives
                (?:^|\s)/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+
            )
            # Optional line/column number in various formats
            (?:
                :\d+(?::\d+)?           # :line or :line:col
                | \[\d+(?:,\s?\d+)?\]   # [line] or [line, col]
                | \(\d+(?:,\s?\d+)?\)   # (line) or (line, col)
            )?
            "#,
        )
        .expect("Failed to compile file path regex")
    })
}

/// Type of detected clickable item
#[derive(Debug, Clone, PartialEq)]
pub enum DetectedItemType {
    /// A URL (http, https, etc.)
    Url,
    /// A file path (optionally with line number)
    FilePath {
        /// Line number if specified (e.g., file.rs:42)
        line: Option<usize>,
        /// Column number if specified (e.g., file.rs:42:10)
        column: Option<usize>,
    },
}

/// Detected URL or file path with position information
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedUrl {
    /// The URL or file path text
    pub url: String,
    /// Start column position
    pub start_col: usize,
    /// End column position (exclusive)
    pub end_col: usize,
    /// Row position
    pub row: usize,
    /// OSC 8 hyperlink ID (if this is an OSC 8 hyperlink, None for regex-detected items)
    pub hyperlink_id: Option<u32>,
    /// Type of detected item (URL or FilePath)
    pub item_type: DetectedItemType,
}

/// Detect URLs in a line of text using regex patterns
pub fn detect_urls_in_line(text: &str, row: usize) -> Vec<DetectedUrl> {
    let regex = url_regex();
    let mut urls = Vec::new();

    for mat in regex.find_iter(text) {
        let url = mat.as_str().to_string();
        let start_col = mat.start();
        let end_col = mat.end();

        urls.push(DetectedUrl {
            url,
            start_col,
            end_col,
            row,
            hyperlink_id: None, // Regex-detected URLs don't have OSC 8 IDs
            item_type: DetectedItemType::Url,
        });
    }

    urls
}

/// Detect file paths in a line of text using regex patterns
/// Detects Unix-style paths like /path/to/file, ./relative, ../parent, ~/home
/// Also detects line numbers like file.rs:42 and file.rs:42:10
pub fn detect_file_paths_in_line(text: &str, row: usize) -> Vec<DetectedUrl> {
    let regex = file_path_regex();
    let mut paths = Vec::new();

    for mat in regex.find_iter(text) {
        let full_match = mat.as_str();
        let mut start_col = mat.start();
        let end_col = mat.end();

        // The absolute path branch uses (?:^|\s) which may include a leading
        // whitespace character in the match. Strip it to get the actual path.
        let trimmed_match = if full_match.starts_with(char::is_whitespace) {
            let trimmed = full_match.trim_start();
            start_col += full_match.len() - trimmed.len();
            trimmed
        } else {
            full_match
        };

        // Parse line and column numbers from the path
        let (path, line, column) = parse_path_with_line_number(trimmed_match);

        paths.push(DetectedUrl {
            url: path,
            start_col,
            end_col,
            row,
            hyperlink_id: None,
            item_type: DetectedItemType::FilePath { line, column },
        });
    }

    paths
}

/// Parse a file path that may include line/column suffixes in various formats:
/// - `:line` or `:line:col` (most common)
/// - `[line]` or `[line, col]` (some editors)
/// - `(line)` or `(line, col)` (some error formats)
fn parse_path_with_line_number(path_str: &str) -> (String, Option<usize>, Option<usize>) {
    // Try bracket format: [line] or [line, col]
    if let Some(bracket_start) = path_str.rfind('[')
        && path_str.ends_with(']')
    {
        let path = path_str[..bracket_start].to_string();
        let inner = &path_str[bracket_start + 1..path_str.len() - 1];
        let (line, col) = parse_line_col_pair(inner);
        if line.is_some() {
            return (path, line, col);
        }
    }

    // Try paren format: (line) or (line, col)
    if let Some(paren_start) = path_str.rfind('(')
        && path_str.ends_with(')')
    {
        let path = path_str[..paren_start].to_string();
        let inner = &path_str[paren_start + 1..path_str.len() - 1];
        let (line, col) = parse_line_col_pair(inner);
        if line.is_some() {
            return (path, line, col);
        }
    }

    // Try colon format: :line or :line:col
    let parts: Vec<&str> = path_str.rsplitn(3, ':').collect();

    match parts.len() {
        3 => {
            // file:line:col format
            let col = parts[0].parse::<usize>().ok();
            let line = parts[1].parse::<usize>().ok();
            if line.is_some() {
                let path = parts[2].to_string();
                (path, line, col)
            } else {
                (path_str.to_string(), None, None)
            }
        }
        2 => {
            // file:line format (or just path with colon)
            let line = parts[0].parse::<usize>().ok();
            if line.is_some() {
                let path = parts[1].to_string();
                (path, line, None)
            } else {
                (path_str.to_string(), None, None)
            }
        }
        _ => (path_str.to_string(), None, None),
    }
}

/// Parse "line" or "line, col" or "line,col" into (Option<line>, Option<col>)
fn parse_line_col_pair(s: &str) -> (Option<usize>, Option<usize>) {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    match parts.len() {
        1 => (parts[0].parse().ok(), None),
        2 => (parts[0].parse().ok(), parts[1].parse().ok()),
        _ => (None, None),
    }
}

/// Detect OSC 8 hyperlinks from terminal cells
///
/// # Arguments
/// * `cells` - Slice of cells from a single row
/// * `row` - Row number
/// * `hyperlink_urls` - Mapping from hyperlink_id to URL string
///
/// # Returns
/// Vector of DetectedUrl objects for OSC 8 hyperlinks in this row
pub fn detect_osc8_hyperlinks(
    cells: &[crate::cell_renderer::Cell],
    row: usize,
    hyperlink_urls: &std::collections::HashMap<u32, String>,
) -> Vec<DetectedUrl> {
    let mut urls = Vec::new();
    let mut current_hyperlink: Option<(u32, usize, String)> = None; // (id, start_col, url)

    for (col, cell) in cells.iter().enumerate() {
        match (cell.hyperlink_id, &current_hyperlink) {
            // Cell has a hyperlink ID
            (Some(id), Some((current_id, _start_col, _url))) if id == *current_id => {
                // Continue existing hyperlink (same ID as previous cell)
                continue;
            }
            (Some(id), _) => {
                // Start of a new hyperlink or different hyperlink
                // First, save the previous hyperlink if there was one
                if let Some((prev_id, start_col, url)) = current_hyperlink.take() {
                    urls.push(DetectedUrl {
                        url,
                        start_col,
                        end_col: col, // Previous hyperlink ends at current position
                        row,
                        hyperlink_id: Some(prev_id),
                        item_type: DetectedItemType::Url,
                    });
                }

                // Start new hyperlink if we have a URL for this ID
                if let Some(url) = hyperlink_urls.get(&id) {
                    current_hyperlink = Some((id, col, url.clone()));
                }
            }
            (None, Some((prev_id, start_col, url))) => {
                // End of current hyperlink
                urls.push(DetectedUrl {
                    url: url.clone(),
                    start_col: *start_col,
                    end_col: col, // Hyperlink ends at current position
                    row,
                    hyperlink_id: Some(*prev_id),
                    item_type: DetectedItemType::Url,
                });
                current_hyperlink = None;
            }
            (None, None) => {
                // No hyperlink in this cell or previous cells
                continue;
            }
        }
    }

    // Save last hyperlink if it extends to the end of the row
    if let Some((id, start_col, url)) = current_hyperlink {
        urls.push(DetectedUrl {
            url,
            start_col,
            end_col: cells.len(), // Extends to end of row
            row,
            hyperlink_id: Some(id),
            item_type: DetectedItemType::Url,
        });
    }

    urls
}

/// Check if a specific position is within a URL or file path
pub fn find_url_at_position(urls: &[DetectedUrl], col: usize, row: usize) -> Option<&DetectedUrl> {
    urls.iter()
        .find(|url| url.row == row && col >= url.start_col && col < url.end_col)
}

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
pub(crate) fn shell_escape(s: &str) -> String {
    // Replace single quotes with escaped version and wrap in single quotes
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests;
