//! Controls that attach to `uniform int`.
//!
//! `int`, `select` and `channel` are grouped because all three are counted
//! against `MAX_SHADER_INT_CONTROLS` -- they compete for the same 16 slots.

use super::parse_helpers::{
    ControlDirective, parse_channel_options, parse_required_i32, parse_select_options,
};
use super::{ShaderControlKind, ShaderControlWarning};

/// Parse a `// control int ...` directive.
///
/// Returns `None` when the control must be dropped; the caller skips to the
/// next line without recording a duplicate, a capacity slot, or a group.
pub(super) fn parse_int(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["min", "max", "step", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "int",
        format!(
            "Int control for `{}` must attach to `uniform int`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let min = match parse_required_i32(directive.key_values, "min") {
        Ok(value) => value,
        Err(error) => {
            directive.warn(
                warnings,
                format!("Int `{}` {}", directive.uniform_name, error),
            );
            return None;
        }
    };
    let max = match parse_required_i32(directive.key_values, "max") {
        Ok(value) => value,
        Err(error) => {
            directive.warn(
                warnings,
                format!("Int `{}` {}", directive.uniform_name, error),
            );
            return None;
        }
    };
    // `step` is the one optional numeric field; absent means 1, present but
    // unparseable is an error rather than a fallback.
    let step = match directive.key_values.get("step") {
        Some(_) => match parse_required_i32(directive.key_values, "step") {
            Ok(value) => value,
            Err(error) => {
                directive.warn(
                    warnings,
                    format!("Int `{}` {}", directive.uniform_name, error),
                );
                return None;
            }
        },
        None => 1,
    };

    if max < min || step <= 0 {
        directive.warn(
            warnings,
            format!(
                "Int `{}` must have max >= min and step > 0",
                directive.uniform_name
            ),
        );
        return None;
    }

    let label = directive.label(warnings, "Int");

    Some(ShaderControlKind::Int {
        min,
        max,
        step,
        label,
    })
}

/// Parse a `// control select ...` directive.
pub(super) fn parse_select(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["options", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "int",
        format!(
            "Select control for `{}` must attach to `uniform int`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let options = match parse_select_options(directive.key_values.get("options")) {
        Ok(options) => options,
        Err(error) => {
            directive.warn(
                warnings,
                format!("Select `{}` {}", directive.uniform_name, error),
            );
            return None;
        }
    };

    let label = directive.label(warnings, "Select");

    Some(ShaderControlKind::Select { options, label })
}

/// Parse a `// control channel ...` directive.
pub(super) fn parse_channel(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["options", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "int",
        format!(
            "Channel control for `{}` must attach to `uniform int`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let options = match parse_channel_options(directive.key_values.get("options")) {
        Ok(options) => options,
        Err(error) => {
            directive.warn(
                warnings,
                format!("Channel `{}` {}", directive.uniform_name, error),
            );
            return None;
        }
    };

    let label = directive.label(warnings, "Channel");

    Some(ShaderControlKind::Channel { options, label })
}
