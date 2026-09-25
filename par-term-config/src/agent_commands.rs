//! Agent-authored command files: load, validate, and persist the per-command
//! YAML files under `<config_dir>/commands/`.
//!
//! A command file wraps exactly one [`CustomActionConfig`] variant plus
//! provenance fields. The design (docs/plans/2026-09-24-agent-authored-commands-design.md,
//! approved 2026-09-24) keeps `id`/`title` inside `action` because every
//! variant embeds both as required fields; the wrapper carries no duplicate.
//!
//! * **script** commands are `shell_command` actions and require first-run
//!   confirmation keyed on a SHA-256 of the canonical action payload
//!   (the confirmation ledger, D4b).
//! * **macro** commands are any of the other five variants and need no
//!   confirmation.
//!
//! Provenance (D4c) lives in the file (`created_by`), not in the path: agents
//! may freely overwrite files they authored, while user-authored files are
//! refused by the MCP write path. This module enforces the invariants on
//! load and provides the write/delete primitives the MCP tools call; it does
//! not decide policy beyond `action.id == <filename stem>`, which is a
//! filesystem-safety invariant (no path traversal) rather than a trust rule.
//!
//! # Crate placement
//!
//! This lives in `par-term-config` because the format wraps
//! `CustomActionConfig` (defined in this crate) and both the root crate
//! (palette/dispatch/CLI) and `par-term-mcp` (the write path) must agree on
//! one definition without an edge through the root crate.

use crate::atomic_save::save_bytes_atomic;
use crate::snippets::CustomActionConfig;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum bytes of one command file accepted on the write path. Bounds
/// damage from a compromised agent (design, Security notes).
pub const MAX_COMMAND_FILE_BYTES: usize = 64 * 1024;

/// Maximum number of command files kept in the commands directory.
pub const MAX_COMMAND_COUNT: usize = 200;

/// Maximum length of a command id in bytes.
pub const MAX_COMMAND_ID_LEN: usize = 64;

/// The commands directory, `<config_dir>/commands/`.
pub fn commands_dir() -> PathBuf {
    crate::config::Config::config_dir().join("commands")
}

/// The confirmation ledger, `<config_dir>/commands/.confirmations.json`.
pub fn confirmations_path() -> PathBuf {
    commands_dir().join(".confirmations.json")
}

/// A valid command id: `[a-z0-9-]+`, 1..=64 bytes. The charset is deliberate —
/// it is also the set of characters safe as a filename stem on every platform
/// and unambiguous inside the `agent-cmd:<id>` keybinding prefix.
pub fn is_valid_command_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_COMMAND_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Who authored a command file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CommandAuthor {
    /// Written by an agent through the MCP tools (or hand-tagged as such).
    Agent,
    /// Written by the user by hand. The field being absent also means user —
    /// files predate the tag or were dropped in without one.
    #[default]
    User,
}

/// One command file on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCommandFile {
    /// Who authored this command. Absent deserializes as [`CommandAuthor::User`].
    #[serde(default)]
    pub created_by: CommandAuthor,

    /// The authoring agent, required when `created_by == agent`. Validated on
    /// load; ignored (and pointless) for user commands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent: Option<String>,

    /// Creation timestamp (RFC 3339). Informational only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,

    /// The wrapped action. Exactly one `CustomActionConfig` variant.
    pub action: CustomActionConfig,
}

impl AgentCommandFile {
    /// The command id (delegates to the embedded action's id).
    pub fn id(&self) -> &str {
        self.action.id()
    }

    /// The display title (delegates to the embedded action's title).
    pub fn title(&self) -> &str {
        self.action.title()
    }

    /// Whether this is a script command (a `shell_command` action) — the only
    /// kind subject to first-run confirmation (D4b).
    pub fn is_script(&self) -> bool {
        matches!(self.action, CustomActionConfig::ShellCommand { .. })
    }

    /// SHA-256 over the canonical serialization of the `action` payload.
    /// Any change to the action produces a new hash, resetting confirmation.
    pub fn body_hash(&self) -> String {
        let bytes = serde_json::to_vec(&self.action).unwrap_or_default();
        let digest = Sha256::digest(&bytes);
        let mut hex = String::with_capacity(digest.len() * 2);
        for b in digest {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        hex
    }

    /// Palette label per the design: `<title> · agent (<source_agent>)` for
    /// agent commands, plain `<title>` for user commands.
    pub fn palette_label(&self) -> String {
        match (self.created_by, &self.source_agent) {
            (CommandAuthor::Agent, Some(agent)) => {
                format!("{} · agent ({})", self.title(), agent)
            }
            _ => self.title().to_string(),
        }
    }
}

/// Load and validate one command file.
///
/// Validation: id charset/length, `action.id == <filename stem>`, and
/// `source_agent` present when `created_by == agent`. The stem equality check
/// doubles as the no-path-traversal guard for the whole feature — an id can
/// never name a file other than its own.
pub fn load_command_file(path: &Path) -> Result<AgentCommandFile> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("command file has no UTF-8 stem")?;

    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if bytes.len() > MAX_COMMAND_FILE_BYTES {
        bail!(
            "command file {} exceeds {} bytes",
            path.display(),
            MAX_COMMAND_FILE_BYTES
        );
    }

    let file: AgentCommandFile =
        serde_yaml_ng::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;

    validate_command(&file, stem)?;
    Ok(file)
}

/// Validate an in-memory command against its expected filename stem.
pub fn validate_command(file: &AgentCommandFile, expected_stem: &str) -> Result<()> {
    let id = file.id();
    if !is_valid_command_id(id) {
        bail!(
            "command id {:?} is invalid: 1-{} bytes of [a-z0-9-]",
            id,
            MAX_COMMAND_ID_LEN
        );
    }
    if id != expected_stem {
        bail!(
            "action id {:?} does not match filename stem {:?}",
            id,
            expected_stem
        );
    }
    if file.created_by == CommandAuthor::Agent
        && file.source_agent.as_deref().unwrap_or("").is_empty()
    {
        bail!("source_agent is required when created_by is agent");
    }
    if let Some(agent) = &file.source_agent
        && agent.trim().is_empty()
    {
        bail!("source_agent must not be blank");
    }
    Ok(())
}

/// A validated command plus its source path, as kept in the store's snapshot.
#[derive(Debug, Clone)]
pub struct LoadedCommand {
    pub file: AgentCommandFile,
    pub path: PathBuf,
}

/// Load every valid command in the directory. Invalid files are skipped with
/// a log line, never a dialog storm (design, Directory watching). Hidden
/// files (the ledger, editor droppings) and non-YAML files are ignored.
pub fn load_all_commands(dir: &Path) -> Vec<LoadedCommand> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            log::debug!(
                "commands dir {} unreadable ({e}) — treating as empty",
                dir.display()
            );
            return out;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !name.ends_with(".yaml") {
            continue;
        }
        match load_command_file(&path) {
            Ok(file) => out.push(LoadedCommand { file, path }),
            Err(e) => log::warn!("skipping invalid command file {}: {e:#}", path.display()),
        }
    }
    out.sort_by(|a, b| a.file.id().cmp(b.file.id()));
    out
}

/// Serialize a command file to YAML bytes.
pub fn command_to_yaml(file: &AgentCommandFile) -> Result<Vec<u8>> {
    let s = serde_yaml_ng::to_string(file).context("serializing command file")?;
    let mut bytes = s.into_bytes();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    Ok(bytes)
}

/// Write one command file atomically. Refuses to overwrite a user-authored
/// command (D4c) and enforces the directory file-count cap.
pub fn save_command_file(file: &AgentCommandFile, dir: &Path) -> Result<PathBuf> {
    let id = file.id();
    validate_command(file, id)?;

    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;

    let path = dir.join(format!("{id}.yaml"));
    if path.exists() {
        let existing = load_command_file(&path)?;
        if existing.created_by == CommandAuthor::User {
            bail!(
                "refusing to overwrite user-authored command {id:?} — the user must \
                 approve or edit it manually"
            );
        }
    } else if load_all_commands(dir).len() >= MAX_COMMAND_COUNT {
        bail!(
            "commands directory is full ({} files); delete commands before adding more",
            MAX_COMMAND_COUNT
        );
    }

    let bytes = command_to_yaml(file)?;
    if bytes.len() > MAX_COMMAND_FILE_BYTES {
        bail!(
            "serialized command is {} bytes; cap is {}",
            bytes.len(),
            MAX_COMMAND_FILE_BYTES
        );
    }
    save_bytes_atomic(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// Delete one command file. Refuses to delete a user-authored command (D4c).
/// Ledger entries for the id are pruned by the caller.
pub fn delete_command_file(id: &str, dir: &Path) -> Result<()> {
    if !is_valid_command_id(id) {
        bail!("invalid command id {id:?}");
    }
    let path = dir.join(format!("{id}.yaml"));
    if !path.exists() {
        bail!("no such command: {id}");
    }
    let existing = load_command_file(&path)?;
    if existing.created_by == CommandAuthor::User {
        bail!(
            "refusing to delete user-authored command {id:?} — the user must \
             approve or delete it manually"
        );
    }
    fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
    Ok(())
}

/// Delete one command file with the user's authority — the Settings UI path.
/// Unlike [`delete_command_file`] (the MCP path) this removes user-authored
/// files too: provenance gates what an *agent* may do, not the user. The
/// id's ledger entry is pruned here as well, so a later command recreated
/// under the same id with the same body asks for confirmation again.
pub fn delete_command_file_as_user(id: &str, dir: &Path) -> Result<()> {
    if !is_valid_command_id(id) {
        bail!("invalid command id {id:?}");
    }
    let path = dir.join(format!("{id}.yaml"));
    if !path.exists() {
        bail!("no such command: {id}");
    }
    fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
    let mut ledger = load_confirmation_ledger(dir);
    if ledger.remove(id).is_some() {
        save_confirmation_ledger(&ledger, dir)?;
    }
    Ok(())
}

/// The confirmation ledger: command id → approved body hash.
pub type ConfirmationLedger = BTreeMap<String, String>;

/// Load the confirmation ledger. A missing or corrupt file is an empty
/// ledger — worst case a script asks for confirmation again.
pub fn load_confirmation_ledger(dir: &Path) -> ConfirmationLedger {
    let path = dir.join(".confirmations.json");
    let Ok(bytes) = fs::read(&path) else {
        return BTreeMap::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        log::warn!(
            "confirmation ledger {} unreadable ({e}); starting empty — \
             scripts will re-ask for confirmation",
            path.display()
        );
        BTreeMap::new()
    })
}

/// Persist the confirmation ledger atomically.
pub fn save_confirmation_ledger(ledger: &ConfirmationLedger, dir: &Path) -> Result<()> {
    let path = dir.join(".confirmations.json");
    let bytes = serde_json::to_vec_pretty(ledger).context("serializing confirmation ledger")?;
    save_bytes_atomic(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script_file(id: &str) -> AgentCommandFile {
        let yaml = format!(
            "created_by: agent\nsource_agent: claude-code\naction:\n  type: shell_command\n  \
             id: {id}\n  title: Test\n  command: echo\n  args: [\"hi\"]\n"
        );
        serde_yaml_ng::from_str(&yaml).unwrap()
    }

    fn macro_file(id: &str) -> AgentCommandFile {
        let yaml = format!(
            "created_by: user\naction:\n  type: insert_text\n  id: {id}\n  title: Greet\n  \
             text: hello\n"
        );
        serde_yaml_ng::from_str(&yaml).unwrap()
    }

    #[test]
    fn roundtrip_script_file() {
        let f = script_file("deploy-staging");
        assert_eq!(f.id(), "deploy-staging");
        assert!(f.is_script());
        assert!(f.source_agent.as_deref() == Some("claude-code"));
        let bytes = command_to_yaml(&f).unwrap();
        let back: AgentCommandFile = serde_yaml_ng::from_slice(&bytes).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn macro_is_not_script_and_needs_no_source() {
        let f = macro_file("greet");
        assert_eq!(f.created_by, CommandAuthor::User);
        assert!(!f.is_script());
        assert!(validate_command(&f, "greet").is_ok());
    }

    #[test]
    fn id_charset_rejected() {
        assert!(is_valid_command_id("deploy-staging"));
        assert!(is_valid_command_id("a"));
        assert!(!is_valid_command_id(""));
        assert!(!is_valid_command_id("Deploy"));
        assert!(!is_valid_command_id("has space"));
        assert!(!is_valid_command_id("has/slash"));
        assert!(!is_valid_command_id("caf\u{e9}"));
        let long = "a".repeat(MAX_COMMAND_ID_LEN + 1);
        assert!(!is_valid_command_id(&long));
    }

    #[test]
    fn validate_rejects_stem_mismatch() {
        let f = script_file("one-id");
        let err = validate_command(&f, "other-id").unwrap_err();
        assert!(err.to_string().contains("does not match"));
    }

    #[test]
    fn validate_rejects_agent_without_source() {
        let mut f = script_file("x");
        f.source_agent = None;
        let err = validate_command(&f, "x").unwrap_err();
        assert!(err.to_string().contains("source_agent"));
    }

    #[test]
    fn body_hash_changes_with_action() {
        let mut f = script_file("x");
        let h1 = f.body_hash();
        if let CustomActionConfig::ShellCommand { title, .. } = &mut f.action {
            *title = "Changed".to_string();
        }
        assert_ne!(h1, f.body_hash());
    }

    #[test]
    fn save_refuses_user_overwrite_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let user = macro_file("mine");
        save_command_file(&user, dir.path()).unwrap();

        // Overwrite attempt with an agent-tagged body of the same id.
        let evil = script_file("mine");
        let err = save_command_file(&evil, dir.path()).unwrap_err();
        assert!(err.to_string().contains("user-authored"));

        let err = delete_command_file("mine", dir.path()).unwrap_err();
        assert!(err.to_string().contains("user-authored"));
    }

    #[test]
    fn user_delete_removes_user_file_and_prunes_ledger() {
        let dir = tempfile::tempdir().unwrap();
        save_command_file(&macro_file("mine"), dir.path()).unwrap();
        save_command_file(&script_file("theirs"), dir.path()).unwrap();
        let mut ledger = ConfirmationLedger::new();
        ledger.insert("theirs".to_string(), "hash".to_string());
        ledger.insert("other".to_string(), "hash-o".to_string());
        save_confirmation_ledger(&ledger, dir.path()).unwrap();

        // The MCP path still refuses the user-authored file.
        assert!(delete_command_file("mine", dir.path()).is_err());
        delete_command_file_as_user("mine", dir.path()).unwrap();
        assert!(!dir.path().join("mine.yaml").exists());

        delete_command_file_as_user("theirs", dir.path()).unwrap();
        assert!(load_all_commands(dir.path()).is_empty());
        let after = load_confirmation_ledger(dir.path());
        assert!(!after.contains_key("theirs"));
        assert_eq!(after.get("other").map(String::as_str), Some("hash-o"));

        assert!(delete_command_file_as_user("mine", dir.path()).is_err());
        assert!(delete_command_file_as_user("../escape", dir.path()).is_err());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let f = script_file("round-trip");
        let path = save_command_file(&f, dir.path()).unwrap();
        assert!(path.ends_with("round-trip.yaml"));

        let all = load_all_commands(dir.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].file, f);
    }

    #[test]
    fn load_all_skips_invalid_files_quietly() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("good.yaml"), "").unwrap();
        // Invalid: id doesn't match stem.
        std::fs::write(
            dir.path().join("bad.yaml"),
            "created_by: user\naction:\n  type: insert_text\n  id: wrong\n  title: T\n  \
             text: x\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(".confirmations.json"), "{}").unwrap();

        let all = load_all_commands(dir.path());
        // good.yaml is empty -> parse error -> skipped; bad.yaml -> mismatch -> skipped.
        assert!(all.is_empty());
    }

    #[test]
    fn ledger_roundtrip_and_corrupt_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = ConfirmationLedger::new();
        ledger.insert("a".to_string(), "hash-a".to_string());
        save_confirmation_ledger(&ledger, dir.path()).unwrap();
        assert_eq!(load_confirmation_ledger(dir.path()), ledger);

        std::fs::write(dir.path().join(".confirmations.json"), "not json").unwrap();
        assert!(load_confirmation_ledger(dir.path()).is_empty());
    }

    #[test]
    fn palette_label_includes_agent_provenance() {
        let f = script_file("x");
        assert_eq!(f.palette_label(), "Test · agent (claude-code)");
        let u = macro_file("greet");
        assert_eq!(u.palette_label(), "Greet");
    }

    #[test]
    fn command_count_cap_enforced() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..MAX_COMMAND_COUNT {
            let f = macro_file(&format!("cmd-{i:03}"));
            save_command_file(&f, dir.path()).unwrap();
        }
        let extra = macro_file("cmd-extra");
        let err = save_command_file(&extra, dir.path()).unwrap_err();
        assert!(err.to_string().contains("full"));
    }
}
