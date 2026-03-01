//! Tests for the shader context module.

use super::context_builder::build_shader_context;
use super::helpers::{
    classify_shaders, is_shader_activation_request, scan_shaders, should_inject_shader_context,
};
use crate::config::Config;
use std::path::Path;

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
        ctx.contains("cursorUv = (iCurrentCursor.xy + 0.5 * iCurrentCursor.zw) / iResolution.xy")
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
