# Shader Control Types Design

## Summary

Extend background shader uniform controls beyond slider, checkbox, and color. The new controls let shader authors expose common GLSL parameters directly in the Settings UI while preserving the existing explicit model: a `// control ...` comment attaches to the immediately following uniform declaration.

This design adds:

- `int` sliders for counts and iterations.
- `select` dropdowns for discrete shader modes.
- `vec2` controls for two-component numeric values.
- `point` controls for normalized screen positions.
- `range` controls for min/max bands.
- `slider scale=log` for exponential-feeling float ranges.
- `angle` controls for rotations/directions.
- `channel` selectors for choosing existing `iChannel0`..`iChannel4` inputs.

Existing slider, checkbox, and color controls remain compatible.

## Goals

- Keep controls explicit and attached to GLSL uniform declarations.
- Support practical shader UI patterns without adding arbitrary dynamic resource binding.
- Store defaults in shader metadata under `defaults.uniforms`, not in control comments.
- Persist user overrides in the existing per-shader override path.
- Add assistant guidance that explains both syntax and when each control type is appropriate.
- Preserve robust validation: malformed controls are skipped with warnings, not fatal errors.

## Non-Goals

- Do not let `channel` controls create or modify texture bindings. V1 only selects among existing channels via an integer uniform.
- Do not introduce a generic untyped control schema. Use typed Rust variants for validation and maintainability.
- Do not add controls to cursor shaders in this phase. This remains background-shader-only, matching current shader uniform controls.

## Control Syntax

All controls use the current attached-comment pattern:

```glsl
// control <type> <fields...>
uniform <type> <name>;
```

Optional `label="..."` is supported by all control types. Unsupported fields produce non-fatal warnings.

### Float Slider

```glsl
// control slider min=0 max=1 step=0.01 label="Glow"
uniform float iGlow;
```

Use for continuous linear amounts such as opacity, strength, mix, brightness, speed, or threshold.

Fields:

- `min` required finite float
- `max` required finite float, `max >= min`
- `step` required finite positive float
- `scale` optional, `linear` default or `log`
- `label` optional quoted string

`scale=log` requires `0 < min < max` and positive `step`.

### Int Slider

```glsl
// control int min=1 max=12 step=1 label="Octaves"
uniform int iOctaves;
```

Use for counts, iterations, samples, octaves, quantization levels, and other integer values.

Fields:

- `min` required integer
- `max` required integer, `max >= min`
- `step` optional positive integer, default `1`
- `label` optional quoted string

### Select Dropdown

```glsl
// control select options="soft,hard,screen,add" label="Blend Mode"
uniform int iBlendMode;
```

Use for discrete shader modes. The shader receives the zero-based selected option index.

Fields:

- `options` required quoted comma-separated labels; at least one non-empty option
- `label` optional quoted string

### Vec2 Control

```glsl
// control vec2 min=-1 max=1 step=0.01 label="Flow"
uniform vec2 iFlow;
```

Use for directions, offsets, scales, velocities, and other two-component numeric values.

Fields:

- `min` required finite float
- `max` required finite float, `max >= min`
- `step` required finite positive float
- `label` optional quoted string

### Point Control

```glsl
// control point label="Origin"
uniform vec2 iOrigin;
```

Use for normalized focal points, ripple origins, light positions, and screen-space anchors. Values are normalized UV coordinates in `0.0..=1.0`.

Fields:

- `label` optional quoted string

### Range Control

```glsl
// control range min=0 max=1 step=0.01 label="Glow Range"
uniform vec2 iGlowRange;
```

Use for low/high thresholds, bandpass ranges, mask windows, and falloff ranges. The shader receives `vec2(low, high)`.

Fields:

- `min` required finite float
- `max` required finite float, `max >= min`
- `step` required finite positive float
- `label` optional quoted string

The UI and normalization enforce `x <= y`.

### Log Slider

Log sliders are the float slider with `scale=log`:

```glsl
// control slider min=0.01 max=100 step=0.01 scale=log label="Frequency"
uniform float iFrequency;
```

Use for frequency, exposure, gain, blur radius, and other controls where useful values span orders of magnitude.

### Angle Control

```glsl
// control angle unit=degrees label="Rotation"
uniform float iRotation;
```

Use for rotation, direction, hue angle, scanline skew, and polar effects.

Fields:

- `unit` optional, `degrees` default or `radians`
- `label` optional quoted string

Defaults and overrides are authored in the declared UI unit. Renderer upload always converts to radians so shader code has a consistent unit.

### Channel Selector

```glsl
// control channel options="0,1,2,3,4" label="Source Channel"
uniform int iSourceChannel;
```

Use when a shader can sample from multiple existing texture/content channels. The shader receives an integer channel index. It must still branch/sample explicitly, for example:

```glsl
vec4 sampleSelectedChannel(int channel, vec2 uv) {
    if (channel == 0) return texture(iChannel0, uv);
    if (channel == 1) return texture(iChannel1, uv);
    if (channel == 2) return texture(iChannel2, uv);
    if (channel == 3) return texture(iChannel3, uv);
    return texture(iChannel4, uv);
}
```

Fields:

- `options` optional quoted comma-separated channel numbers from `0` through `4`; default `"0,1,2,3,4"`
- `label` optional quoted string

## Defaults and Overrides

Defaults remain in shader metadata:

```yaml
/*! par-term shader metadata
name: Advanced Controls Demo
defaults:
  uniforms:
    iGlow: 0.5
    iFrequency: 3.0
    iOctaves: 4
    iBlendMode: 2
    iFlow: [0.25, -0.1]
    iOrigin: [0.5, 0.5]
    iGlowRange: [0.2, 0.8]
    iRotation: 45.0
    iSourceChannel: 4
*/
```

Value types:

- `Float(f32)` for float sliders, log sliders, and angle controls.
- `Int(i32)` for int, select, and channel controls.
- `Vec2([f32; 2])` for vec2, point, and range controls.
- Existing `Bool(bool)` and `Color(ShaderColorValue)` remain unchanged.

Numeric YAML compatibility:

- Integer YAML numbers may deserialize as `Int` and fractional YAML numbers as `Float`.
- Float controls accept compatible `Int` defaults by converting to `f32`, preserving existing metadata such as `iGlow: 1`.
- Int/select/channel controls accept compatible finite integral `Float` defaults by converting to `i32`.
- Non-integral floats for int-like controls are wrong-type values and fall back.

Normalization applies in one shared path used by live UI, Save Defaults, and renderer upload:

- Float sliders clamp to `min..max`.
- Log sliders clamp to the positive parsed range.
- Int values clamp to `min..max` and snap to the nearest step offset from `min`.
- Select values clamp to a valid zero-based option index.
- Channel values must be one of the allowed channel options; otherwise they fall back to the first allowed option.
- Vec2 values clamp each component to `min..max`.
- Point values clamp each component to `0..1`.
- Range values clamp to `min..max` and reorder/enforce low/high.
- Angle values are stored in the declared UI unit and uploaded as radians.

Wrong-type defaults are ignored for that control and fall back to the control fallback value.

Fallback values:

- Slider/log slider: `min`.
- Int: `min`.
- Select: `0`.
- Vec2: `[min, min]`.
- Point: `[0.5, 0.5]`.
- Range: `[min, max]`.
- Angle: `0.0` in the declared unit.
- Channel: first allowed channel option, usually `0`.
- Existing checkbox: `false`.
- Existing color: opaque white.

## Parser and Validation

Add typed variants to `ShaderControlKind` rather than storing loose maps.

Validation rules:

- `slider` attaches to `uniform float`.
- `slider scale=log` requires positive finite bounds.
- `int`, `select`, and `channel` attach to `uniform int`.
- `vec2`, `point`, and `range` attach to `uniform vec2`.
- `angle` attaches to `uniform float`.
- Labels must be quoted.
- Select options must be quoted comma-separated labels.
- Channel options must be quoted comma-separated integers in `0..=4`.
- Duplicate controls for the same uniform are ignored after warning.
- Over-limit controls are ignored after warning.
- Comments not immediately followed by a compatible uniform are ignored after warning.

## Renderer and Transpiler

Extend the custom-control uniform buffer with two additional bounded slot classes:

- `int_values`: 16 integer slots, stored as aligned `ivec4`/`uvec4` groups.
- `vec2_values`: 16 `vec2` slots, stored in aligned `vec4` groups or `vec4[8]`.

Existing slot classes remain:

- Float slots: used by slider, log slider, and angle.
- Bool slots: used by checkbox.
- Color slots: used by color.

Transpiler mapping:

- Float slider/log/angle declarations become `#define name <float slot>`.
- Int/select/channel declarations become `#define name <int slot>`.
- Vec2/point/range declarations become `#define name <vec2 slot>`.
- Existing bool/color mappings remain unchanged.

Malformed or over-limit attached declarations are stripped and replaced with safe fallback defines, matching the existing behavior that prevents unresolved uniforms from reaching Naga/WGSL translation.

## Settings UI

Add widgets to the existing Shader Uniform Controls section:

- Slider/log slider: egui slider; log response when supported or helper mapping otherwise.
- Int: integer slider/drag.
- Select: combo box with option labels.
- Vec2: two numeric controls labeled X/Y.
- Point: normalized X/Y controls plus a simple Center button; no canvas picker in v1.
- Range: two numeric controls/sliders enforcing low <= high.
- Angle: numeric slider/drag in declared UI unit.
- Channel: combo box with `iChannelN` labels.

Each control keeps the existing reset button behavior. Reset removes only that uniform override and prunes empty per-shader override entries. Save Defaults writes normalized effective values to shader metadata.

## Limits

Use a default cap of 16 controls per slot class:

- 16 float controls: slider, log slider, angle.
- 16 int controls: int, select, channel.
- 16 vec2 controls: vec2, point, range.
- 16 bool controls: checkbox.
- 16 color controls: color.

Extra controls are skipped with warnings.

## Documentation and Assistant Guidance

Update:

- `docs/CUSTOM_SHADERS.md` with syntax, defaults, limits, examples, and when-to-use guidance.
- `docs/ASSISTANT_PANEL.md` to mention expanded shader-control support.
- `src/ai_inspector/shader_context/context_builder.rs` so assistant prompts recommend appropriate controls.
- Shader context tests to assert the new guidance is present.
- `CHANGELOG.md`.

Assistant guidance should recommend:

- `slider` for continuous amounts.
- `scale=log` for frequency/exposure/gain/radius values spanning orders of magnitude.
- `int` for counts/iterations/octaves.
- `select` for discrete modes.
- `vec2` for directions/offsets/scales.
- `point` for normalized origins/focal points.
- `range` for min/max thresholds and bands.
- `angle` for rotation/direction.
- `channel` for choosing among existing `iChannel0`..`iChannel4` sources.

## Test Plan

Focused tests:

- Parser accepts every new control type and rejects invalid combinations.
- YAML defaults deserialize/serialize for int and vec2 values.
- Normalization clamps/snaps/reorders values correctly.
- Renderer uploads int, vec2, log, angle, select, and channel values correctly.
- Transpiler strips declarations and emits correct defines/fallbacks.
- Settings UI helpers normalize effective values consistently.
- Shader assistant context includes syntax and when-to-use guidance.

Final verification:

```bash
make checkall
```

If a full-check failure is unrelated, isolate and report it rather than hiding it.
