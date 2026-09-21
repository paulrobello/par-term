//! Git-backed plugin distribution: `add`, `update`, `remove`.
//!
//! The `par-term plugin add|update|remove` CLI subcommands and their Settings
//! UI equivalents run through this module. Everything shells out to the `git`
//! binary (the same pattern as the root crate's status-bar git poller) rather
//! than adding a git crate, and EVERY invocation runs with
//! `GIT_TERMINAL_PROMPT=0` under a hard deadline: these calls run inside the
//! GPU process (Settings UI) where an auth prompt has no terminal to answer
//! it. The env var covers git's own prompts; the deadline is the backstop for
//! prompts it cannot reach — an `ssh` passphrase ask writes to the
//! controlling tty directly, and the kill turns that hang into an error.
//!
//! Security shape (board card 01a0c5253b287cb39a11bf6182634197):
//! - `add` clones to a staging directory first (a clone target named by the
//!   URL says nothing about the manifest id, which must equal the final
//!   directory name), validates the manifest on the final path, and refuses
//!   to overwrite an existing target. Plugins land disabled by construction:
//!   nothing here touches the enabled set.
//! - `update` fetches, shows a diff, and fast-forwards only. A history
//!   rewrite upstream is refused, never reset — the trust model rests on the
//!   user reading plugin code before enabling it, and a reset would silently
//!   discard exactly that (decision D3).
//! - `remove` only touches directories `add` could have created: a `.git`
//!   directory WITH an `origin` remote. A hand-copied plugin (no `.git`) and
//!   a bare `git init` (no remote) are both refused.

use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::manifest::{DiscoveredPlugin, PluginManifest, validate_plugin_dir};

/// Deadline for `git clone`. Generous for a slow clone of a real plugin
/// repository; the point is that no git invocation can hang the caller
/// forever.
const CLONE_TIMEOUT: Duration = Duration::from_secs(180);
/// Deadline for `git fetch` / `git pull`.
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);
const PULL_TIMEOUT: Duration = Duration::from_secs(60);
/// Deadline for diff/stat-producing invocations.
const DIFF_TIMEOUT: Duration = Duration::from_secs(30);
/// Deadline for the small read-only invocations (rev-parse, remote).
const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// How often the bounded runner re-checks whether git has exited.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// A fetched-but-not-applied update, for display before the fast-forward
/// runs.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdatePreview {
    /// True when FETCH_HEAD equals HEAD — there is nothing to apply.
    pub up_to_date: bool,
    /// Short sha of the installed commit.
    pub current: String,
    /// Short sha of the fetched commit.
    pub incoming: String,
    /// `git diff --stat HEAD..FETCH_HEAD`, shown to the user before
    /// applying.
    pub diff: String,
}

/// Clone `url` into `root` under the plugin's manifest id, validating the
/// result. Refuses non-URL sources (decision D1: local installs stay the
/// documented hand-copy), refuses an existing target directory by naming it,
/// and removes everything it staged on any failure. The plugin lands
/// disabled: enabling is a separate config step nothing here performs.
pub fn add(root: &Path, url: &str) -> Result<DiscoveredPlugin, String> {
    if !is_git_url(url) {
        return Err(format!(
            "`{url}` is not a git URL; plugin add installs from git URLs only. \
             Local plugin directories are installed by copying them into {} by hand.",
            root.display()
        ));
    }
    std::fs::create_dir_all(root)
        .map_err(|e| format!("cannot create plugins root {}: {e}", root.display()))?;

    // Stage the clone under root itself, dot-prefixed: same filesystem, so
    // the rename into place cannot hit a cross-device boundary. Discovery
    // skips nothing by name, so a staging directory left behind by a crash
    // surfaces as a visible warning instead of disappearing silently.
    let staging = root.join(format!(".incoming-{}-{}", std::process::id(), now_millis()));
    let result = (|| {
        let mut clone = git(Some(root), &[]);
        clone.arg("clone").arg("--quiet").arg(url).arg(&staging);
        let out = output_with_timeout(&mut clone, CLONE_TIMEOUT)?;
        if !out.status.success() {
            let stderr = out.stderr.trim();
            return Err(format!(
                "git clone failed: {}{}",
                out.status,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            ));
        }
        stage_into_place(root, &staging)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

/// Move a freshly cloned staging directory to its final `<root>/<manifest
/// id>` home and validate it there. Every error path leaves the plugins root
/// without a new entry (the caller removes the staging directory; this
/// function removes the moved directory when validation fails).
fn stage_into_place(root: &Path, staging: &Path) -> Result<DiscoveredPlugin, String> {
    // The manifest id decides the final directory name, and the URL it was
    // cloned from says nothing about it — read it from the clone.
    let manifest_raw = std::fs::read_to_string(staging.join("manifest.json"))
        .map_err(|e| format!("cloned repository has no readable manifest.json: {e}"))?;
    let manifest: PluginManifest = serde_json::from_str(&manifest_raw)
        .map_err(|e| format!("cloned manifest.json is invalid: {e}"))?;
    if !safe_segment(&manifest.id) {
        return Err(format!(
            "manifest id `{}` is not a usable plugin directory name",
            manifest.id
        ));
    }
    let final_dir = root.join(&manifest.id);
    if final_dir.exists() {
        return Err(format!(
            "plugin directory already exists: {}. Remove it first \
             (`plugin remove {}` if it was git-installed) and add again.",
            final_dir.display(),
            manifest.id
        ));
    }
    std::fs::rename(staging, &final_dir)
        .map_err(|e| format!("failed to move the cloned plugin into place: {e}"))?;
    // Full validation runs on the final path only — validate_plugin_dir
    // requires the directory name to equal the manifest id, which is not
    // true of the staging name.
    match validate_plugin_dir(&final_dir) {
        Ok(plugin) => Ok(plugin),
        Err(reason) => {
            // Never leave behind a plugin that would only ever surface as a
            // discovery warning.
            let _ = std::fs::remove_dir_all(&final_dir);
            Err(format!("cloned plugin failed validation: {reason}"))
        }
    }
}

/// Fetch upstream for a git-installed plugin and describe what applying
/// would bring, without applying anything — the user sees this diff before
/// the fast-forward runs.
pub fn update_fetch(dir: &Path) -> Result<UpdatePreview, String> {
    require_installed(dir)?;
    let branch = run_git_ok(
        Some(dir),
        &["rev-parse", "--abbrev-ref", "HEAD"],
        READ_TIMEOUT,
        "git rev-parse HEAD",
    )?;
    let fetch_args = ["fetch", "--quiet", "origin", branch.as_str()];
    run_git_ok(Some(dir), &fetch_args, FETCH_TIMEOUT, "git fetch")?;
    let current = run_git_ok(
        Some(dir),
        &["rev-parse", "--short", "HEAD"],
        READ_TIMEOUT,
        "git rev-parse HEAD",
    )?;
    let incoming = run_git_ok(
        Some(dir),
        &["rev-parse", "--short", "FETCH_HEAD"],
        READ_TIMEOUT,
        "git rev-parse FETCH_HEAD",
    )?;
    if current == incoming {
        return Ok(UpdatePreview {
            up_to_date: true,
            current,
            incoming,
            diff: String::new(),
        });
    }
    let diff = run_git_ok(
        Some(dir),
        &["diff", "--stat", "HEAD..FETCH_HEAD"],
        DIFF_TIMEOUT,
        "git diff",
    )?;
    Ok(UpdatePreview {
        up_to_date: false,
        current,
        incoming,
        diff,
    })
}

/// Fast-forward a git-installed plugin to the fetched upstream. Refuses when
/// a fast-forward is impossible — never resets. Nothing is changed on
/// refusal; if upstream rewrote history, removing and re-adding the plugin
/// is the user's explicit way to take the rewritten version (decision D3).
pub fn update_apply(dir: &Path) -> Result<String, String> {
    require_installed(dir)?;
    let out = run_git(Some(dir), &["pull", "--ff-only", "--quiet"], PULL_TIMEOUT)?;
    if !out.status.success() {
        let stderr = out.stderr.trim();
        return Err(format!(
            "update refused — `git pull --ff-only` failed and nothing was changed. \
             If upstream rewrote history, remove and re-add the plugin to take its version. \
             git said: {}{}",
            out.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }
    run_git_ok(
        Some(dir),
        &["rev-parse", "--short", "HEAD"],
        READ_TIMEOUT,
        "git rev-parse HEAD",
    )
}

/// Remove a git-installed plugin directory. Refuses anything `add` could not
/// have created: no `.git` directory (hand-copied), or `.git` without an
/// `origin` remote (a bare `git init`).
pub fn remove(root: &Path, id: &str) -> Result<(), String> {
    if !safe_segment(id) {
        return Err(format!("`{id}` is not a plugin directory name"));
    }
    let dir = root.join(id);
    if !dir.is_dir() {
        return Err(format!(
            "no plugin directory named `{id}` under {}",
            root.display()
        ));
    }
    require_installed(&dir)?;
    std::fs::remove_dir_all(&dir).map_err(|e| format!("failed to remove {}: {e}", dir.display()))
}

/// True when `dir` looks installed by `add`: a `.git` directory with an
/// `origin` remote. Hand-copied plugins have no `.git`; a bare `git init`
/// has `.git` but no remote. The Settings UI uses this to decide which rows
/// offer Update/Remove.
pub fn is_git_installed(dir: &Path) -> bool {
    dir.join(".git").is_dir() && origin_url(dir).is_some()
}

fn origin_url(dir: &Path) -> Option<String> {
    run_git_ok(
        Some(dir),
        &["remote", "get-url", "origin"],
        READ_TIMEOUT,
        "git remote get-url",
    )
    .ok()
}

fn require_installed(dir: &Path) -> Result<(), String> {
    if !dir.join(".git").is_dir() {
        return Err(format!(
            "{} was not installed by plugin add (no .git directory). \
             Hand-copied plugins are removed by deleting their directory.",
            dir.display()
        ));
    }
    if origin_url(dir).is_none() {
        return Err(format!(
            "{} has .git but no `origin` remote, so it was not installed by \
             plugin add. Remove its directory by hand.",
            dir.display()
        ));
    }
    Ok(())
}

/// True for plugin ids usable as one directory segment under the plugins
/// root. `git clone` names its target from the URL, but the manifest id
/// decides the final directory, and a hostile or careless `../`-style id
/// must not move the rename outside the root.
fn safe_segment(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\']) && !id.contains('\0')
}

/// True for sources `add` accepts: scheme-qualified git URLs (https://,
/// git://, ssh://, file://, …) and scp-like `user@host:path` forms. Bare
/// filesystem paths — absolute, relative, `~/` — are refused (decision D1).
fn is_git_url(url: &str) -> bool {
    let url = url.trim();
    if url.is_empty() {
        return false;
    }
    if let Some((scheme, rest)) = url.split_once("://") {
        return !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            && !rest.is_empty();
    }
    match url.split_once(':') {
        // scp-like form: a colon before any separator, e.g.
        // git@github.com:owner/repo.git
        Some((host, rest)) => !host.is_empty() && !host.contains('/') && !rest.is_empty(),
        None => false,
    }
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Captured result of a git invocation that exited within its deadline.
#[derive(Debug)]
struct BoundedOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

/// Run `cmd` to completion, killing it if it outlives `timeout`.
///
/// Mirrors the root crate's `src/process_timeout.rs`; this private copy
/// exists because the root crate depends on THIS crate, so the helper cannot
/// be shared. Draining both pipes on helper threads matters as much here as
/// there: `git pull` can emit more than a pipe buffer of progress output,
/// and polling `try_wait` without reading would deadlock.
fn output_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<BoundedOutput, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn: {e}"))?;

    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "stdout pipe unavailable".to_string())?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| "stderr pipe unavailable".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = child_stdout.read_to_string(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = child_stderr.read_to_string(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("timed out after {:.1}s", timeout.as_secs_f64()));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => return Err(format!("failed while waiting: {e}")),
        }
    };

    Ok(BoundedOutput {
        status,
        stdout: stdout_reader
            .join()
            .map_err(|_| "stdout reader thread panicked".to_string())?,
        stderr: stderr_reader
            .join()
            .map_err(|_| "stderr reader thread panicked".to_string())?,
    })
}

/// Build a `git` invocation that can never stop to ask the user anything.
fn git(cwd: Option<&Path>, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.args(args);
    cmd
}

fn run_git(cwd: Option<&Path>, args: &[&str], timeout: Duration) -> Result<BoundedOutput, String> {
    output_with_timeout(&mut git(cwd, args), timeout)
}

/// Run git, require exit 0, and return trimmed stdout. The error names the
/// operation, the exit status, and git's stderr — the CLI and Settings UI
/// surface this string verbatim.
fn run_git_ok(
    cwd: Option<&Path>,
    args: &[&str],
    timeout: Duration,
    what: &str,
) -> Result<String, String> {
    let out = run_git(cwd, args, timeout)?;
    if !out.status.success() {
        let stderr = out.stderr.trim();
        return Err(format!(
            "{what} failed: {}{}",
            out.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ));
    }
    Ok(out.stdout.trim().to_string())
}

// Tests run real git against file:// remotes built in temp directories — no
// network. The shell-out fixtures need unix (exec-bit, sh); the pure-logic
// ones run everywhere git exists.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::discover_plugins;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Minimal valid panel-plugin manifest (the same shape as manifest.rs's
    /// MINIMAL_PANEL_MANIFEST fixture). `{id}` is replaced per fixture.
    const PANEL_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "{id}",
        "name": "Test Panel",
        "version": "0.1.0",
        "kinds": ["panel"],
        "entryPoints": { "panel": { "command": "panel.sh", "args": [] } }
    }"#;

    /// Parses as a manifest but fails validation: unknown kind.
    const BOGUS_KIND_MANIFEST: &str = r#"{
        "schemaVersion": 1,
        "id": "{id}",
        "name": "Bogus",
        "version": "0.1.0",
        "kinds": ["not-a-kind"],
        "entryPoints": { "panel": { "command": "panel.sh", "args": [] } }
    }"#;

    fn write_exec_file(path: &Path) {
        fs::write(path, "#!/bin/sh\nsleep 30\n").expect("write entry");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path).expect("entry metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms).expect("chmod entry");
        }
    }

    fn git_ok(cwd: &Path, args: &[&str]) -> String {
        run_git_ok(Some(cwd), args, READ_TIMEOUT, "git").expect("git fixture command")
    }

    fn git_commit(cwd: &Path, msg: &str) {
        git_ok(
            cwd,
            &[
                "-c",
                "user.name=par-term-test",
                "-c",
                "user.email=test@par-term.test",
                "commit",
                "--quiet",
                "-m",
                msg,
            ],
        );
    }

    /// Create a source git repository at `<tmp>/src` holding one valid
    /// panel plugin with the given id, committed; return its path.
    fn source_repo(tmp: &Path, id: &str, manifest: &str) -> PathBuf {
        let src = tmp.join("src");
        fs::create_dir_all(&src).expect("create src");
        fs::write(src.join("manifest.json"), manifest.replace("{id}", id)).expect("write manifest");
        write_exec_file(&src.join("panel.sh"));
        git_ok(&src, &["init", "--quiet"]);
        git_ok(&src, &["add", "-A"]);
        git_commit(&src, "initial plugin");
        src
    }

    fn file_url(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    /// One prepared world per test: a source repo plus an empty plugins
    /// root, siblings under a temp directory.
    struct Fixture {
        _tmp: TempDir,
        src: PathBuf,
        root: PathBuf,
    }

    fn fixture(id: &str, manifest: &str) -> Fixture {
        let tmp = TempDir::new().expect("temp dir");
        let src = source_repo(tmp.path(), id, manifest);
        let root = tmp.path().join("plugins");
        Fixture {
            _tmp: tmp,
            src,
            root,
        }
    }

    fn installed_plugin(fx: &Fixture) -> PathBuf {
        fx.root.join("com.test.panel")
    }

    #[test]
    fn add_clones_into_place_and_lands_discoverable() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        let added = add(&fx.root, &file_url(&fx.src)).expect("add should succeed");
        assert_eq!(added.manifest.id, "com.test.panel");
        // Landed under the manifest id, discoverable, git-installed.
        assert!(installed_plugin(&fx).is_dir());
        let (plugins, warnings) = discover_plugins(&fx.root);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.id, "com.test.panel");
        assert!(is_git_installed(&installed_plugin(&fx)));
    }

    #[test]
    fn add_refuses_an_existing_target_directory() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        let existing = installed_plugin(&fx);
        fs::create_dir_all(&existing).expect("pre-create target");
        fs::write(existing.join("keep.txt"), "hand-copied").expect("seed target");
        let err = add(&fx.root, &file_url(&fx.src)).expect_err("must refuse");
        assert!(
            err.contains("already exists"),
            "error should name the refusal: {err}"
        );
        assert!(
            err.contains(existing.display().to_string().as_str()),
            "error should name the target directory: {err}"
        );
        // The hand-placed directory is untouched.
        assert_eq!(
            fs::read_to_string(existing.join("keep.txt")).unwrap(),
            "hand-copied"
        );
    }

    #[test]
    fn add_refuses_non_url_sources() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        for bad in ["/absolute/path", "relative/dir", "~/plugin", "."] {
            let err = add(&fx.root, bad).expect_err("local paths must be refused");
            assert!(
                err.contains("not a git URL"),
                "error for `{bad}` should state the refusal: {err}"
            );
        }
    }

    #[test]
    fn add_surfaces_a_clone_failure_cleanly() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        let nowhere = fx._tmp.path().join("does-not-exist");
        let err = add(&fx.root, &file_url(&nowhere)).expect_err("clone must fail");
        assert!(err.contains("git clone failed"), "unexpected error: {err}");
        assert!(!installed_plugin(&fx).exists());
    }

    #[test]
    fn add_removes_everything_it_staged_when_validation_fails() {
        let fx = fixture("com.test.bad", BOGUS_KIND_MANIFEST);
        let err = add(&fx.root, &file_url(&fx.src)).expect_err("must refuse");
        assert!(
            err.contains("failed validation"),
            "error should carry the validation reason: {err}"
        );
        assert!(
            err.contains("unknown kinds"),
            "error should carry the validation reason: {err}"
        );
        // Neither the plugin nor the staging clone is left behind.
        assert!(!fx.root.join("com.test.bad").exists());
        let leftovers: Vec<_> = fs::read_dir(&fx.root)
            .expect("plugins root exists")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(leftovers.is_empty(), "leftovers: {leftovers:?}");
    }

    #[test]
    fn update_fetch_reports_a_diff_then_applies_a_fast_forward() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        add(&fx.root, &file_url(&fx.src)).expect("add should succeed");

        // Upstream gains a commit touching one new file.
        fs::write(fx.src.join("feature.txt"), "new upstream feature").expect("write feature");
        git_ok(&fx.src, &["add", "-A"]);
        git_commit(&fx.src, "add feature");

        let dir = installed_plugin(&fx);
        let preview = update_fetch(&dir).expect("fetch should succeed");
        assert!(!preview.up_to_date);
        assert_ne!(preview.current, preview.incoming);
        assert!(
            preview.diff.contains("feature.txt"),
            "diff should name the changed file: {}",
            preview.diff
        );
        assert!(!dir.join("feature.txt").exists(), "not applied yet");

        let new_head = update_apply(&dir).expect("apply should fast-forward");
        assert_eq!(new_head, preview.incoming);
        assert!(dir.join("feature.txt").exists());
    }

    #[test]
    fn update_reports_up_to_date_when_nothing_new() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        add(&fx.root, &file_url(&fx.src)).expect("add should succeed");
        let preview = update_fetch(&installed_plugin(&fx)).expect("fetch should succeed");
        assert!(preview.up_to_date);
        assert_eq!(preview.current, preview.incoming);
    }

    #[test]
    fn update_refuses_a_history_rewrite_without_resetting() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        add(&fx.root, &file_url(&fx.src)).expect("add should succeed");

        // Rewrite upstream's root commit: no ancestry with the installed one.
        git_ok(
            &fx.src,
            &[
                "-c",
                "user.name=par-term-test",
                "-c",
                "user.email=test@par-term.test",
                "commit",
                "--quiet",
                "--amend",
                "-m",
                "rewritten history",
            ],
        );

        let dir = installed_plugin(&fx);
        let preview = update_fetch(&dir).expect("fetch still succeeds");
        assert!(!preview.up_to_date);
        let err = update_apply(&dir).expect_err("must refuse a non-fast-forward");
        assert!(
            err.contains("nothing was changed"),
            "unexpected error: {err}"
        );
        assert!(
            err.contains("remove and re-add"),
            "error should name the remedy: {err}"
        );
        // The installed plugin is untouched.
        assert!(dir.join("manifest.json").exists());
        let head = git_ok(&dir, &["rev-parse", "--short", "HEAD"]);
        assert_eq!(head, preview.current);
    }

    #[test]
    fn remove_deletes_a_git_installed_plugin() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        add(&fx.root, &file_url(&fx.src)).expect("add should succeed");
        let dir = installed_plugin(&fx);
        assert!(dir.is_dir());
        remove(&fx.root, "com.test.panel").expect("remove should succeed");
        assert!(!dir.exists());
        let (plugins, _) = discover_plugins(&fx.root);
        assert!(plugins.is_empty());
    }

    #[test]
    fn remove_refuses_a_hand_copied_directory() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        let dir = installed_plugin(&fx);
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("manifest.json"),
            PANEL_MANIFEST.replace("{id}", "com.test.panel"),
        )
        .expect("write manifest");
        write_exec_file(&dir.join("panel.sh"));

        let err = remove(&fx.root, "com.test.panel").expect_err("must refuse");
        assert!(
            err.contains("not installed by plugin add"),
            "unexpected error: {err}"
        );
        assert!(dir.is_dir(), "the hand-copied plugin must survive");
    }

    #[test]
    fn remove_refuses_a_bare_git_init_without_a_remote() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        let dir = installed_plugin(&fx);
        fs::create_dir_all(&dir).expect("create dir");
        git_ok(&dir, &["init", "--quiet"]);

        let err = remove(&fx.root, "com.test.panel").expect_err("must refuse");
        assert!(
            err.contains("no `origin` remote"),
            "unexpected error: {err}"
        );
        assert!(dir.is_dir(), "the local repo must survive");
    }

    #[test]
    fn remove_names_a_missing_plugin() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        let err = remove(&fx.root, "com.test.nope").expect_err("must refuse");
        assert!(
            err.contains("no plugin directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn is_git_installed_is_false_for_plain_directories() {
        let fx = fixture("com.test.panel", PANEL_MANIFEST);
        assert!(!is_git_installed(&fx.root));
        assert!(!is_git_installed(&fx.src)); // a repo, but not a plugin home
    }

    #[cfg(unix)]
    #[test]
    fn a_command_that_outlives_its_deadline_is_killed() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let err = output_with_timeout(&mut cmd, Duration::from_millis(500))
            .expect_err("sleep 30 must exceed a 500ms deadline");
        assert!(err.contains("timed out"), "unexpected error: {err}");
    }

    #[cfg(unix)]
    #[test]
    fn git_invocations_carry_no_terminal_prompt() {
        // A `!` alias makes git run our shell command with ITS environment,
        // so printing the variable proves what every plugin_git invocation
        // sees — the setting that keeps a private-repo URL from blocking the
        // GPU process on a credential prompt.
        let tmp = TempDir::new().expect("temp dir");
        let out = run_git_ok(
            Some(tmp.path()),
            &[
                "-c",
                r#"alias.prompt-check=!sh -c 'printf %s "$GIT_TERMINAL_PROMPT"'"#,
                "prompt-check",
            ],
            READ_TIMEOUT,
            "git alias",
        )
        .expect("alias should run");
        assert_eq!(out, "0");
    }
}
