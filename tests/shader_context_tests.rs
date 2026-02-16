//! Integration tests for shader context generation.

use par_term::ai_inspector::shader_context::{build_shader_context, should_inject_shader_context};
use par_term::config::Config;

#[test]
fn test_shader_context_contains_all_sections() {
    let config = Config::default();
    let ctx = build_shader_context(&config);

    // All sections must be present
    assert!(ctx.contains("[Shader Assistant Context]"));
    assert!(ctx.contains("## Current Shader State"));
    assert!(ctx.contains("## Available Shaders"));
    assert!(ctx.contains("## Debug Files"));
    assert!(ctx.contains("## Available Uniforms"));
    assert!(ctx.contains("## Minimal Shader Template"));
    assert!(ctx.contains("## How to Apply Changes"));
}

#[test]
fn test_shader_context_template_is_valid_glsl() {
    let config = Config::default();
    let ctx = build_shader_context(&config);

    // Template must contain the mainImage signature
    assert!(ctx.contains("void mainImage(out vec4 fragColor, in vec2 fragCoord)"));
    assert!(ctx.contains("iChannel4"));
    assert!(ctx.contains("iResolution"));
}

#[test]
fn test_keyword_detection_comprehensive() {
    let config = Config::default();

    // Positive cases
    let positive = vec![
        "Create a shader effect",
        "Help me with GLSL code",
        "What WGSL output do I get?",
        "Make a CRT effect",
        "Add scanline post-processing",
        "Port this Shadertoy shader",
        "Fix the cursor effect",
        "iTime is not working",
    ];
    for msg in positive {
        assert!(
            should_inject_shader_context(msg, &config),
            "Expected true for: {msg}"
        );
    }

    // Negative cases
    let negative = vec![
        "How do I change the font?",
        "Set terminal background color",
        "Configure keybindings",
        "What version is this?",
    ];
    for msg in negative {
        assert!(
            !should_inject_shader_context(msg, &config),
            "Expected false for: {msg}"
        );
    }
}

#[test]
fn test_shader_context_with_active_config() {
    let mut config = Config::default();
    config.custom_shader = Some("crt.glsl".to_string());
    config.custom_shader_enabled = true;
    config.custom_shader_animation_speed = 2.0;
    config.custom_shader_brightness = 0.5;

    let ctx = build_shader_context(&config);

    // Should include active shader info
    assert!(ctx.contains("crt.glsl"));
    assert!(ctx.contains("enabled"));
    assert!(ctx.contains("animation_speed: 2"));
    assert!(ctx.contains("brightness: 0.5"));
    assert!(ctx.contains("/tmp/par_term_crt_shader.wgsl"));
}
