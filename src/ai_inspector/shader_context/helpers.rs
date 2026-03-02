//! Shader-detection helpers and file-system utilities.
//!
//! Contains keyword-based detection for shader-related messages,
//! shader directory scanning, and filename classification.

use std::path::Path;

use crate::config::Config;

/// Shader-related keywords that trigger context injection (all lowercase).
pub(super) const SHADER_KEYWORDS: &[&str] = &[
    "shader",
    "glsl",
    "wgsl",
    "effect",
    "crt",
    "scanline",
    "post-process",
    "postprocess",
    "fragment",
    "mainimage",
    "ichannel",
    "itime",
    "iresolution",
    "shadertoy",
    "transpile",
    "naga",
    "cursor effect",
    "cursor shader",
    "background effect",
    "background shader",
];

/// Returns `true` if shader context should be injected for the given message.
///
/// Context is injected when the message contains any shader-related keyword
/// (case-insensitive) **or** when a custom background/cursor shader is
/// currently enabled in the config.
pub fn should_inject_shader_context(message: &str, config: &Config) -> bool {
    // Check if any shader is currently enabled
    if config.shader.custom_shader_enabled && config.shader.custom_shader.is_some() {
        return true;
    }
    if config.shader.cursor_shader_enabled && config.shader.cursor_shader.is_some() {
        return true;
    }

    // Check for keyword matches (case-insensitive)
    let lower = message.to_lowercase();
    SHADER_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Returns `true` when the user message appears to request that a shader be
/// activated / set as current (not just discussed or edited).
pub fn is_shader_activation_request(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let mentions_shader = lower.contains("shader");
    if !mentions_shader {
        return false;
    }

    lower.contains("set shader")
        || lower.contains("active shader")
        || lower.contains("activate shader")
        || lower.contains("set that shader")
        || lower.contains("set the shader")
        || lower.contains("set as the active shader")
        || lower.contains("set as current")
        || lower.contains("current shader")
}

/// Classify shader filenames into background and cursor categories.
///
/// Cursor shaders are identified by filenames starting with `"cursor_"`.
/// Returns `(background_shaders, cursor_shaders)`.
pub(super) fn classify_shaders(shaders: &[String]) -> (Vec<&str>, Vec<&str>) {
    let mut background: Vec<&str> = Vec::new();
    let mut cursor: Vec<&str> = Vec::new();

    for name in shaders {
        if name.starts_with("cursor_") {
            cursor.push(name.as_str());
        } else {
            background.push(name.as_str());
        }
    }

    (background, cursor)
}

/// Scan a directory for shader files (`.glsl`, `.frag`, `.shader`).
///
/// Returns a sorted list of filenames (not full paths). If the directory
/// does not exist or cannot be read, returns an empty `Vec`.
pub(super) fn scan_shaders(shaders_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(shaders_dir) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let ext = path.extension()?.to_str()?;
            if matches!(ext, "glsl" | "frag" | "shader") {
                Some(entry.file_name().to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();

    names.sort();
    names
}
