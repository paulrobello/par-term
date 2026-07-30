//! Filesystem operations for ACP agent requests.
//!
//! These functions handle `fs/read_text_file`, `fs/write_text_file`,
//! `fs/list_directory`, and `fs/find` RPC calls from the agent.
//! They are executed directly in the async message handler task
//! (via `spawn_blocking`) so they do not depend on UI-thread state.
//!
//! # Security
//!
//! These handlers are the *unconditional floor* on agent filesystem access.
//! They are deliberately a denylist rather than an allowlist, because
//! [`super::permissions::is_safe_write_path`] already answers a different
//! question one layer up ("may this write skip the user prompt?"). A write
//! that reaches here may have been explicitly approved by the user, so the
//! only correct control at this layer is "never, regardless of approval".
//!
//! Three layers of path restriction are enforced:
//!
//! 1. **Sensitive path blocklist** (`is_sensitive_path`): credential stores
//!    such as `~/.ssh/`, `~/.gnupg/`, and `/etc/` are rejected for *both*
//!    reads and writes, even when `auto_approve` is enabled.
//!
//! 2. **Protected write blocklist** (`is_protected_write_path`): paths that
//!    par-term or the operating system later *executes* — shell init files,
//!    launch agents, autostart units, and par-term's own hot-reloaded config
//!    — are rejected for writes. Reads of these are allowed: diagnosing a
//!    user's shell configuration is a core job of a terminal assistant, and
//!    they are not credential stores.
//!
//! 3. **Directory restrictions for listing/find**: `list_directory_entries`
//!    and `find_files_recursive` apply the sensitive blocklist so that a
//!    malicious agent cannot enumerate credential directories.

/// Maximum file size allowed for reading via ACP (50MB).
/// This prevents memory exhaustion from reading multi-GB files.
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50MB

// =========================================================================
// Sensitive path blocklist (SEC-011, SEC-014)
// =========================================================================

/// Resolve `p` for comparison against an already-canonicalized path.
///
/// Both operands must be canonical or the comparison silently fails open: a
/// symlinked `$HOME`, or a `~/.zshrc` symlinked into a dotfiles repository,
/// would otherwise never match the canonicalized incoming path. When `p` does
/// not exist there is nothing to resolve, so the literal path is used — that
/// still denies, which is the safe direction.
fn canonical_or_raw(p: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Sensitive path prefixes that ACP file operations must never access,
/// regardless of `auto_approve` mode.
///
/// The check is performed on the **canonicalized** absolute path, so
/// symlink-based traversal attacks are mitigated before the comparison.
///
/// # Rationale
///
/// - `~/.ssh/`: private keys, authorized_keys, known_hosts
/// - `~/.gnupg/`: PGP private keys
/// - `~/.aws/`: AWS credentials and config
/// - `~/.docker/`: Docker credentials (config.json may contain auth tokens)
/// - `~/.netrc`: plaintext credentials for curl, ftp, and other tools
/// - `~/.config/gh/`: GitHub CLI authentication tokens
/// - `~/.config/gcloud/`: Google Cloud SDK credentials and service account keys
/// - `/etc/`: system configuration, passwd, sudoers, shadow
fn is_sensitive_path(canonical: &std::path::Path) -> bool {
    // Paths under the user's home directory that contain credentials.
    if let Some(home) = dirs::home_dir() {
        let home = canonical_or_raw(&home);
        let ssh_dir = canonical_or_raw(&home.join(".ssh"));
        let gnupg_dir = canonical_or_raw(&home.join(".gnupg"));
        let aws_dir = canonical_or_raw(&home.join(".aws"));
        let docker_dir = canonical_or_raw(&home.join(".docker"));
        let netrc_file = canonical_or_raw(&home.join(".netrc"));
        let gh_config_dir = canonical_or_raw(&home.join(".config").join("gh"));
        let gcloud_config_dir = canonical_or_raw(&home.join(".config").join("gcloud"));

        if canonical.starts_with(&ssh_dir)
            || canonical.starts_with(&gnupg_dir)
            || canonical.starts_with(&aws_dir)
            || canonical.starts_with(&docker_dir)
            || canonical == netrc_file
            || canonical.starts_with(&gh_config_dir)
            || canonical.starts_with(&gcloud_config_dir)
        {
            return true;
        }
    }
    // System credential and configuration directories.
    if canonical.starts_with("/etc/") || canonical == std::path::Path::new("/etc") {
        return true;
    }
    false
}

/// Canonicalize `path` and check it against the sensitive path blocklist.
/// Returns `Ok(canonical)` when safe, `Err(message)` when blocked.
fn check_path_allowed(path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::Path::new(path);

    // Resolve the canonical path (follows symlinks, resolves ..).
    // For non-existent paths, canonicalize the parent and re-append the filename
    // so that new-file creation in safe directories is still allowed.
    let canonical = if p.exists() {
        std::fs::canonicalize(p).map_err(|e| format!("Cannot resolve path: {e}"))?
    } else {
        let parent = p
            .parent()
            .ok_or_else(|| "Path has no parent directory".to_string())?;
        let canonical_parent =
            std::fs::canonicalize(parent).map_err(|e| format!("Cannot resolve parent: {e}"))?;
        let file_name = p
            .file_name()
            .ok_or_else(|| "Path has no file name".to_string())?;
        canonical_parent.join(file_name)
    };

    if is_sensitive_path(&canonical) {
        return Err(format!(
            "Access denied: '{}' is in a restricted directory. \
             ACP agents cannot read or list ~/.ssh/, ~/.gnupg/, ~/.aws/, ~/.docker/, \
             ~/.netrc, ~/.config/gh/, ~/.config/gcloud/, or /etc/.",
            path
        ));
    }

    Ok(canonical)
}

/// Paths that par-term or the operating system later *executes*, and which an
/// ACP agent must therefore never overwrite — regardless of `auto_approve` or
/// of an explicit user approval granted at the permission layer.
///
/// Reads are intentionally *not* blocked here. These files are not credential
/// stores, and reading them is a legitimate and frequent request for a
/// terminal assistant ("why is my PATH wrong?"). Only the write side turns
/// them into a command-execution primitive.
///
/// # Categories
///
/// - **Shell init files** at the home root: sourced by every new shell, so a
///   write is equivalent to arbitrary command execution on the user's next
///   prompt. Matched by exact path so a repository's own `.bashrc` fixture
///   stays writable.
/// - **Auto-start / persistence locations**: launchd agents and daemons on
///   macOS, XDG autostart and systemd user units on Linux, and the Start Menu
///   Startup folder on Windows.
/// - **par-term's own executable-chain config**, when `config_dir` is known:
///   `config.yaml` is hot-reloaded by design for ACP agents
///   (`src/app/window_state/config_watchers.rs`) and carries triggers and
///   automation; `profiles.yaml` carries `command` / `command_args` that
///   profile auto-switch runs; `arrangements.yaml` and `last_session.yaml`
///   respawn panes; and `agents/` holds ACP agent definitions with
///   `run_command`. The rest of the directory — `shaders/`, `sounds/`,
///   `.config-update.json` — stays writable, which is why this is a
///   file-level rule and not a prefix on `config_dir`.
fn is_protected_write_path(
    canonical: &std::path::Path,
    config_dir: Option<&std::path::Path>,
) -> bool {
    if let Some(home) = dirs::home_dir() {
        let home = canonical_or_raw(&home);
        // Shell startup files. A write here executes on the next shell.
        const SHELL_INIT_FILES: &[&str] = &[
            ".bashrc",
            ".bash_profile",
            ".bash_login",
            ".bash_logout",
            ".profile",
            ".zshrc",
            ".zshenv",
            ".zprofile",
            ".zlogin",
            ".zlogout",
            ".cshrc",
            ".tcshrc",
            ".kshrc",
            ".login",
            ".logout",
            // readline can bind a key to an arbitrary command macro.
            ".inputrc",
        ];
        if SHELL_INIT_FILES
            .iter()
            .any(|name| canonical == canonical_or_raw(&home.join(name)))
        {
            return true;
        }

        // fish keeps its startup files in a directory rather than a dotfile.
        // fish resolves this under ~/.config even on macOS, so this does not
        // go through `dirs::config_dir()`.
        let dot_config = home.join(".config");
        if canonical.starts_with(canonical_or_raw(&dot_config.join("fish"))) {
            return true;
        }

        // Auto-start / persistence locations.
        let auto_start_roots = [
            home.join("Library").join("LaunchAgents"),
            home.join("Library").join("LaunchDaemons"),
            dot_config.join("autostart"),
            dot_config.join("systemd").join("user"),
        ];
        if auto_start_roots
            .iter()
            .any(|root| canonical.starts_with(canonical_or_raw(root)))
        {
            return true;
        }
    }

    #[cfg(target_os = "macos")]
    if canonical.starts_with("/Library/LaunchAgents")
        || canonical.starts_with("/Library/LaunchDaemons")
        || canonical.starts_with("/System/Library/LaunchAgents")
        || canonical.starts_with("/System/Library/LaunchDaemons")
    {
        return true;
    }

    #[cfg(windows)]
    if let Some(app_data) = dirs::config_dir()
        && canonical.starts_with(
            app_data
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
                .join("Startup"),
        )
    {
        return true;
    }

    // par-term's own config files that feed a command-execution path.
    if let Some(config_dir) = config_dir {
        let config_dir = canonical_or_raw(config_dir);
        const EXECUTABLE_CONFIG_FILES: &[&str] = &[
            "config.yaml",
            "config.yml",
            "profiles.yaml",
            "profiles.yml",
            "arrangements.yaml",
            "arrangements.yml",
            "last_session.yaml",
            "last_session.yml",
        ];
        if EXECUTABLE_CONFIG_FILES
            .iter()
            .any(|name| canonical == canonical_or_raw(&config_dir.join(name)))
        {
            return true;
        }
        // ACP agent definitions carry `run_command`.
        if canonical.starts_with(canonical_or_raw(&config_dir.join("agents"))) {
            return true;
        }
    }

    false
}

/// Canonicalize `path` and check it against both the sensitive-path blocklist
/// and the protected-write blocklist.
///
/// `config_dir` is par-term's configuration directory, when known, so that the
/// hot-reloaded `config.yaml` and its siblings can be protected without
/// duplicating the XDG resolution that `par-term-config` owns.
fn check_write_path_allowed(
    path: &str,
    config_dir: Option<&std::path::Path>,
) -> Result<std::path::PathBuf, String> {
    let canonical = check_path_allowed(path)?;
    if is_protected_write_path(&canonical, config_dir) {
        return Err(format!(
            "Access denied: '{}' is executed by par-term or the operating system, \
             so ACP agents cannot write to it. This covers shell startup files \
             (~/.zshrc, ~/.bashrc, ~/.profile, ...), auto-start locations \
             (~/Library/LaunchAgents/, ~/.config/autostart/, ...), and par-term's \
             own config.yaml / profiles.yaml / agents/. Use the `config/update` \
             RPC to change par-term settings.",
            path
        ));
    }
    Ok(canonical)
}

/// Read a text file, optionally returning a line range.
///
/// `line` is 1-based (line 1 is the first line).
///
/// # Security
///
/// - Files larger than `MAX_FILE_SIZE` (50MB) are rejected.
/// - Paths under `~/.ssh/`, `~/.gnupg/`, and `/etc/` are unconditionally blocked.
pub fn read_file_with_range(
    path: &str,
    line: Option<u64>,
    limit: Option<u64>,
) -> Result<String, String> {
    // SEC-011: Validate path against sensitive directory blocklist before reading.
    check_path_allowed(path)?;

    // Check file size before reading to prevent memory exhaustion.
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "File too large: {} bytes (max {} bytes)",
            metadata.len(),
            MAX_FILE_SIZE
        ));
    }

    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;

    match (line, limit) {
        (None, None) => Ok(content),
        _ => {
            let skip = line.unwrap_or(1).saturating_sub(1) as usize;
            let lines: Vec<&str> = content.lines().skip(skip).collect();
            let taken: Vec<&str> = if let Some(lim) = limit {
                lines.into_iter().take(lim as usize).collect()
            } else {
                lines
            };
            Ok(taken.join("\n"))
        }
    }
}

/// Write content to a file, creating parent directories as needed.
///
/// Requires an absolute path for safety.
///
/// `config_dir` is par-term's configuration directory when known; pass `None`
/// only when there is no par-term config to protect.
///
/// # Security
///
/// Rejects credential paths (`~/.ssh/`, `~/.aws/`, `/etc/`, ...) and paths that
/// par-term or the OS later executes (shell init files, launch agents,
/// `config.yaml`). See `check_write_path_allowed`.
pub fn write_file_safe(
    path: &str,
    content: &str,
    config_dir: Option<&std::path::Path>,
) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    // SEC-001: Validate path against the sensitive directory blocklist and the
    // protected-write blocklist before writing. Mirrors read_file_with_range /
    // list_directory_entries / find_files_recursive so an agent cannot
    // overwrite files under ~/.ssh/, ~/.aws/, /etc/, nor any file par-term or
    // the OS will subsequently execute.
    check_write_path_allowed(path, config_dir)?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {e}"))?;
    }
    std::fs::write(p, content).map_err(|e| format!("Failed to write file: {e}"))
}

/// List directory entries, optionally filtering by a glob-like pattern.
///
/// Returns a sorted vec of JSON objects with `name`, `path`, `isDirectory`, and
/// `isFile` fields.
///
/// # Security
///
/// Paths under `~/.ssh/`, `~/.gnupg/`, and `/etc/` are blocked (SEC-014).
pub fn list_directory_entries(
    path: &str,
    pattern: Option<&str>,
) -> Result<Vec<serde_json::Value>, String> {
    let dir = std::path::Path::new(path);
    if !dir.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    // SEC-014: Block listing of sensitive directories.
    check_path_allowed(path)?;
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {e}"))?;

    let mut result: Vec<serde_json::Value> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Simple glob matching: supports "*.ext" and "*" patterns.
        if let Some(pat) = pattern
            && !glob_match_simple(pat, &name)
        {
            continue;
        }

        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        result.push(serde_json::json!({
            "name": name,
            "path": entry.path().to_string_lossy(),
            "isDirectory": file_type.is_dir(),
            "isFile": file_type.is_file(),
        }));
    }
    result.sort_by(|a, b| {
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a_name.cmp(b_name)
    });
    Ok(result)
}

/// Maximum directory depth for recursive file searches.
/// This prevents stack overflow from deep directory trees or symlink loops.
const MAX_SEARCH_DEPTH: usize = 20;

/// Recursively find files matching a glob pattern.
///
/// Supports simple patterns like `*.glsl`, `**/*.rs`, and literal names.
/// Returns a sorted list of absolute file paths.
///
/// # Security
///
/// - Maximum recursion depth is limited to `MAX_SEARCH_DEPTH` to prevent stack overflow.
/// - Symlinks are skipped to prevent infinite loops from symlink cycles.
/// - Paths under `~/.ssh/`, `~/.gnupg/`, and `/etc/` are blocked (SEC-014).
pub fn find_files_recursive(base_path: &str, pattern: &str) -> Result<Vec<String>, String> {
    let base = std::path::Path::new(base_path);
    if !base.is_absolute() {
        return Err("Path must be absolute".to_string());
    }
    if !base.exists() {
        return Err(format!("Path does not exist: {base_path}"));
    }
    // SEC-014: Block recursive search of sensitive directories.
    check_path_allowed(base_path)?;

    let mut results = Vec::new();
    // Strip leading **/ for simple recursive matching.
    let file_pattern = pattern.strip_prefix("**/").unwrap_or(pattern);

    fn walk_dir(
        dir: &std::path::Path,
        file_pattern: &str,
        results: &mut Vec<String>,
        remaining_depth: usize,
    ) -> Result<(), String> {
        // Stop recursion if we've reached the maximum depth.
        if remaining_depth == 0 {
            return Ok(());
        }

        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            // Get file type and skip symlinks to prevent infinite loops from symlink cycles.
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                walk_dir(&path, file_pattern, results, remaining_depth - 1)?;
            } else if file_type.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if glob_match_simple(file_pattern, &name) {
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }
        Ok(())
    }

    walk_dir(base, file_pattern, &mut results, MAX_SEARCH_DEPTH)?;
    results.sort();
    Ok(results)
}

/// Simple glob matching for directory listing filters.
///
/// Supports `*` (match anything), `*.ext` (match extension), and literal names.
pub fn glob_match_simple(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{ext}"));
    }
    if let Some(prefix) = pattern.strip_suffix("*") {
        return name.starts_with(prefix);
    }
    name == pattern
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// Home-rooted cases are asserted against the predicate rather than
    /// [`write_file_safe`], deliberately: a test whose failure mode is
    /// "overwrite the developer's real ~/.zshrc" is not an acceptable test.
    /// End-to-end coverage of the same code path is provided below via a
    /// temporary `config_dir`.
    fn home() -> PathBuf {
        dirs::home_dir().expect("home dir")
    }

    #[test]
    fn shell_init_files_are_write_protected() {
        for name in [
            ".zshrc",
            ".zshenv",
            ".zprofile",
            ".bashrc",
            ".bash_profile",
            ".profile",
            ".inputrc",
        ] {
            let path = home().join(name);
            assert!(
                is_protected_write_path(&path, None),
                "{name} must be write-protected"
            );
        }
    }

    #[test]
    fn fish_config_directory_is_write_protected() {
        let path = home().join(".config").join("fish").join("config.fish");
        assert!(is_protected_write_path(&path, None));
    }

    #[test]
    fn auto_start_locations_are_write_protected() {
        for path in [
            home()
                .join("Library")
                .join("LaunchAgents")
                .join("evil.plist"),
            home()
                .join("Library")
                .join("LaunchDaemons")
                .join("evil.plist"),
            home()
                .join(".config")
                .join("autostart")
                .join("evil.desktop"),
            home()
                .join(".config")
                .join("systemd")
                .join("user")
                .join("evil.service"),
        ] {
            assert!(
                is_protected_write_path(&path, None),
                "{} must be write-protected",
                path.display()
            );
        }
    }

    #[test]
    fn shell_init_files_stay_readable() {
        // Deliberate asymmetry: these are command-execution vectors on write,
        // but they are not credential stores, and reading them is a core
        // terminal-assistant task.
        assert!(!is_sensitive_path(&home().join(".zshrc")));
        assert!(!is_sensitive_path(&home().join(".bashrc")));
    }

    #[test]
    fn credential_paths_stay_read_protected() {
        assert!(is_sensitive_path(&home().join(".ssh").join("id_ed25519")));
        assert!(is_sensitive_path(&home().join(".aws").join("credentials")));
        assert!(is_sensitive_path(Path::new("/etc/passwd")));
    }

    #[test]
    fn par_term_executable_config_is_write_protected() {
        let config_dir = PathBuf::from("/opt/par-term-config");
        for name in [
            "config.yaml",
            "profiles.yaml",
            "arrangements.yaml",
            "last_session.yaml",
        ] {
            assert!(
                is_protected_write_path(&config_dir.join(name), Some(&config_dir)),
                "{name} must be write-protected"
            );
        }
        assert!(is_protected_write_path(
            &config_dir.join("agents").join("rogue.toml"),
            Some(&config_dir)
        ));
    }

    #[test]
    fn par_term_non_executable_config_stays_writable() {
        // The rule is file-level, not a prefix on config_dir: shaders and the
        // MCP config-update handoff file must keep working.
        let config_dir = PathBuf::from("/opt/par-term-config");
        for path in [
            config_dir.join("shaders").join("crt.glsl"),
            config_dir.join(".config-update.json"),
            config_dir.join("command_history.yaml"),
            config_dir.join("sounds").join("bell.wav"),
        ] {
            assert!(
                !is_protected_write_path(&path, Some(&config_dir)),
                "{} must stay writable",
                path.display()
            );
        }
    }

    #[test]
    fn write_file_safe_rejects_hot_reloaded_config() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().to_path_buf();
        let target = config_dir.join("config.yaml");

        let err = write_file_safe(
            &target.to_string_lossy(),
            "triggers: [pwned]",
            Some(&config_dir),
        )
        .expect_err("writing config.yaml must be denied");

        assert!(err.contains("Access denied"), "unexpected message: {err}");
        assert!(!target.exists(), "denied write must not create the file");
    }

    #[test]
    fn write_file_safe_allows_shader_inside_config_dir() {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_dir = temp.path().to_path_buf();
        let target = config_dir.join("shaders").join("crt.glsl");
        // `check_path_allowed` canonicalizes the parent, so the parent must
        // already exist. See the nested-directory note in the crate report:
        // `write_file_safe` documents "creating parent directories as needed"
        // but checks the path before creating them.
        std::fs::create_dir_all(target.parent().expect("parent")).expect("create shaders dir");

        write_file_safe(
            &target.to_string_lossy(),
            "void main() {}",
            Some(&config_dir),
        )
        .expect("writing a shader must be allowed");

        assert_eq!(
            std::fs::read_to_string(&target).expect("read back"),
            "void main() {}"
        );
    }

    #[test]
    fn write_file_safe_rejects_credential_directory() {
        let target = home().join(".ssh").join("authorized_keys");
        let err = write_file_safe(&target.to_string_lossy(), "ssh-rsa AAAA", None)
            .expect_err("writing into ~/.ssh must be denied");
        assert!(err.contains("Access denied"), "unexpected message: {err}");
    }

    #[test]
    fn read_file_with_range_rejects_credential_directory() {
        let target = home().join(".ssh").join("id_ed25519");
        let err = read_file_with_range(&target.to_string_lossy(), None, None)
            .expect_err("reading ~/.ssh must be denied");
        // When ~/.ssh is absent the parent canonicalization fails first; both
        // outcomes are a refusal, which is what this asserts.
        assert!(
            err.contains("Access denied") || err.contains("Cannot resolve"),
            "unexpected message: {err}"
        );
    }
}
