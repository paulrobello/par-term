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

    let available = scan_shaders(&shaders_dir);
    let (bg_shaders, cursor_shaders) = classify_shaders(&available);

    let mut ctx = String::with_capacity(2048);

    ctx.push_str("[Shader Assistant Context]\n");
    ctx.push_str(
        "Use this block as reference context. Treat [Observation] sections as state,\n\
         [Constraint] sections as hard rules, and [Instruction] sections as guidance.\n\n",
    );

    // ---- Current Shader State ----
    ctx.push_str("## [Observation] Current Shader State\n");

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
    ctx.push_str("## [Observation] Available Shaders\n");

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
    ctx.push_str("## [Observation] Debug Files\n");

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
    ctx.push_str("## [Observation] Available Uniforms\n");
    ctx.push_str("Common (all shaders):\n");
    ctx.push_str("  - `iTime` (float) - elapsed time in seconds\n");
    ctx.push_str(
        "  - `iResolution` (vec3) - viewport resolution in pixels (xy) and aspect ratio (z)\n",
    );
    ctx.push_str(
        "  - `iMouse` (vec4) - mouse position: xy=current, zw=click (Shadertoy-compatible)\n",
    );
    ctx.push_str("  - `iChannel0`..`iChannel3` (sampler2D) - user texture channels\n");
    ctx.push_str("  - `iChannel4` (sampler2D) - terminal content texture (par-term specific)\n");
    ctx.push_str("  - `iChannelResolution[0..4]` (vec3) - per-channel texture sizes\n");
    ctx.push_str(
        "  - `iProgress` (vec4) - progress state [state, percent, isActive, activeCount]\n",
    );
    ctx.push_str("Cursor shader extras:\n");
    ctx.push_str("  - `iCurrentCursor` (vec4) - current cursor (xy=top-left px, zw=size px)\n");
    ctx.push_str("  - `iPreviousCursor` (vec4) - previous cursor (xy=top-left px, zw=size px)\n");
    ctx.push_str(
        "  - `iCurrentCursorColor` (vec4) - current cursor RGBA (alpha includes blink opacity)\n",
    );
    ctx.push_str("  - `iPreviousCursorColor` (vec4) - previous cursor RGBA\n");
    ctx.push_str("  - `iTimeCursorChange` (float) - time since last cursor move\n");
    ctx.push_str("  - `iCursorTrailDuration` (float), `iCursorGlowRadius` (float), `iCursorGlowIntensity` (float)\n");

    ctx.push('\n');

    // ---- GLSL Compatibility Rules ----
    ctx.push_str("## [Constraint] GLSL Compatibility Rules\n");
    ctx.push_str("- Avoid passing sampler uniforms (e.g. `sampler2D`) as function parameters.\n");
    ctx.push_str("  Some GLSL versions / toolchains reject sampler-typed function arguments.\n");
    ctx.push_str("- Prefer sampling global uniforms like `iChannel0`..`iChannel4` directly.\n");
    ctx.push_str("Safe helper pattern:\n");
    ctx.push_str("```glsl\n");
    ctx.push_str("vec4 sampleTerminal(vec2 uv) {\n");
    ctx.push_str("    return texture(iChannel4, uv);\n");
    ctx.push_str("}\n");
    ctx.push_str("```\n");
    ctx.push_str("- UV/channel sampling rules:\n");
    ctx.push_str("  - `texture()` expects normalized UVs in `[0,1]`.\n");
    ctx.push_str("  - Start from `uv = fragCoord / iResolution.xy` for screen-space sampling.\n");
    ctx.push_str("  - After transforms (rotation/scale/offset), clamp before sampling:\n");
    ctx.push_str("    `vec2 suv = clamp(transformedUv, vec2(0.0), vec2(1.0));`\n");
    ctx.push_str("  - Do not mix pixel-space and UV-space in one variable.\n");
    ctx.push_str("    Convert pixel coords with `/ iResolution.xy` before `texture()`.\n");
    ctx.push_str(
        "  - Avoid arbitrary `+0.5` UV offsets unless intentionally correcting a known sampling artifact.\n",
    );
    ctx.push_str(
        "    Random `+0.5` shifts usually move sampling into the wrong coordinate space.\n",
    );
    ctx.push_str("- Coordinate-space contract:\n");
    ctx.push_str("  - `fragCoord` and cursor uniforms are pixel-space values.\n");
    ctx.push_str(
        "  - Texture sampling is UV-space. Convert once with `uv = fragCoord / iResolution.xy`.\n",
    );
    ctx.push_str("  - Keep pixel and UV vars separate (`cursorPx`, `cursorUv`, `distPx`, etc.).\n");
    ctx.push_str(
        "  - If mixing cursor data with UV math, convert explicitly: `cursorUv = (iCurrentCursor.xy + 0.5 * iCurrentCursor.zw) / iResolution.xy`.\n",
    );
    ctx.push_str(
        "  - Avoid implicit/double Y-flips. Use one coordinate convention per calculation path.\n",
    );
    ctx.push_str("- Optional channel textures:\n");
    ctx.push_str("  - Unset iChannel0-3 default to transparent 1x1 placeholders.\n");
    ctx.push_str("  - Detect a real configured texture with resolution > 1px, not `> 0.0`.\n");
    ctx.push_str("    Example: `bool hasTex0 = iChannelResolution[0].x > 1.0 && iChannelResolution[0].y > 1.0;`\n");

    ctx.push('\n');

    // ---- Minimal Shader Template ----
    ctx.push_str("## [Instruction] Minimal Shader Template\n");
    ctx.push_str("```glsl\n");
    ctx.push_str("void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n");
    ctx.push_str("    vec2 uv = fragCoord / iResolution.xy;\n");
    ctx.push_str("    vec4 tex = texture(iChannel4, uv);\n");
    ctx.push_str("    fragColor = tex;\n");
    ctx.push_str("}\n");
    ctx.push_str("```\n");

    ctx.push('\n');

    // ---- How to Apply Changes ----
    ctx.push_str("## [Instruction] How to Apply Changes\n");
    ctx.push_str(&format!(
        "1. Write shader GLSL files to: `{}`\n",
        shaders_dir.display()
    ));
    ctx.push_str("2. Use the `config_update` MCP tool to activate the shader:\n");
    ctx.push_str("   ```json\n");
    ctx.push_str(
        "   config_update({\"updates\": {\"custom_shader\": \"filename.glsl\", \"custom_shader_enabled\": true}})\n",
    );
    ctx.push_str("   ```\n");
    ctx.push_str("   For cursor shaders use `cursor_shader` and `cursor_shader_enabled` keys.\n");
    ctx.push_str("3. Changes apply immediately — no restart or manual config edit needed.\n");
    ctx.push_str("4. For visual debugging/verification, use the `terminal_screenshot` MCP tool\n");
    ctx.push_str("   to capture the current terminal output (including shader rendering).\n");
    ctx.push_str("   This may require user permission before the screenshot is returned.\n");
    ctx.push_str(
        "5. Do not stop after writing the file if the user also asked to activate/set it.\n",
    );
    ctx.push_str(
        "   Completion requires a `config_update` call that sets the shader key and enable flag.\n",
    );
    ctx.push_str("6. If reading/listing the shader directory fails, do NOT loop on `Read` for the directory.\n");
    ctx.push_str(
        "   You can write a new file directly to the shader directory path (for example `vortex_checker.glsl`) and then activate it.\n",
    );

    ctx.push('\n');

    // ---- Available Config Keys ----
    ctx.push_str("## [Constraint] Available Config Keys\n");
    ctx.push_str("Background shader: custom_shader (string|null), custom_shader_enabled (bool),\n");
    ctx.push_str("  custom_shader_animation (bool), custom_shader_animation_speed (float),\n");
    ctx.push_str("  custom_shader_brightness (float), custom_shader_text_opacity (float)\n");
    ctx.push_str("Cursor shader: cursor_shader (string|null), cursor_shader_enabled (bool),\n");
    ctx.push_str("  cursor_shader_animation (bool), cursor_shader_animation_speed (float),\n");
    ctx.push_str("  cursor_shader_glow_radius (float), cursor_shader_glow_intensity (float)\n");
    ctx.push('\n');
    ctx.push_str(
        "[Constraint] Do NOT edit config.yaml directly — always use the config_update tool.\n",
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
        assert!(should_inject_shader_context(
            "help me write a shader",
            &config
        ));
    }

    #[test]
    fn test_is_shader_activation_request_true() {
        assert!(is_shader_activation_request(
            "create a shader and set that shader as the active shader"
        ));
    }

    #[test]
    fn test_is_shader_activation_request_false() {
        assert!(!is_shader_activation_request(
            "explain how the custom shader uniforms work"
        ));
    }

    #[test]
    fn test_keyword_shader_uppercase() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "I want a SHADER effect",
            &config
        ));
    }

    #[test]
    fn test_keyword_shader_mixed_case() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "Create a Shader for CRT",
            &config
        ));
    }

    #[test]
    fn test_keyword_glsl() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "write some GLSL code",
            &config
        ));
    }

    #[test]
    fn test_keyword_wgsl() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "what does the wgsl look like",
            &config
        ));
    }

    #[test]
    fn test_keyword_crt() {
        let config = default_config();
        assert!(should_inject_shader_context("I want a crt effect", &config));
    }

    #[test]
    fn test_keyword_scanline() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "add scanline overlay",
            &config
        ));
    }

    #[test]
    fn test_keyword_postprocess_hyphenated() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "post-process my terminal",
            &config
        ));
    }

    #[test]
    fn test_keyword_postprocess_concatenated() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "add a postprocess effect",
            &config
        ));
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
        assert!(should_inject_shader_context(
            "how to use iChannel0",
            &config
        ));
    }

    #[test]
    fn test_keyword_itime() {
        let config = default_config();
        assert!(should_inject_shader_context("animate with iTime", &config));
    }

    #[test]
    fn test_keyword_iresolution() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "normalize by iResolution",
            &config
        ));
    }

    #[test]
    fn test_keyword_shadertoy() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "port this Shadertoy shader",
            &config
        ));
    }

    #[test]
    fn test_keyword_transpile() {
        let config = default_config();
        assert!(should_inject_shader_context("transpile my glsl", &config));
    }

    #[test]
    fn test_keyword_naga() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "naga transpiler error",
            &config
        ));
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
        assert!(should_inject_shader_context(
            "cool background effect",
            &config
        ));
    }

    #[test]
    fn test_keyword_background_shader() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "tweak my background shader",
            &config
        ));
    }

    #[test]
    fn test_keyword_effect() {
        let config = default_config();
        assert!(should_inject_shader_context(
            "what visual effect can I add?",
            &config
        ));
    }

    // ---- Negative keyword cases ----

    #[test]
    fn test_no_keywords_general_question() {
        let config = default_config();
        assert!(!should_inject_shader_context(
            "how do I list files?",
            &config
        ));
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
        assert!(ctx.contains("## [Observation] Current Shader State"));
        assert!(ctx.contains("## [Observation] Available Shaders"));
        assert!(ctx.contains("## [Observation] Debug Files"));
        assert!(ctx.contains("## [Observation] Available Uniforms"));
        assert!(ctx.contains("## [Constraint] GLSL Compatibility Rules"));
        assert!(ctx.contains("## [Instruction] Minimal Shader Template"));
        assert!(ctx.contains("## [Instruction] How to Apply Changes"));
        assert!(ctx.contains("## [Constraint] Available Config Keys"));
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
        assert!(ctx.contains("iChannelResolution[0..4]"));
        assert!(ctx.contains("iProgress"));
        assert!(ctx.contains("iCurrentCursor"));
        assert!(ctx.contains("iPreviousCursor"));
        assert!(ctx.contains("iCurrentCursorColor"));
        assert!(ctx.contains("iPreviousCursorColor"));
        assert!(ctx.contains("iTimeCursorChange"));
        assert!(ctx.contains("iCursorTrailDuration"));
        assert!(ctx.contains("iCursorGlowRadius"));
        assert!(ctx.contains("iCursorGlowIntensity"));
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
    fn test_context_sampler_compatibility_guidance() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("Avoid passing sampler uniforms"));
        assert!(ctx.contains("sampler2D"));
        assert!(ctx.contains("sampleTerminal"));
        assert!(ctx.contains("texture(iChannel4, uv)"));
        assert!(ctx.contains("`texture()` expects normalized UVs in `[0,1]`"));
        assert!(ctx.contains("fragCoord / iResolution.xy"));
        assert!(ctx.contains("clamp(transformedUv, vec2(0.0), vec2(1.0))"));
        assert!(ctx.contains("Do not mix pixel-space and UV-space"));
        assert!(ctx.contains("Avoid arbitrary `+0.5` UV offsets"));
        assert!(ctx.contains("`fragCoord` and cursor uniforms are pixel-space values"));
        assert!(
            ctx.contains(
                "cursorUv = (iCurrentCursor.xy + 0.5 * iCurrentCursor.zw) / iResolution.xy"
            )
        );
        assert!(ctx.contains("Avoid implicit/double Y-flips"));
        assert!(ctx.contains("Unset iChannel0-3 default to transparent 1x1 placeholders"));
        assert!(ctx.contains("iChannelResolution[0].x > 1.0 && iChannelResolution[0].y > 1.0"));
    }

    #[test]
    fn test_context_how_to_apply_section() {
        let config = default_config();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("config_update"));
        assert!(ctx.contains("custom_shader_enabled"));
        assert!(ctx.contains("cursor_shader_enabled"));
        assert!(ctx.contains("Do NOT edit config.yaml directly"));
    }
}
