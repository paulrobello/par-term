//! Controls that attach to `uniform float`.
//!
//! `slider` and `angle` are grouped because they share more than a uniform
//! type: both are counted against `MAX_SHADER_FLOAT_CONTROLS`, so adding an
//! angle control uses up a slider slot. Keeping them in one file makes that
//! shared budget visible.

use super::parse_helpers::{ControlDirective, parse_float_range_control};
use super::{AngleUnit, ShaderControlKind, ShaderControlWarning, SliderScale};

/// Parse a `// control slider ...` directive.
///
/// Returns `None` when the control must be dropped; the caller skips to the
/// next line without recording a duplicate, a capacity slot, or a group.
pub(super) fn parse_slider(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["min", "max", "step", "scale", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "float",
        format!(
            "Slider control for `{}` must attach to `uniform float`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let (min, max, step) =
        match parse_float_range_control("Slider", directive.uniform_name, directive.key_values) {
            Ok(values) => values,
            Err(error) => {
                directive.warn(warnings, error);
                return None;
            }
        };

    let scale = match directive.key_values.get("scale").map(String::as_str) {
        Some("linear") | None => SliderScale::Linear,
        Some("log") => SliderScale::Log,
        Some(_) => {
            directive.warn(
                warnings,
                format!(
                    "Slider `{}` scale must be `linear` or `log`",
                    directive.uniform_name
                ),
            );
            return None;
        }
    };

    if scale == SliderScale::Log && !(0.0 < min && min < max) {
        directive.warn(
            warnings,
            format!(
                "Slider `{}` scale=log requires 0 < min < max",
                directive.uniform_name
            ),
        );
        return None;
    }

    let label = directive.label(warnings, "Slider");

    Some(ShaderControlKind::Slider {
        min,
        max,
        step,
        scale,
        label,
    })
}

/// Parse a `// control angle ...` directive.
pub(super) fn parse_angle(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["unit", "label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "float",
        format!(
            "Angle control for `{}` must attach to `uniform float`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let unit = match directive.key_values.get("unit").map(String::as_str) {
        Some("degrees") | None => AngleUnit::Degrees,
        Some("radians") => AngleUnit::Radians,
        Some(_) => {
            directive.warn(
                warnings,
                format!(
                    "Angle `{}` unit must be `degrees` or `radians`",
                    directive.uniform_name
                ),
            );
            return None;
        }
    };

    let label = directive.label(warnings, "Angle");

    Some(ShaderControlKind::Angle { unit, label })
}
