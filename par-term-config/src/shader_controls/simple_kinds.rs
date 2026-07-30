//! The two controls that own a capacity budget outright.
//!
//! `checkbox` is the only consumer of `MAX_SHADER_BOOL_CONTROLS` and `color`
//! the only consumer of `MAX_SHADER_COLOR_CONTROLS`, so neither shares slots
//! with anything the way the float, int and vec2 kinds do.

use super::parse_helpers::ControlDirective;
use super::{ShaderControlKind, ShaderControlWarning};

/// Parse a `// control checkbox ...` directive.
///
/// Returns `None` when the control must be dropped; the caller skips to the
/// next line without recording a duplicate, a capacity slot, or a group.
pub(super) fn parse_checkbox(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["label", "group"]);

    if !directive.require_uniform_type(
        warnings,
        "bool",
        format!(
            "Checkbox control for `{}` must attach to `uniform bool`",
            directive.uniform_name
        ),
    ) {
        return None;
    }

    let label = directive.label(warnings, "Checkbox");

    Some(ShaderControlKind::Checkbox { label })
}

/// Parse a `// control color ...` directive.
///
/// The only kind that accepts two uniform types, so it cannot use
/// [`ControlDirective::require_uniform_type`]. `vec4` defaults to an alpha
/// channel and `vec3` does not; an explicit `alpha=true` on a `vec3` is an
/// error, while an unrecognized `alpha=` value only warns and falls back to
/// the default.
pub(super) fn parse_color(
    directive: &ControlDirective<'_>,
    warnings: &mut Vec<ShaderControlWarning>,
) -> Option<ShaderControlKind> {
    directive.warn_unknown_fields(warnings, &["alpha", "label", "group"]);

    if directive.uniform_type != "vec3" && directive.uniform_type != "vec4" {
        directive.warn(
            warnings,
            format!(
                "Color control for `{}` must attach to `uniform vec3` or `uniform vec4`",
                directive.uniform_name
            ),
        );
        return None;
    }

    let default_alpha = directive.uniform_type == "vec4";
    let alpha = match directive.key_values.get("alpha").map(String::as_str) {
        Some("true") => true,
        Some("false") => false,
        Some(_) => {
            directive.warn(
                warnings,
                format!(
                    "Color `{}` alpha must be `true` or `false`; using default",
                    directive.uniform_name
                ),
            );
            default_alpha
        }
        None => default_alpha,
    };

    if directive.uniform_type == "vec3" && alpha {
        directive.warn(
            warnings,
            format!(
                "Color control `{}` cannot use alpha=true with `uniform vec3`",
                directive.uniform_name
            ),
        );
        return None;
    }

    let label = directive.label(warnings, "Color");

    Some(ShaderControlKind::Color { alpha, label })
}
