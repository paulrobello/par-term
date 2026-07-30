//! Canonical locations of the shader debug dumps.
//!
//! Two components write these files — the transpiler dumps the wrapped GLSL it
//! feeds to naga, and the custom-shader renderer dumps the WGSL naga produces —
//! and several more report the paths to the user through shader diagnostics and
//! the AI inspector. They used to disagree: the renderer hardcoded `/tmp` on
//! Unix while the transpiler used the system temp directory, so on macOS the
//! reported wrapped-GLSL path pointed somewhere the file had never been
//! written. Everything now resolves through this module.

use std::path::{Path, PathBuf};

/// File name of the wrapped-GLSL dump written by the transpiler in debug builds.
pub const WRAPPED_GLSL_FILE_NAME: &str = "par_term_debug_wrapped.glsl";

/// Directory the shader debug dumps are written to.
///
/// This is [`std::env::temp_dir`]: `$TMPDIR` where set, a per-user directory on
/// macOS, `%TEMP%` on Windows. Deliberately not a hardcoded `/tmp` — that is not
/// even an absolute path on Windows, and a predictable name in a world-writable
/// directory is a symlink hazard on multi-user systems, which is why the
/// transpiler dump is written `0600`.
pub fn debug_dump_dir() -> PathBuf {
    std::env::temp_dir()
}

/// Path of the transpiled-WGSL dump for `shader_name`.
///
/// `shader_name` may be a bare name or a path; only the file stem is used, so
/// `crt`, `crt.glsl` and `~/.config/par-term/shaders/crt.glsl` all resolve to
/// the same dump.
pub fn transpiled_wgsl_path(shader_name: &str) -> PathBuf {
    let stem = Path::new(shader_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(shader_name);
    debug_dump_dir().join(format!("par_term_{stem}_shader.wgsl"))
}

/// Path of the wrapped-GLSL dump the transpiler writes in debug builds.
pub fn wrapped_glsl_path() -> PathBuf {
    debug_dump_dir().join(WRAPPED_GLSL_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_land_in_the_system_temp_dir() {
        assert!(transpiled_wgsl_path("crt").starts_with(std::env::temp_dir()));
        assert!(wrapped_glsl_path().starts_with(std::env::temp_dir()));
    }

    #[test]
    fn shader_name_is_reduced_to_its_stem() {
        let expected = debug_dump_dir().join("par_term_crt_shader.wgsl");
        assert_eq!(transpiled_wgsl_path("crt"), expected);
        assert_eq!(transpiled_wgsl_path("crt.glsl"), expected);
        assert_eq!(
            transpiled_wgsl_path("/home/u/.config/par-term/shaders/crt.glsl"),
            expected
        );
    }

    /// The renderer and the transpiler must agree, and both must agree with
    /// what shader diagnostics reports — the drift this module exists to stop.
    #[test]
    fn both_dump_kinds_share_one_directory() {
        assert_eq!(
            transpiled_wgsl_path("crt").parent(),
            wrapped_glsl_path().parent()
        );
    }
}
