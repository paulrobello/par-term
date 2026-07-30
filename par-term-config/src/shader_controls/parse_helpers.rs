//! Lexical and field-level helpers shared by every control-kind parser.
//!
//! These are the primitives the per-kind parsers in `float_kinds`, `int_kinds`,
//! `vec2_kinds` and `simple_kinds` are built from: splitting a `// control`
//! directive into tokens, turning those tokens into key/value pairs, and
//! validating individual field values. Nothing here decides what a control
//! *is* -- that is the per-kind parsers' job.

use super::ShaderControlWarning;
use std::collections::BTreeMap;

/// One `// control ...` directive paired with the `uniform` declaration on the
/// following line, after tokenization.
///
/// Bundling these six values is what lets each per-kind parser take two
/// arguments instead of seven.
pub(super) struct ControlDirective<'a> {
    /// 1-based line number of the `// control` comment, used for warnings.
    pub(super) line: usize,
    /// The control-type token as written, e.g. `slider`. Lowercase; warnings
    /// about unrecognized *fields* quote it verbatim.
    pub(super) control_type: &'a str,
    /// GLSL type of the uniform on the next line, e.g. `float`.
    pub(super) uniform_type: &'a str,
    /// Name of the uniform on the next line.
    pub(super) uniform_name: &'a str,
    /// `key=value` fields parsed off the directive.
    pub(super) key_values: &'a BTreeMap<String, String>,
    /// Tokens that were not well-formed `key=value` pairs.
    pub(super) malformed_tokens: &'a [String],
}

impl ControlDirective<'_> {
    /// Push a warning against this directive's line.
    pub(super) fn warn(&self, warnings: &mut Vec<ShaderControlWarning>, message: String) {
        push_warning(warnings, self.line, message);
    }

    /// Warn about malformed tokens and any field outside `allowed_fields`.
    pub(super) fn warn_unknown_fields(
        &self,
        warnings: &mut Vec<ShaderControlWarning>,
        allowed_fields: &[&str],
    ) {
        warn_for_unrecognized_fields(
            warnings,
            self.line,
            self.control_type,
            self.key_values,
            self.malformed_tokens,
            allowed_fields,
        );
    }

    /// Read the optional `label` field.
    ///
    /// An unquoted label warns and yields `None`; it does **not** reject the
    /// control. `display_name` is the capitalized kind name used in that
    /// warning (`Slider`, `Checkbox`, ...), which differs from
    /// [`Self::control_type`] and is quoted verbatim in the message.
    pub(super) fn label(
        &self,
        warnings: &mut Vec<ShaderControlWarning>,
        display_name: &str,
    ) -> Option<String> {
        parse_quoted_label(
            warnings,
            self.line,
            display_name,
            self.uniform_name,
            self.key_values.get("label"),
        )
    }

    /// Reject the control unless its uniform has type `expected`.
    ///
    /// Returns `false` after warning, which the caller turns into `None`.
    pub(super) fn require_uniform_type(
        &self,
        warnings: &mut Vec<ShaderControlWarning>,
        expected: &str,
        message: String,
    ) -> bool {
        if self.uniform_type == expected {
            true
        } else {
            self.warn(warnings, message);
            false
        }
    }
}

pub(super) fn parse_uniform_declaration(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("uniform ") || !trimmed.ends_with(';') {
        return None;
    }

    let without_semicolon = trimmed.trim_end_matches(';').trim();
    let mut parts = without_semicolon.split_whitespace();
    let uniform = parts.next()?;
    let ty = parts.next()?;
    let name = parts.next()?;

    if uniform != "uniform" || parts.next().is_some() {
        return None;
    }

    Some((ty, name))
}

pub(super) fn tokenize_control_directive(rest: &str) -> (Vec<String>, Vec<String>) {
    let mut tokens = Vec::new();
    let mut malformed_tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for character in rest.chars() {
        match character {
            '"' => {
                in_quotes = !in_quotes;
                current.push(character);
            }
            character if character.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            character => current.push(character),
        }
    }

    if !current.is_empty() {
        if in_quotes {
            malformed_tokens.push(current.clone());
        }
        tokens.push(current);
    }

    (tokens, malformed_tokens)
}

pub(super) fn parse_key_values(tokens: &[String]) -> (BTreeMap<String, String>, Vec<String>) {
    let mut key_values = BTreeMap::new();
    let mut malformed_tokens = Vec::new();

    for token in tokens {
        match token.split_once('=') {
            Some((key, value)) if !key.is_empty() && !value.is_empty() => {
                key_values.insert(key.to_string(), value.to_string());
            }
            _ => malformed_tokens.push(token.to_string()),
        }
    }

    (key_values, malformed_tokens)
}

pub(super) fn push_warning(warnings: &mut Vec<ShaderControlWarning>, line: usize, message: String) {
    warnings.push(ShaderControlWarning { line, message });
}

/// Capacity gate shared by the five per-type counters in `parse_shader_controls`
/// (QA-003: collapses five identical `if count >= MAX { warn; continue }` blocks).
///
/// Returns `true` when `count` has reached `limit`, after pushing a warning
/// identical to the original inline messages. The caller `continue`s the parse
/// loop on `true` so the over-limit control is dropped — matching the prior
/// inline behavior.
pub(super) fn check_and_push_capacity_warning(
    warnings: &mut Vec<ShaderControlWarning>,
    line: usize,
    type_label: &str,
    limit: usize,
    count: usize,
    uniform_name: &str,
) -> bool {
    if count >= limit {
        push_warning(
            warnings,
            line,
            format!(
                "Only the first {} {} controls are active; ignoring over-limit control `{}`",
                limit, type_label, uniform_name
            ),
        );
        true
    } else {
        false
    }
}

pub(super) fn unquote(value: &str) -> Option<&str> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
}

pub(super) fn parse_quoted_label(
    warnings: &mut Vec<ShaderControlWarning>,
    line: usize,
    control_type: &str,
    uniform_name: &str,
    value: Option<&String>,
) -> Option<String> {
    let value = value?;
    match unquote(value) {
        Some(label) => Some(label.to_string()),
        None => {
            push_warning(
                warnings,
                line,
                format!("{} `{}` label must be quoted", control_type, uniform_name),
            );
            None
        }
    }
}

pub(super) fn warn_for_unrecognized_fields(
    warnings: &mut Vec<ShaderControlWarning>,
    line: usize,
    control_type: &str,
    key_values: &BTreeMap<String, String>,
    malformed_tokens: &[String],
    allowed_fields: &[&str],
) {
    for token in malformed_tokens {
        push_warning(
            warnings,
            line,
            format!("Malformed control token `{}`", token),
        );
    }

    for key in key_values.keys() {
        if !allowed_fields.contains(&key.as_str()) {
            push_warning(
                warnings,
                line,
                format!("Unknown {} control field `{}`", control_type, key),
            );
        }
    }
}

pub(super) fn parse_required_f32(
    key_values: &BTreeMap<String, String>,
    key: &str,
) -> Result<f32, String> {
    let value = key_values
        .get(key)
        .ok_or_else(|| format!("missing `{}`", key))?
        .parse::<f32>()
        .map_err(|_| format!("invalid `{}`", key))?;

    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("`{}` must be finite", key))
    }
}

pub(super) fn parse_required_i32(
    key_values: &BTreeMap<String, String>,
    key: &str,
) -> Result<i32, String> {
    key_values
        .get(key)
        .ok_or_else(|| format!("missing `{}`", key))?
        .parse::<i32>()
        .map_err(|_| format!("invalid `{}`", key))
}

pub(super) fn parse_float_range_control(
    control_label: &str,
    uniform_name: &str,
    key_values: &BTreeMap<String, String>,
) -> Result<(f32, f32, f32), String> {
    let min = parse_required_f32(key_values, "min")
        .map_err(|error| format!("{} `{}` {}", control_label, uniform_name, error))?;
    let max = parse_required_f32(key_values, "max")
        .map_err(|error| format!("{} `{}` {}", control_label, uniform_name, error))?;
    let step = parse_required_f32(key_values, "step")
        .map_err(|error| format!("{} `{}` {}", control_label, uniform_name, error))?;

    if max < min || step <= 0.0 {
        return Err(format!(
            "{} `{}` must have max >= min and step > 0",
            control_label, uniform_name
        ));
    }

    Ok((min, max, step))
}

pub(super) fn parse_select_options(value: Option<&String>) -> Result<Vec<String>, String> {
    let value = value.ok_or_else(|| "missing `options`".to_string())?;
    let quoted = unquote(value).ok_or_else(|| "`options` must be quoted".to_string())?;
    let mut options = Vec::new();
    for option in quoted.split(',').map(str::trim) {
        if option.is_empty() {
            return Err("`options` must not contain empty labels".to_string());
        }
        options.push(option.to_string());
    }

    if options.is_empty() {
        Err("`options` must include at least one label".to_string())
    } else {
        Ok(options)
    }
}

pub(super) fn parse_channel_options(value: Option<&String>) -> Result<Vec<i32>, String> {
    let Some(value) = value else {
        return Ok(vec![0, 1, 2, 3, 4]);
    };
    let quoted = unquote(value).ok_or_else(|| "`options` must be quoted".to_string())?;
    let mut options = Vec::new();
    for option in quoted.split(',').map(str::trim) {
        if option.is_empty() {
            return Err("`options` must contain channel numbers in 0..=4".to_string());
        }
        let channel = option
            .parse::<i32>()
            .map_err(|_| "`options` must contain channel numbers in 0..=4".to_string())?;
        if !(0..=4).contains(&channel) {
            return Err("`options` must contain channel numbers in 0..=4".to_string());
        }
        options.push(channel);
    }

    if options.is_empty() {
        Err("`options` must contain at least one channel in 0..=4".to_string())
    } else {
        Ok(options)
    }
}
