//! Shader context generation for AI Inspector agent prompts.
//!
//! When users discuss shaders with the ACP agent, this module detects
//! shader-related keywords and builds a rich context block describing the
//! current shader state, available shaders, debug paths, uniforms, and
//! a minimal template so the agent can assist with shader creation, editing,
//! debugging, and management.

use std::path::Path;

use crate::config::Config;

/// Shader-related keywords that trigger context injection (all lowercase).
const SHADER_KEYWORDS: &[&str] = &[
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
    if config.custom_shader_enabled && config.custom_shader.is_some() {
        return true;
    }
    if config.cursor_shader_enabled && config.cursor_shader.is_some() {
        return true;
    }

    // Check for keyword matches (case-insensitive)
    let lower = message.to_lowercase();
    SHADER_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Classify shader filenames into background and cursor categories.
///
/// Cursor shaders are identified by filenames starting with `"cursor_"`.
/// Returns `(background_shaders, cursor_shaders)`.
fn classify_shaders(shaders: &[String]) -> (Vec<&str>, Vec<&str>) {
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
fn scan_shaders(shaders_dir: &Path) -> Vec<String> {
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

/// Build a shader context block to inject into agent prompts.
///
/// The returned string contains sections on the current shader state,
/// available shaders, debug file paths, available uniforms, a minimal
/// shader template, and instructions for applying changes.
pub fn build_shader_context(config: &Config) -> String {
    let shaders_dir = Config::shaders_dir();
    let config_path = Config::config_path();

    let available = scan_shaders(&shaders_dir);
    let (bg_shaders, cursor_shaders) = classify_shaders(&available);

    let mut ctx = String::with_capacity(2048);

    ctx.push_str("[Shader Assistant Context]\n\n");

    // ---- Current Shader State ----
    ctx.push_str("## Current Shader State\n");

    // Background shader
    if let Some(ref name) = config.custom_shader {
        ctx.push_str(&format!("- Background shader: `{name}`"));
        if config.custom_shader_enabled {
            ctx.push_str(" (enabled)\n");
        } else {
            ctx.push_str(" (disabled)\n");
        }
        if config.custom_shader_enabled {
            ctx.push_str(&format!(
                "  - animation_speed: {}\n",
                config.custom_shader_animation_speed
            ));
            ctx.push_str(&format!(
                "  - brightness: {}\n",
                config.custom_shader_brightness
            ));
            ctx.push_str(&format!(
                "  - text_opacity: {}\n",
                config.custom_shader_text_opacity
            ));
        }
    } else {
        ctx.push_str("- Background shader: none\n");
    }

    // Cursor shader
    if let Some(ref name) = config.cursor_shader {
        ctx.push_str(&format!("- Cursor shader: `{name}`"));
        if config.cursor_shader_enabled {
            ctx.push_str(" (enabled)\n");
        } else {
            ctx.push_str(" (disabled)\n");
        }
        if config.cursor_shader_enabled {
            ctx.push_str(&format!(
                "  - animation_speed: {}\n",
                config.cursor_shader_animation_speed
            ));
            ctx.push_str(&format!(
                "  - glow_radius: {}\n",
                config.cursor_shader_glow_radius
            ));
            ctx.push_str(&format!(
                "  - glow_intensity: {}\n",
                config.cursor_shader_glow_intensity
            ));
        }
    } else {
        ctx.push_str("- Cursor shader: none\n");
    }

    ctx.push('\n');

    // ---- Available Shaders ----
    ctx.push_str("## Available Shaders\n");

    if bg_shaders.is_empty() && cursor_shaders.is_empty() {
        ctx.push_str("No shaders found in the shaders directory.\n");
    } else {
        if !bg_shaders.is_empty() {
            ctx.push_str("Background shaders:\n");
            for s in &bg_shaders {
                ctx.push_str(&format!("  - {s}\n"));
            }
        }
        if !cursor_shaders.is_empty() {
            ctx.push_str("Cursor shaders:\n");
            for s in &cursor_shaders {
                ctx.push_str(&format!("  - {s}\n"));
            }
        }
    }

    ctx.push('\n');

    // ---- Debug Files ----
    ctx.push_str("## Debug Files\n");

    if let Some(ref name) = config.custom_shader {
        let stem = Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        ctx.push_str(&format!(
            "- Transpiled WGSL: `/tmp/par_term_{stem}_shader.wgsl`\n"
        ));
    }
    if let Some(ref name) = config.cursor_shader {
        let stem = Path::new(name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        ctx.push_str(&format!(
            "- Cursor transpiled WGSL: `/tmp/par_term_{stem}_shader.wgsl`\n"
        ));
    }
    ctx.push_str("- Wrapped GLSL (last shader): `/tmp/par_term_debug_wrapped.glsl`\n");

    ctx.push('\n');

    // ---- Available Uniforms ----
    ctx.push_str("## Available Uniforms\n");
    ctx.push_str("Common (all shaders):\n");
    ctx.push_str("  - `iTime` (float) - elapsed time in seconds\n");
    ctx.push_str("  - `iResolution` (vec3) - viewport resolution in pixels (xy) and aspect ratio (z)\n");
    ctx.push_str(
        "  - `iMouse` (vec4) - mouse position: xy=current, zw=click (Shadertoy-compatible)\n",
    );
    ctx.push_str("  - `iChannel0`..`iChannel3` (sampler2D) - user texture channels\n");
    ctx.push_str("  - `iChannel4` (sampler2D) - terminal content texture (par-term specific)\n");
    ctx.push_str("Cursor shader extras:\n");
    ctx.push_str("  - `iCurrentCursor` (vec2) - current cursor position in pixels\n");
    ctx.push_str("  - `iPreviousCursor` (vec2) - previous cursor position in pixels\n");
    ctx.push_str("  - `iTimeCursorChange` (float) - time since last cursor move\n");

    ctx.push('\n');

    // ---- Minimal Shader Template ----
    ctx.push_str("## Minimal Shader Template\n");
    ctx.push_str("```glsl\n");
    ctx.push_str("void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n");
    ctx.push_str("    vec2 uv = fragCoord / iResolution.xy;\n");
    ctx.push_str("    vec4 tex = texture(iChannel4, uv);\n");
    ctx.push_str("    fragColor = tex;\n");
    ctx.push_str("}\n");
    ctx.push_str("```\n");

    ctx.push('\n');

    // ---- How to Apply Changes ----
    ctx.push_str("## How to Apply Changes\n");
    ctx.push_str(&format!(
        "1. Write shader GLSL files to: `{}`\n",
        shaders_dir.display()
    ));
    ctx.push_str(&format!(
        "2. Edit `{}` to set `custom_shader` or `cursor_shader` to the filename\n",
        config_path.display()
    ));
    ctx.push_str("3. Set `custom_shader_enabled: true` or `cursor_shader_enabled: true`\n");
    ctx.push_str(
        "4. par-term watches shader files and hot-reloads on save - no restart required\n",
    );

    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a default config with no active shaders.
    fn default_config() -> Config {
        Config::default()
    }

    // ---- Keyword detection ----

    #[test]
    fn test_keyword_shader_lowercase() {
        let config = default_config();
        assert!(should_inject_shader_context("help me write a shader", &config));
    }

    #[test]
    fn test_keyword_shader_uppercase() {
        let config = default_config();
        assert!(should_inject_shader_context("I want a SHADER effect", &config));
    }

    #[test]
    fn test_keyword_shader_mixed_case() {
        let config = default_config();
        assert!(should_inject_shader_context("Create a Shader for CRT", &config));
    }

    #[test]
    fn test_keyword_glsl() {
        let config = default_config();
        assert!(should_inject_shader_context("write some GLSL code", &config));
    }

    #[test]
    fn test_keyword_wgsl() {
        let config = default_config();
        assert!(should_inject_shader_context("what does the wgsl look like", &config));
    }

    #[test]
    fn test_keyword_crt() {
        let config = default_config();
        assert!(should_inject_shader_context("I want a crt effect", &config));
    }

    #[test]
    fn test_keyword_scanline() {
        let config = default_config();
        assert!(should_inject_shader_context("add scanline overlay", &config));
    }

    #[test]
    fn test_keyword_postprocess_hyphenated() {
        let config = default_config();
        assert!(should_inject_shader_context("post-process my terminal", &config));
    }

    #[test]
    fn test_keyword_postprocess_concatenated() {
        let config = default_config();
        assert!(should_inject_shader_context("add a postprocess effect", &config));
    }

    #[test]
    fn test_keyword_fragment() {
        let config = default_config();
        assert!(should_inject_shader_context("fragment shader", &config));
    }

    #[test]
    fn test_keyword_mainimage() {
        let config = default_config();
        assert!(should_inject_shader_context("implement mainImage", &config));
    }

    #[test]
    fn test_keyword_ichannel() {
        let config = default_config();
        assert!(should_inject_shader_context("how to use iChannel0", &config));
    }

    #[test]
    fn test_keyword_itime() {
        let config = default_config();
        assert!(should_inject_shader_context("animate with iTime", &config));
    }

    #[test]
    fn test_keyword_iresolution() {
        let config = default_config();
        assert!(should_inject_shader_context("normalize by iResolution", &config));
    }

    #[test]
    fn test_keyword_shadertoy() {
        let config = default_config();
        assert!(should_inject_shader_context("port this Shadertoy shader", &config));
    }

    #[test]
    fn test_keyword_transpile() {
        let config = default_config();
        assert!(should_inject_shader_context("transpile my glsl", &config));
    }

    #[test]
    fn test_keyword_naga() {
        let config = default_config();
        assert!(should_inject_shader_context("naga transpiler error", &config));
    }

    #[test]
    fn test_keyword_cursor_effect() {
        let config = default_config();
        assert!(should_inject_shader_context("add a cursor effect", &config));
    }

    #[test]
    fn test_keyword_cursor_shader() {
        let config = default_config();
        assert!(should_inject_shader_context("edit cursor shader", &config));
    }

    #[test]
    fn test_keyword_background_effect() {
        let config = default_config();
        assert!(should_inject_shader_context("cool background effect", &config));
    }

    #[test]
    fn test_keyword_background_shader() {
        let config = default_config();
        assert!(should_inject_shader_context("tweak my background shader", &config));
    }

    #[test]
    fn test_keyword_effect() {
        let config = default_config();
        assert!(should_inject_shader_context("what visual effect can I add?", &config));
    }

    // ---- Negative keyword cases ----

    #[test]
    fn test_no_keywords_general_question() {
        let config = default_config();
        assert!(!should_inject_shader_context("how do I list files?", &config));
    }

    #[test]
    fn test_no_keywords_empty_message() {
        let config = default_config();
        assert!(!should_inject_shader_context("", &config));
    }

    #[test]
    fn test_no_keywords_unrelated() {
        let config = default_config();
        assert!(!should_inject_shader_context(
            "change font size to 14",
            &config
        ));
    }

    // ---- Active shader triggering ----

    #[test]
    fn test_active_background_shader_triggers() {
        let mut config = default_config();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = true;
        // Should trigger even without shader keywords
        assert!(should_inject_shader_context("hello world", &config));
    }

    #[test]
    fn test_disabled_background_shader_no_trigger() {
        let mut config = default_config();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = false;
        // Shader is set but disabled, no keywords -> no trigger
        assert!(!should_inject_shader_context("hello world", &config));
    }

    #[test]
    fn test_enabled_but_no_shader_file_no_trigger() {
        let mut config = default_config();
        config.custom_shader = None;
        config.custom_shader_enabled = true;
        // Enabled but no shader file set -> no trigger
        assert!(!should_inject_shader_context("hello world", &config));
    }

    // ---- Cursor shader triggering ----

    #[test]
    fn test_active_cursor_shader_triggers() {
        let mut config = default_config();
        config.cursor_shader = Some("cursor_glow.glsl".to_string());
        config.cursor_shader_enabled = true;
        assert!(should_inject_shader_context("hello world", &config));
    }

    #[test]
    fn test_disabled_cursor_shader_no_trigger() {
        let mut config = default_config();
        config.cursor_shader = Some("cursor_glow.glsl".to_string());
        config.cursor_shader_enabled = false;
        assert!(!should_inject_shader_context("hello world", &config));
    }

    // ---- classify_shaders ----

    #[test]
    fn test_classify_empty() {
        let shaders: Vec<String> = vec![];
        let (bg, cur) = classify_shaders(&shaders);
        assert!(bg.is_empty());
        assert!(cur.is_empty());
    }

    #[test]
    fn test_classify_mixed() {
        let shaders: Vec<String> = vec![
            "crt.glsl".to_string(),
            "cursor_glow.glsl".to_string(),
            "matrix.glsl".to_string(),
            "cursor_trail.frag".to_string(),
        ];
        let (bg, cur) = classify_shaders(&shaders);
        assert_eq!(bg, vec!["crt.glsl", "matrix.glsl"]);
        assert_eq!(cur, vec!["cursor_glow.glsl", "cursor_trail.frag"]);
    }

    #[test]
    fn test_classify_all_background() {
        let shaders: Vec<String> = vec!["crt.glsl".to_string(), "rain.glsl".to_string()];
        let (bg, cur) = classify_shaders(&shaders);
        assert_eq!(bg.len(), 2);
        assert!(cur.is_empty());
    }

    #[test]
    fn test_classify_all_cursor() {
        let shaders: Vec<String> = vec![
            "cursor_glow.glsl".to_string(),
            "cursor_trail.glsl".to_string(),
        ];
        let (bg, cur) = classify_shaders(&shaders);
        assert!(bg.is_empty());
        assert_eq!(cur.len(), 2);
    }

    // ---- scan_shaders ----

    #[test]
    fn test_scan_shaders_nonexistent_dir() {
        let result = scan_shaders(Path::new("/tmp/par_term_test_nonexistent_dir_xyz"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_shaders_with_files() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let dir_path = dir.path();

        // Create test shader files
        std::fs::write(dir_path.join("crt.glsl"), "void mainImage() {}").unwrap();
        std::fs::write(dir_path.join("rain.frag"), "void mainImage() {}").unwrap();
        std::fs::write(dir_path.join("glow.shader"), "void mainImage() {}").unwrap();
        // Non-shader files should be excluded
        std::fs::write(dir_path.join("readme.txt"), "not a shader").unwrap();
        std::fs::write(dir_path.join("notes.md"), "notes").unwrap();

        let result = scan_shaders(dir_path);
        assert_eq!(result, vec!["crt.glsl", "glow.shader", "rain.frag"]);
    }

    #[test]
    fn test_scan_shaders_ignores_directories() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let dir_path = dir.path();

        std::fs::write(dir_path.join("effect.glsl"), "void mainImage() {}").unwrap();
        std::fs::create_dir(dir_path.join("subdir.glsl")).unwrap();

        let result = scan_shaders(dir_path);
        assert_eq!(result, vec!["effect.glsl"]);
    }

    // ---- Context builder output sections ----

    #[test]
    fn test_context_contains_header() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("[Shader Assistant Context]"));
    }

    #[test]
    fn test_context_contains_all_sections() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("## Current Shader State"));
        assert!(ctx.contains("## Available Shaders"));
        assert!(ctx.contains("## Debug Files"));
        assert!(ctx.contains("## Available Uniforms"));
        assert!(ctx.contains("## Minimal Shader Template"));
        assert!(ctx.contains("## How to Apply Changes"));
    }

    #[test]
    fn test_context_no_active_shaders() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("Background shader: none"));
        assert!(ctx.contains("Cursor shader: none"));
    }

    #[test]
    fn test_context_with_active_background_shader() {
        let mut config = default_config();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = true;
        config.custom_shader_animation_speed = 1.5;
        config.custom_shader_brightness = 0.8;
        config.custom_shader_text_opacity = 0.9;

        let ctx = build_shader_context(&config);
        assert!(ctx.contains("Background shader: `crt.glsl` (enabled)"));
        assert!(ctx.contains("animation_speed: 1.5"));
        assert!(ctx.contains("brightness: 0.8"));
        assert!(ctx.contains("text_opacity: 0.9"));
    }

    #[test]
    fn test_context_with_disabled_background_shader() {
        let mut config = default_config();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = false;

        let ctx = build_shader_context(&config);
        assert!(ctx.contains("Background shader: `crt.glsl` (disabled)"));
        // Parameters should NOT be listed when disabled
        assert!(!ctx.contains("animation_speed:"));
    }

    #[test]
    fn test_context_with_active_cursor_shader() {
        let mut config = default_config();
        config.cursor_shader = Some("cursor_glow.glsl".to_string());
        config.cursor_shader_enabled = true;
        config.cursor_shader_animation_speed = 2.0;
        config.cursor_shader_glow_radius = 100.0;
        config.cursor_shader_glow_intensity = 0.5;

        let ctx = build_shader_context(&config);
        assert!(ctx.contains("Cursor shader: `cursor_glow.glsl` (enabled)"));
        assert!(ctx.contains("animation_speed: 2"));
        assert!(ctx.contains("glow_radius: 100"));
        assert!(ctx.contains("glow_intensity: 0.5"));
    }

    #[test]
    fn test_context_debug_paths_with_shaders() {
        let mut config = default_config();
        config.custom_shader = Some("crt.glsl".to_string());
        config.cursor_shader = Some("cursor_glow.glsl".to_string());

        let ctx = build_shader_context(&config);
        assert!(ctx.contains("/tmp/par_term_crt_shader.wgsl"));
        assert!(ctx.contains("/tmp/par_term_cursor_glow_shader.wgsl"));
        assert!(ctx.contains("/tmp/par_term_debug_wrapped.glsl"));
    }

    #[test]
    fn test_context_uniforms_section() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("iTime"));
        assert!(ctx.contains("iResolution"));
        assert!(ctx.contains("iMouse"));
        assert!(ctx.contains("iChannel0"));
        assert!(ctx.contains("iChannel4"));
        assert!(ctx.contains("iCurrentCursor"));
        assert!(ctx.contains("iPreviousCursor"));
        assert!(ctx.contains("iTimeCursorChange"));
    }

    #[test]
    fn test_context_template_section() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("void mainImage(out vec4 fragColor, in vec2 fragCoord)"));
        assert!(ctx.contains("iResolution.xy"));
        assert!(ctx.contains("iChannel4"));
    }

    #[test]
    fn test_context_how_to_apply_section() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("custom_shader_enabled: true"));
        assert!(ctx.contains("cursor_shader_enabled: true"));
        assert!(ctx.contains("hot-reloads"));
    }
}
