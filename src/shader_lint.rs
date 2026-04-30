//! Shader linting and readability scoring helpers.
//!
//! The lint pass is intentionally lightweight: it validates the par-term shader
//! metadata/control surface and catches common configuration mismatches without
//! requiring a GPU device.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

const METADATA_MARKER: &str = "/*! par-term shader metadata";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

impl LintSeverity {
    fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintDiagnostic {
    pub severity: LintSeverity,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShaderLintReport {
    pub diagnostics: Vec<LintDiagnostic>,
    pub metadata_present: bool,
    pub control_count: usize,
}

impl ShaderLintReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == LintSeverity::Error)
    }

    fn push(&mut self, severity: LintSeverity, line: Option<usize>, message: impl Into<String>) {
        self.diagnostics.push(LintDiagnostic {
            severity,
            line,
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadabilityScore {
    pub score: u8,
    pub suggested_brightness: f32,
    pub suggested_text_opacity: f32,
    pub notes: Vec<String>,
}

pub fn lint_shader_source(source: &str) -> ShaderLintReport {
    let mut report = ShaderLintReport::default();

    if !source.contains("mainImage") {
        report.push(
            LintSeverity::Error,
            None,
            "Shader must define a Shadertoy-style mainImage function",
        );
    }

    let metadata = match extract_metadata_yaml(source) {
        MetadataYaml::Absent => {
            report.push(
                LintSeverity::Warning,
                None,
                "Missing par-term shader metadata block",
            );
            None
        }
        MetadataYaml::Unterminated => {
            report.metadata_present = true;
            report.push(
                LintSeverity::Error,
                None,
                "Unterminated par-term shader metadata block",
            );
            None
        }
        MetadataYaml::Present(yaml) => {
            report.metadata_present = true;
            match serde_yaml_ng::from_str::<par_term_config::ShaderMetadata>(yaml) {
                Ok(metadata) => Some(metadata),
                Err(error) => {
                    report.push(
                        LintSeverity::Error,
                        None,
                        format!("Invalid shader metadata YAML: {error}"),
                    );
                    None
                }
            }
        }
    };

    validate_metadata_defaults(metadata.as_ref(), &mut report);
    validate_channel_references(source, metadata.as_ref(), &mut report);
    validate_controls(source, &mut report);

    report
}

pub fn lint_shader_file(path: &Path) -> Result<ShaderLintReport, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read shader '{}': {error}", path.display()))?;
    Ok(lint_shader_source(&source))
}

pub fn score_shader_readability(source: &str) -> ReadabilityScore {
    let lower = source.to_lowercase();
    let color_estimate = estimate_color_brightness(source);
    let mut penalty = 0_i32;
    let mut notes = Vec::new();

    if color_estimate.max_component >= 0.85 {
        penalty += 30;
        notes.push("bright shader output can reduce contrast behind text".to_string());
    } else if color_estimate.max_component >= 0.65 {
        penalty += 15;
        notes.push("moderately bright shader output may need dimming".to_string());
    }

    if color_estimate.average_component >= 0.60 {
        penalty += 15;
        notes.push("high average luminance leaves less contrast headroom".to_string());
    }

    if lower.contains("itime") {
        penalty += 10;
        notes.push("animation can distract during dense terminal reading".to_string());
    }

    if lower.contains("sin(") || lower.contains("cos(") || lower.contains("tan(") {
        penalty += 5;
    }

    if lower.contains("noise") || lower.contains("random") || lower.contains("hash") {
        penalty += 10;
        notes.push("noise/random patterns can create busy backgrounds".to_string());
    }

    if has_high_frequency_time_factor(&lower) {
        penalty += 10;
        notes.push("fast time modulation may cause visible flicker".to_string());
    }

    if lower.contains("ichannel4") || lower.contains("fragcoord") && lower.contains("+") {
        penalty += 8;
        notes.push(
            "terminal-content sampling or coordinate distortion can affect glyph clarity"
                .to_string(),
        );
    }

    let score = (100 - penalty).clamp(0, 100) as u8;
    let (suggested_brightness, suggested_text_opacity) = match score {
        85..=100 => (0.75, 0.90),
        70..=84 => (0.60, 0.93),
        55..=69 => (0.45, 0.96),
        _ => (0.30, 1.00),
    };

    if notes.is_empty() {
        notes.push("low-distraction source-level readability profile".to_string());
    }

    ReadabilityScore {
        score,
        suggested_brightness,
        suggested_text_opacity,
        notes,
    }
}

pub fn format_lint_report(
    path: &Path,
    report: &ShaderLintReport,
    readability: Option<&ReadabilityScore>,
) -> String {
    let mut output = String::new();
    output.push_str(&format!("Shader lint: {}\n", path.display()));

    if report.diagnostics.is_empty() {
        output.push_str("No lint issues found.\n");
    } else {
        for diagnostic in &report.diagnostics {
            let location = diagnostic
                .line
                .map(|line| format!(":{line}"))
                .unwrap_or_default();
            output.push_str(&format!(
                "{}{}: {}: {}\n",
                path.display(),
                location,
                diagnostic.severity.label(),
                diagnostic.message
            ));
        }
    }

    output.push_str(&format!(
        "Metadata: {}\n",
        if report.metadata_present {
            "present"
        } else {
            "missing"
        }
    ));
    output.push_str(&format!("Controls: {}\n", report.control_count));

    if let Some(readability) = readability {
        output.push_str(&format!(
            "Readability: {}/100\nSuggested defaults:\n  custom_shader_brightness = {:.2}\n  custom_shader_text_opacity = {:.2}\n",
            readability.score,
            readability.suggested_brightness,
            readability.suggested_text_opacity
        ));
        if !readability.notes.is_empty() {
            output.push_str("Notes:\n");
            for note in &readability.notes {
                output.push_str(&format!("  - {note}\n"));
            }
        }
    }

    output
}

pub fn apply_readability_defaults(path: &Path, score: &ReadabilityScore) -> Result<(), String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read shader '{}': {error}", path.display()))?;
    let mut metadata = par_term_config::parse_shader_metadata(&source).unwrap_or_default();
    metadata.defaults.brightness = Some(score.suggested_brightness);
    metadata.defaults.text_opacity = Some(score.suggested_text_opacity);
    par_term_config::update_shader_metadata_file(path, &metadata)
}

pub fn shader_lint_settings_report(path: &Path) -> Result<String, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read shader '{}': {error}", path.display()))?;
    let report = lint_shader_source(&source);
    let readability = score_shader_readability(&source);
    Ok(format_lint_report(path, &report, Some(&readability)))
}

pub fn shader_lint_cli(
    path: &Path,
    include_readability: bool,
    apply: bool,
    prompt_to_apply: bool,
) -> anyhow::Result<()> {
    let source = std::fs::read_to_string(path)?;
    let report = lint_shader_source(&source);
    let readability = (include_readability || apply).then(|| score_shader_readability(&source));
    print!(
        "{}",
        format_lint_report(path, &report, readability.as_ref())
    );

    if report.has_errors() {
        return Err(anyhow::anyhow!("shader lint failed"));
    }

    if let Some(score) = readability.as_ref() {
        let should_apply = apply || (prompt_to_apply && prompt_user_to_apply()?);
        if should_apply {
            apply_readability_defaults(path, score).map_err(|error| anyhow::anyhow!(error))?;
            println!(
                "Applied suggested readability defaults to {}",
                path.display()
            );
        }
    }

    Ok(())
}

fn prompt_user_to_apply() -> anyhow::Result<bool> {
    print!("Apply suggested readability defaults to shader metadata? [y/N] ");
    io::stdout().flush()?;

    let mut response = String::new();
    io::stdin().read_line(&mut response)?;
    Ok(matches!(
        response.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

enum MetadataYaml<'a> {
    Absent,
    Unterminated,
    Present(&'a str),
}

fn extract_metadata_yaml(source: &str) -> MetadataYaml<'_> {
    let Some(start_marker) = source.find(METADATA_MARKER) else {
        return MetadataYaml::Absent;
    };

    let Some(yaml_start_offset) = source[start_marker + METADATA_MARKER.len()..].find('\n') else {
        return MetadataYaml::Unterminated;
    };
    let yaml_start = start_marker + METADATA_MARKER.len() + yaml_start_offset + 1;

    let Some(yaml_end_offset) = source[yaml_start..].find("*/") else {
        return MetadataYaml::Unterminated;
    };

    MetadataYaml::Present(source[yaml_start..yaml_start + yaml_end_offset].trim())
}

fn validate_metadata_defaults(
    metadata: Option<&par_term_config::ShaderMetadata>,
    report: &mut ShaderLintReport,
) {
    let Some(metadata) = metadata else {
        return;
    };

    if let Some(brightness) = metadata.defaults.brightness
        && (!brightness.is_finite() || !(0.05..=1.0).contains(&brightness))
    {
        report.push(
            LintSeverity::Warning,
            None,
            "defaults.brightness should be a finite value in 0.05..=1.0",
        );
    }

    if let Some(text_opacity) = metadata.defaults.text_opacity
        && (!text_opacity.is_finite() || !(0.0..=1.0).contains(&text_opacity))
    {
        report.push(
            LintSeverity::Warning,
            None,
            "defaults.text_opacity should be a finite value in 0.0..=1.0",
        );
    }

    if let Some(animation_speed) = metadata.defaults.animation_speed
        && (!animation_speed.is_finite() || animation_speed <= 0.0)
    {
        report.push(
            LintSeverity::Warning,
            None,
            "defaults.animation_speed should be a finite positive value",
        );
    }
}

fn validate_channel_references(
    source: &str,
    metadata: Option<&par_term_config::ShaderMetadata>,
    report: &mut ShaderLintReport,
) {
    let references = referenced_channels(source);
    let defaults = metadata.map(|metadata| &metadata.defaults);

    for channel in references {
        match channel {
            0..=3 => {
                let configured = defaults.is_some_and(|defaults| match channel {
                    0 => {
                        defaults.channel0.is_some()
                            || defaults.use_background_as_channel0 == Some(true)
                    }
                    1 => defaults.channel1.is_some(),
                    2 => defaults.channel2.is_some(),
                    3 => defaults.channel3.is_some(),
                    _ => false,
                });
                if !configured {
                    report.push(
                        LintSeverity::Warning,
                        None,
                        format!(
                            "Shader references iChannel{channel}, but metadata defaults.channel{channel} is not set"
                        ),
                    );
                }
            }
            4 => {
                if defaults.is_none_or(|defaults| defaults.full_content != Some(true)) {
                    report.push(
                        LintSeverity::Warning,
                        None,
                        "Shader references iChannel4; set metadata defaults.full_content: true when terminal content sampling is required",
                    );
                }
            }
            _ => {}
        }
    }

    if source.contains("iCubemap") {
        let configured = defaults.is_some_and(|defaults| {
            defaults.cubemap.is_some() && defaults.cubemap_enabled != Some(false)
        });
        if !configured {
            report.push(
                LintSeverity::Warning,
                None,
                "Shader references iCubemap, but metadata defaults.cubemap is not set or cubemap is disabled",
            );
        }
    }
}

fn validate_controls(source: &str, report: &mut ShaderLintReport) {
    let control_parse = par_term_config::parse_shader_controls(source);
    report.control_count = control_parse.controls.len();
    for warning in control_parse.warnings {
        report.push(LintSeverity::Warning, Some(warning.line), warning.message);
    }
}

fn referenced_channels(source: &str) -> BTreeSet<u8> {
    let mut references = BTreeSet::new();
    for channel in 0_u8..=4 {
        if source.contains(&format!("iChannel{channel}")) {
            references.insert(channel);
        }
    }
    references
}

#[derive(Debug, Clone, Copy)]
struct ColorEstimate {
    max_component: f32,
    average_component: f32,
}

fn estimate_color_brightness(source: &str) -> ColorEstimate {
    let mut values = Vec::new();
    for constructor in ["vec3(", "vec4("] {
        let mut remaining = source;
        while let Some(index) = remaining.find(constructor) {
            let start = index + constructor.len();
            let Some(end) = remaining[start..].find(')') else {
                break;
            };
            let args = &remaining[start..start + end];
            values.extend(
                args.split(',')
                    .take(3)
                    .filter_map(parse_normalized_float_prefix),
            );
            remaining = &remaining[start + end + 1..];
        }
    }

    if values.is_empty() {
        values.extend(
            source
                .split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
                .filter_map(|token| {
                    token.parse::<f32>().ok().filter(|value| {
                        value.is_finite() && (0.0..=1.0).contains(value) && *value != 0.0
                    })
                }),
        );
    }

    if values.is_empty() {
        return ColorEstimate {
            max_component: 0.5,
            average_component: 0.5,
        };
    }

    let max_component = values.iter().copied().fold(0.0, f32::max);
    let average_component = values.iter().sum::<f32>() / values.len() as f32;
    ColorEstimate {
        max_component,
        average_component,
    }
}

fn parse_normalized_float_prefix(value: &str) -> Option<f32> {
    let token: String = value
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    token
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

fn has_high_frequency_time_factor(source: &str) -> bool {
    source.contains("itime * 8.")
        || source.contains("itime*8.")
        || source.contains("itime * 9")
        || source.contains("itime*9")
        || source.contains("itime * 10")
        || source.contains("itime*10")
        || source.contains("itime * 20")
        || source.contains("itime*20")
}
