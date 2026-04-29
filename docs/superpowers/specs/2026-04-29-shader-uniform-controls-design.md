# Shader Uniform Controls Design

## Summary

Add v1 support for ad-hoc controls on the custom background shader page. Shader authors declare controls with a special comment immediately before a GLSL uniform declaration. Par-term parses those declarations, renders matching controls in settings, persists user changes as per-shader overrides, and passes resolved values to the shader at runtime.

## Goals

- Support float sliders and bool checkboxes for custom shader uniforms.
- Keep shader source explicit: controls attach to normal GLSL `uniform` declarations.
- Reuse the existing three-tier shader configuration model: user override, shader metadata default, global/type fallback.
- Keep the existing built-in custom shader uniform buffer layout stable.
- Make invalid control comments non-fatal and visible to the user.

## Non-goals

- No color pickers, integer controls, enum controls, vectors, or texture selectors in v1.
- No self-contained comment syntax that creates uniforms without GLSL declarations.
- No session-only controls; values persist through existing per-shader config overrides.
- No support for misspelled fields such as `defaullt`.

## Shader Syntax

Controls are declared by placing a `// control ...` comment immediately before the uniform they control.

```glsl
// control slider min=0 max=1 step=0.01
uniform float iGlow;

// control checkbox
uniform bool iEnabled;
```

Supported v1 controls:

- `slider` on `uniform float` with required `min`, `max`, and `step` fields.
- `checkbox` on `uniform bool` with no required fields.

The comment does not contain a default value. Defaults are declared in shader metadata.

```yaml
defaults:
  uniforms:
    iGlow: 0.5
    iEnabled: true
```

Only correct field spelling is supported. Unknown fields or malformed comments produce warnings rather than breaking shader loading.

## Data Model

Extend shader metadata and per-shader config with a custom uniform value map under `uniforms`.

Resolution order for each controlled uniform:

1. User per-shader override in `config.yaml`.
2. Shader metadata default in `defaults.uniforms`.
3. Type fallback: slider `min` for floats, `false` for bools.

The same map shape is used for user overrides and metadata defaults so existing per-shader save/reset patterns remain consistent.

## Runtime Shader Plumbing

Custom controlled uniforms use a separate fixed-size uniform block instead of extending the existing 304-byte built-in Shadertoy/par-term uniform struct. This avoids destabilizing the current built-in layout and its Rust/GLSL synchronization requirements.

V1 limits:

- 16 custom float controls.
- 16 custom bool controls.

The transpiler/wrapper maps shader-declared uniforms such as `uniform float iGlow;` and `uniform bool iEnabled;` to values from the generated custom uniform block. The shader author still writes and reads normal uniform names in GLSL.

If a shader declares more controlled uniforms than the v1 limits, extras are ignored with warnings.

## Settings UI

When the active background shader contains valid control declarations, the existing `Shader Settings` collapsible section shows a `Shader Controls` subsection.

Behavior:

- Sliders use the uniform name as the label by default, with range and step from the control comment.
- Checkboxes use the uniform name as the label by default.
- Changing a control writes a per-shader override immediately, sets `has_changes`, and participates in the existing settings apply/save flow.
- Each control has a reset button that clears only that uniform override.
- `Reset All Overrides` clears custom uniform overrides in addition to existing per-shader override fields.
- `Save Defaults to Shader` writes current effective custom uniform values to `defaults.uniforms` in the shader metadata block.

Warnings for invalid comments appear near shader settings and in the shader editor warning/error area. They do not prevent editing or shader use unless shader compilation itself fails.

## Error Handling

- Missing slider fields (`min`, `max`, or `step`) produce a warning and skip that control.
- Unsupported uniform types produce a warning and skip that control.
- Duplicate controls for the same uniform produce a warning; the first valid declaration wins.
- Values are clamped to slider ranges before upload.
- Extra controls beyond the fixed v1 limits are ignored with warnings.

## Testing

Add focused tests for:

- Control parser: valid slider, valid checkbox, missing slider fields, unsupported uniform type, duplicate names, and extra tokens.
- Config resolution: override beats metadata default, metadata default beats fallback, fallback uses slider min or `false`.
- Metadata serialization: `defaults.uniforms` round-trips and `Save Defaults to Shader` includes custom uniform defaults.
- Transpiler/runtime mapping: controlled uniform names are available to shader source and compile through the wrapper.
- Settings mutation helpers where practical: control change writes override, reset clears one override, reset-all clears all custom uniform overrides.

Verification should run narrow relevant tests first, then `make checkall` before claiming implementation complete.

## Documentation

Update `docs/CUSTOM_SHADERS.md` with:

- `// control slider min=... max=... step=...` syntax.
- `// control checkbox` syntax.
- Metadata defaults via `defaults.uniforms`.
- Persistence behavior through per-shader overrides.
- V1 limits and non-fatal warning behavior.
- A complete minimal shader example.
