use clap::Parser;
use par_term::cli::{Cli, Commands};
use par_term::shader_lint::{
    LintSeverity, apply_readability_defaults, lint_shader_source, score_shader_readability,
};

fn sample_shader(body: &str, defaults: &str) -> String {
    format!(
        r#"/*! par-term shader metadata
name: "Test Shader"
defaults:
{defaults}
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {{
{body}
}}
"#
    )
}

#[test]
fn lint_warns_when_terminal_channel_used_without_full_content_default() {
    let source = sample_shader(
        "    vec2 uv = fragCoord / iResolution.xy;\n    fragColor = texture(iChannel4, uv);",
        "  brightness: 0.8\n  text_opacity: 0.9\n  full_content: false",
    );

    let report = lint_shader_source(&source);

    assert!(!report.has_errors());
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == LintSeverity::Warning
            && diagnostic.message.contains("iChannel4")
            && diagnostic.message.contains("full_content")
    }));
}

#[test]
fn lint_warns_when_referenced_texture_channel_has_no_metadata_default() {
    let source = sample_shader(
        "    vec2 uv = fragCoord / iResolution.xy;\n    fragColor = texture(iChannel2, uv);",
        "  brightness: 0.8\n  text_opacity: 0.9\n  full_content: false",
    );

    let report = lint_shader_source(&source);

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == LintSeverity::Warning
            && diagnostic.message.contains("iChannel2")
            && diagnostic.message.contains("defaults.channel2")
    }));
}

#[test]
fn lint_reports_malformed_shader_controls_as_warnings() {
    let source = sample_shader(
        "    // control slider min=0 max=1 step=0.1\n    uniform bool iGlow;\n    fragColor = vec4(0.1, 0.1, 0.1, 1.0);",
        "  brightness: 0.8\n  text_opacity: 0.9\n  full_content: false",
    );

    let report = lint_shader_source(&source);

    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == LintSeverity::Warning
            && diagnostic.message.contains("Slider control")
            && diagnostic.message.contains("uniform float")
    }));
}

#[test]
fn readability_recommends_safe_defaults_for_bright_animated_shader() {
    let source = sample_shader(
        "    vec2 uv = fragCoord / iResolution.xy;\n    float pulse = sin(iTime * 20.0) * 0.5 + 0.5;\n    fragColor = vec4(vec3(0.95 + pulse * 0.05), 1.0);",
        "  brightness: 1.0\n  text_opacity: 0.7\n  full_content: false",
    );

    let score = score_shader_readability(&source);

    assert!(score.score <= 60, "score was {}", score.score);
    assert!(
        score.suggested_brightness <= 0.35,
        "brightness was {}",
        score.suggested_brightness
    );
    assert!(
        score.suggested_text_opacity >= 0.95,
        "text opacity was {}",
        score.suggested_text_opacity
    );
    assert!(score.notes.iter().any(|note| note.contains("bright")));
}

#[test]
fn readability_allows_brighter_defaults_for_dark_static_shader() {
    let source = sample_shader(
        "    fragColor = vec4(vec3(0.04, 0.05, 0.07), 1.0);",
        "  brightness: 0.8\n  text_opacity: 0.95\n  full_content: false",
    );

    let score = score_shader_readability(&source);

    assert!(score.score >= 80, "score was {}", score.score);
    assert!(
        score.suggested_brightness >= 0.65,
        "brightness was {}",
        score.suggested_brightness
    );
    assert!(
        score.suggested_text_opacity <= 0.92,
        "text opacity was {}",
        score.suggested_text_opacity
    );
}

#[test]
fn cli_parses_shader_lint_subcommand_with_readability_flag() {
    let cli = Cli::try_parse_from([
        "par-term",
        "shader-lint",
        "shaders/crt.glsl",
        "--readability",
    ])
    .expect("shader-lint should parse");

    match cli.command {
        Some(Commands::ShaderLint {
            path,
            readability,
            apply,
            no_prompt,
        }) => {
            assert_eq!(path, std::path::PathBuf::from("shaders/crt.glsl"));
            assert!(readability);
            assert!(!apply);
            assert!(!no_prompt);
        }
        _ => panic!("expected shader-lint command"),
    }
}

#[test]
fn cli_parses_shader_lint_apply_flag() {
    let cli = Cli::try_parse_from(["par-term", "shader-lint", "shaders/crt.glsl", "--apply"])
        .expect("shader-lint --apply should parse");

    match cli.command {
        Some(Commands::ShaderLint {
            path,
            readability,
            apply,
            no_prompt,
        }) => {
            assert_eq!(path, std::path::PathBuf::from("shaders/crt.glsl"));
            assert!(!readability);
            assert!(apply);
            assert!(!no_prompt);
        }
        _ => panic!("expected shader-lint command"),
    }
}

#[test]
fn cli_parses_shader_lint_no_prompt_flag() {
    let cli = Cli::try_parse_from([
        "par-term",
        "shader-lint",
        "shaders/crt.glsl",
        "--readability",
        "--no-prompt",
    ])
    .expect("shader-lint --no-prompt should parse");

    match cli.command {
        Some(Commands::ShaderLint {
            path,
            readability,
            apply,
            no_prompt,
        }) => {
            assert_eq!(path, std::path::PathBuf::from("shaders/crt.glsl"));
            assert!(readability);
            assert!(!apply);
            assert!(no_prompt);
        }
        _ => panic!("expected shader-lint command"),
    }
}

#[test]
fn apply_readability_defaults_updates_shader_metadata() {
    let mut file = tempfile::NamedTempFile::new().expect("temp shader");
    let source = sample_shader(
        "    float pulse = sin(iTime * 20.0) * 0.5 + 0.5;\n    fragColor = vec4(vec3(0.95 + pulse * 0.05), 1.0);",
        "  brightness: 1.0\n  text_opacity: 0.7\n  full_content: false",
    );
    std::io::Write::write_all(&mut file, source.as_bytes()).expect("write shader");

    let score = score_shader_readability(&source);
    apply_readability_defaults(file.path(), &score).expect("apply readability defaults");

    let updated = std::fs::read_to_string(file.path()).expect("read updated shader");
    let metadata = par_term_config::parse_shader_metadata(&updated).expect("metadata exists");
    assert_eq!(
        metadata.defaults.brightness,
        Some(score.suggested_brightness)
    );
    assert_eq!(
        metadata.defaults.text_opacity,
        Some(score.suggested_text_opacity)
    );
    assert_eq!(metadata.defaults.full_content, Some(false));
}
