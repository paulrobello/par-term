//! Controls that attach to `uniform vec2`.
//!
//! `vec2`, `point` and `range` are grouped because all three are counted
//! against `MAX_SHADER_VEC2_CONTROLS` -- they compete for the same 16 slots.

use super::parse_helpers::{ControlDirective, parse_float_range_control};
use super::{ShaderControlKind, ShaderControlWarning};

/// Parse a `// control vec2 ...` directive.
///
/// Returns `None` when the control must be dropped; the caller skips to the
/// next line without recording a duplicate, a capacity slot, or a group.
pub(super) fn parse_vec2(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["min", "max", "step", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "vec2",
        format!(
            "Vec2 control for `{}` must attach to `uniform vec2`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let (min, max, step) =
        match parse_float_range_control("Vec2", directive.uniform_name, directive.key_values) {
            Ok(values) => values,
            Err(error) => {
                directive.warn(warnings, error);
                return None;
            }
        };

    let label = directive.label(warnings, "Vec2");

    Some(ShaderControlKind::Vec2 {
        min,
        max,
        step,
        label,
    })
}

/// Parse a `// control point ...` directive.
pub(super) fn parse_point(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "vec2",
        format!(
            "Point control for `{}` must attach to `uniform vec2`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let label = directive.label(warnings, "Point");

    Some(ShaderControlKind::Point { label })
}

/// Parse a `// control range ...` directive.
pub(super) fn parse_range(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["min", "max", "step", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "vec2",
        format!(
            "Range control for `{}` must attach to `uniform vec2`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let (min, max, step) =
        match parse_float_range_control("Range", directive.uniform_name, directive.key_values) {
            Ok(values) => values,
            Err(error) => {
                directive.warn(warnings, error);
                return None;
            }
        };

    let label = directive.label(warnings, "Range");

    Some(ShaderControlKind::Range {
        min,
        max,
        step,
        label,
    })
}
