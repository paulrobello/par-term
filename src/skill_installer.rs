//! par-mux agent skill installer.
//!
//! The skill ships embedded in the binary via `include_str!`, so every package
//! (`.app` bundle, release archive, `cargo run`) carries it; installing copies
//! the embedded copy into the Claude Code skills directory (`~/.claude/skills/par-mux/`).
//! Mirrors the role `shell_integration_installer` and `shader_installer` play for
//! the other welcome-dialog integrations.

use std::io;
use std::path::PathBuf;

/// The bundled par-mux agent skill, embedded at compile time.
pub const PAR_MUX_SKILL_MD: &str = include_str!("../skills/par-mux/SKILL.md");

/// The user's Claude Code skills directory (`~/.claude/skills`).
pub fn claude_skills_dir() -> io::Result<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".claude").join("skills"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory not found"))
}

/// Directory the par-mux agent skill installs under.
pub fn par_mux_skill_dir() -> io::Result<PathBuf> {
    Ok(claude_skills_dir()?.join("par-mux"))
}

/// Install (or refresh) the bundled par-mux agent skill.
///
/// Always overwrites: the embedded copy is the source of truth, so an install
/// after a par-term upgrade refreshes the skill to the shipping version.
pub fn install_agent_skill() -> io::Result<PathBuf> {
    let dir = par_mux_skill_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("SKILL.md");
    std::fs::write(&path, PAR_MUX_SKILL_MD)?;
    log::info!("Installed par-mux agent skill to {}", path.display());
    Ok(path)
}
