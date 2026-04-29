# Shader Control Types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add int, select, vec2, point, range, log slider, angle, and channel shader uniform controls, then document them and teach the shader assistant when to use them.

**Architecture:** Extend the existing explicit `// control` pipeline. `par-term-config` parses typed controls and typed metadata values, `par-term-render` uploads normalized values through bounded custom-control uniform slots and rewrites attached uniforms into macros, and `par-term-settings-ui` renders widgets using the same normalization rules used for Save Defaults and renderer upload.

**Tech Stack:** Rust 2024, serde/serde_yaml_ng, egui, wgpu uniform buffers, GLSL-to-WGSL transpilation, cargo tests, `make checkall`.

---

## File Map

- Modify `par-term-config/src/types/shader.rs`
  - Add `ShaderVec2Value` if useful or store `[f32; 2]` directly in `ShaderUniformValue::Vec2`.
  - Add `ShaderUniformValue::Int(i32)` and `ShaderUniformValue::Vec2([f32; 2])`.
  - Preserve existing color hex serialization.
- Modify `par-term-config/src/shader_controls.rs`
  - Add typed control variants, parser helpers, fallback values, and parser tests.
- Modify `par-term-config/src/lib.rs`
  - Re-export any new public types needed by renderer/UI.
- Modify `par-term-render/src/custom_shader_renderer/types.rs`
  - Add int and vec2 control slots.
  - Normalize and upload all new control kinds.
- Modify `par-term-render/src/custom_shader_renderer/transpiler.rs`
  - Replace new controlled uniforms with `#define` macros and safe fallbacks.
- Modify `par-term-settings-ui/src/background_tab/shader_settings.rs`
  - Add UI widgets and normalization for all new kinds.
  - Keep override reset/pruning behavior.
- Modify `src/ai_inspector/shader_context/context_builder.rs`
  - Update assistant guidance block with syntax and when-to-use recommendations.
- Modify `src/ai_inspector/shader_context/tests.rs` and `tests/shader_context_tests.rs`
  - Assert expanded guidance appears.
- Modify `docs/CUSTOM_SHADERS.md`, `docs/ASSISTANT_PANEL.md`, `CHANGELOG.md`
  - Document syntax, defaults, limits, and use cases.

---

## Task 1: Config model and parser

**Files:**
- Modify: `par-term-config/src/types/shader.rs`
- Modify: `par-term-config/src/shader_controls.rs`
- Modify: `par-term-config/src/lib.rs`

- [ ] **Step 1: Add failing parser/model tests**

Add tests to `par-term-config/src/shader_controls.rs` in the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn parses_new_numeric_control_types() {
    let source = r#"
// control slider min=0.01 max=100 step=0.01 scale=log label="Frequency"
uniform float iFrequency;
// control int min=1 max=12 step=1 label="Octaves"
uniform int iOctaves;
// control select options="soft,hard,screen,add" label="Blend Mode"
uniform int iBlendMode;
// control vec2 min=-1 max=1 step=0.01 label="Flow"
uniform vec2 iFlow;
// control point label="Origin"
uniform vec2 iOrigin;
// control range min=0 max=1 step=0.01 label="Glow Range"
uniform vec2 iGlowRange;
// control angle unit=degrees label="Rotation"
uniform float iRotation;
// control channel options="0,1,2,3,4" label="Source Channel"
uniform int iSourceChannel;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.warnings, Vec::<ShaderControlWarning>::new());
    assert_eq!(result.controls.len(), 8);
    assert!(matches!(
        result.controls[0].kind,
        ShaderControlKind::Slider { scale: SliderScale::Log, .. }
    ));
    assert!(matches!(
        result.controls[1].kind,
        ShaderControlKind::Int { min: 1, max: 12, step: 1, .. }
    ));
    assert!(matches!(
        &result.controls[2].kind,
        ShaderControlKind::Select { options, .. } if options == &["soft", "hard", "screen", "add"]
    ));
    assert!(matches!(result.controls[3].kind, ShaderControlKind::Vec2 { .. }));
    assert!(matches!(result.controls[4].kind, ShaderControlKind::Point { .. }));
    assert!(matches!(result.controls[5].kind, ShaderControlKind::Range { .. }));
    assert!(matches!(
        result.controls[6].kind,
        ShaderControlKind::Angle { unit: AngleUnit::Degrees, .. }
    ));
    assert!(matches!(
        &result.controls[7].kind,
        ShaderControlKind::Channel { options, .. } if options == &[0, 1, 2, 3, 4]
    ));
}

#[test]
fn warns_and_skips_invalid_new_control_types() {
    let source = r#"
// control slider min=0 max=10 step=1 scale=log
uniform float iBadLog;
// control int min=10 max=1 step=1
uniform int iBadInt;
// control select options=""
uniform int iBadSelect;
// control vec2 min=0 max=1 step=0
uniform vec2 iBadVec2;
// control angle unit=turns
uniform float iBadAngle;
// control channel options="0,9"
uniform int iBadChannel;
// control point alpha=true
uniform vec2 iPoint;
"#;

    let result = parse_shader_controls(source);

    assert_eq!(result.controls.len(), 1);
    assert_eq!(result.controls[0].name, "iPoint");
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Log slider")));
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Int control")));
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Select")));
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Vec2")));
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Angle")));
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Channel")));
    assert!(result.warnings.iter().any(|warning| warning.message.contains("Unknown point control field `alpha`")));
}

#[test]
fn fallback_values_for_new_control_types_are_stable() {
    assert_eq!(
        fallback_value_for_control(&ShaderControl {
            name: "iOctaves".to_string(),
            kind: ShaderControlKind::Int { min: 2, max: 8, step: 1, label: None },
        }),
        ShaderUniformValue::Int(2)
    );
    assert_eq!(
        fallback_value_for_control(&ShaderControl {
            name: "iBlend".to_string(),
            kind: ShaderControlKind::Select { options: vec!["a".to_string(), "b".to_string()], label: None },
        }),
        ShaderUniformValue::Int(0)
    );
    assert_eq!(
        fallback_value_for_control(&ShaderControl {
            name: "iFlow".to_string(),
            kind: ShaderControlKind::Vec2 { min: -1.0, max: 1.0, step: 0.1, label: None },
        }),
        ShaderUniformValue::Vec2([-1.0, -1.0])
    );
    assert_eq!(
        fallback_value_for_control(&ShaderControl {
            name: "iOrigin".to_string(),
            kind: ShaderControlKind::Point { label: None },
        }),
        ShaderUniformValue::Vec2([0.5, 0.5])
    );
    assert_eq!(
        fallback_value_for_control(&ShaderControl {
            name: "iRange".to_string(),
            kind: ShaderControlKind::Range { min: 0.2, max: 0.8, step: 0.01, label: None },
        }),
        ShaderUniformValue::Vec2([0.2, 0.8])
    );
}
```

Add tests to `par-term-config/src/types/shader.rs` tests section or its existing shader config tests:

```rust
#[test]
fn shader_uniform_values_parse_int_float_and_vec2_defaults() {
    let metadata: ShaderMetadata = serde_yaml_ng::from_str(
        r#"
defaults:
  uniforms:
    iCount: 4
    iAmount: 0.5
    iOffset: [0.25, -0.5]
"#,
    )
    .expect("metadata parses");

    assert_eq!(metadata.defaults.uniforms.get("iCount"), Some(&ShaderUniformValue::Int(4)));
    assert_eq!(metadata.defaults.uniforms.get("iAmount"), Some(&ShaderUniformValue::Float(0.5)));
    assert_eq!(metadata.defaults.uniforms.get("iOffset"), Some(&ShaderUniformValue::Vec2([0.25, -0.5])));
}
```

- [ ] **Step 2: Run failing config tests**

Run:

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-config shader_controls::tests::parses_new_numeric_control_types -- --nocapture
cargo test -p par-term-config shader_uniform_values_parse_int_float_and_vec2_defaults -- --nocapture
```

Expected: fail to compile because `SliderScale`, `AngleUnit`, and new `ShaderControlKind` / `ShaderUniformValue` variants do not exist yet.

- [ ] **Step 3: Implement config types and parsing**

Make these concrete type changes in `par-term-config/src/shader_controls.rs`:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SliderScale {
    Linear,
    Log,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AngleUnit {
    Degrees,
    Radians,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderControlKind {
    Slider { min: f32, max: f32, step: f32, scale: SliderScale, label: Option<String> },
    Checkbox { label: Option<String> },
    Color { alpha: bool, label: Option<String> },
    Int { min: i32, max: i32, step: i32, label: Option<String> },
    Select { options: Vec<String>, label: Option<String> },
    Vec2 { min: f32, max: f32, step: f32, label: Option<String> },
    Point { label: Option<String> },
    Range { min: f32, max: f32, step: f32, label: Option<String> },
    Angle { unit: AngleUnit, label: Option<String> },
    Channel { options: Vec<i32>, label: Option<String> },
}
```

Update existing matches for `Slider` and `Checkbox` to include the new fields. Existing slider controls without `scale` or `label` must parse as `scale: SliderScale::Linear, label: None`; existing `// control checkbox` must parse as `Checkbox { label: None }`.

Add helpers in `shader_controls.rs` with these exact signatures and behavior:

```rust
fn parse_required_f32(key_values: &BTreeMap<String, String>, key: &str) -> Result<f32, String>;
fn parse_required_i32(key_values: &BTreeMap<String, String>, key: &str) -> Result<i32, String>;
fn parse_optional_i32(key_values: &BTreeMap<String, String>, key: &str, default: i32) -> Result<i32, String>;
fn parse_quoted_csv(value: Option<&String>) -> Result<Vec<String>, String>;
fn parse_channel_options(value: Option<&String>) -> Result<Vec<i32>, String>;
fn validate_float_bounds(control_name: &str, min: f32, max: f32, step: f32) -> Result<(), String>;
```

Required behavior:

- `parse_required_f32` returns a finite `f32` or a message containing the missing/invalid key.
- `parse_required_i32` returns an `i32` or a message containing the missing/invalid key.
- `parse_optional_i32` returns `default` when the key is absent and rejects invalid present values.
- `parse_quoted_csv` strips surrounding quotes, splits on comma, trims labels, and rejects empty option lists or empty labels.
- `parse_channel_options` defaults to `[0, 1, 2, 3, 4]`, otherwise parses quoted comma-separated integers and rejects values outside `0..=4`.
- `validate_float_bounds` rejects non-finite values, `max < min`, and `step <= 0.0`.

In `par-term-config/src/types/shader.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderUniformValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Color(ShaderColorValue),
    Vec2([f32; 2]),
}
```

Update `from_yaml_value`:

- YAML bool -> `Bool`.
- YAML number with exact integer representation -> `Int`.
- YAML finite non-integer number -> `Float`.
- YAML string starting `#` -> `Color`.
- YAML sequence length 2 -> `Vec2` with finite f32 components.
- YAML sequence length 3 or 4 -> `Color`.
- Other strings and sequence lengths are invalid.

Update serialization:

- `Int` serializes as integer.
- `Vec2` serializes as a two-element sequence.
- Existing `Color` still serializes as hex.

- [ ] **Step 4: Run config tests**

Run:

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-config shader_controls -- --nocapture
cargo test -p par-term-config shader_metadata -- --nocapture
cargo test -p par-term-config shader_config -- --nocapture
```

Expected: all pass.

- [ ] **Step 5: Commit config work**

```bash
git add par-term-config/src/types/shader.rs par-term-config/src/shader_controls.rs par-term-config/src/lib.rs
git commit -m "feat(config): parse expanded shader controls"
```

---

## Task 2: Renderer upload and transpiler mappings

**Files:**
- Modify: `par-term-render/src/custom_shader_renderer/types.rs`
- Modify: `par-term-render/src/custom_shader_renderer/transpiler.rs`

- [ ] **Step 1: Add failing renderer/transpiler tests**

Add tests to `par-term-render/src/custom_shader_renderer/types.rs` in `custom_uniform_tests`:

```rust
#[test]
fn uploads_int_vec2_angle_and_channel_control_slots() {
    use par_term_config::{AngleUnit, ShaderControl, ShaderControlKind, ShaderUniformValue, SliderScale};
    use std::collections::BTreeMap;

    let controls = vec![
        ShaderControl { name: "iFrequency".to_string(), kind: ShaderControlKind::Slider { min: 0.01, max: 100.0, step: 0.01, scale: SliderScale::Log, label: None } },
        ShaderControl { name: "iRotation".to_string(), kind: ShaderControlKind::Angle { unit: AngleUnit::Degrees, label: None } },
        ShaderControl { name: "iOctaves".to_string(), kind: ShaderControlKind::Int { min: 1, max: 12, step: 1, label: None } },
        ShaderControl { name: "iBlend".to_string(), kind: ShaderControlKind::Select { options: vec!["soft".to_string(), "hard".to_string()], label: None } },
        ShaderControl { name: "iChannel".to_string(), kind: ShaderControlKind::Channel { options: vec![1, 4], label: None } },
        ShaderControl { name: "iFlow".to_string(), kind: ShaderControlKind::Vec2 { min: -1.0, max: 1.0, step: 0.01, label: None } },
        ShaderControl { name: "iOrigin".to_string(), kind: ShaderControlKind::Point { label: None } },
        ShaderControl { name: "iRange".to_string(), kind: ShaderControlKind::Range { min: 0.0, max: 1.0, step: 0.01, label: None } },
    ];
    let mut values = BTreeMap::new();
    values.insert("iFrequency".to_string(), ShaderUniformValue::Float(3.0));
    values.insert("iRotation".to_string(), ShaderUniformValue::Float(180.0));
    values.insert("iOctaves".to_string(), ShaderUniformValue::Int(20));
    values.insert("iBlend".to_string(), ShaderUniformValue::Int(1));
    values.insert("iChannel".to_string(), ShaderUniformValue::Int(9));
    values.insert("iFlow".to_string(), ShaderUniformValue::Vec2([2.0, -2.0]));
    values.insert("iOrigin".to_string(), ShaderUniformValue::Vec2([1.5, -0.5]));
    values.insert("iRange".to_string(), ShaderUniformValue::Vec2([0.8, 0.2]));

    let uniforms = CustomShaderControlUniforms::from_controls(&controls, &values);

    assert_eq!(uniforms.float_values[0][0], 3.0);
    assert!((uniforms.float_values[0][1] - std::f32::consts::PI).abs() < 0.0001);
    assert_eq!(uniforms.int_values[0][0], 12);
    assert_eq!(uniforms.int_values[0][1], 1);
    assert_eq!(uniforms.int_values[0][2], 1);
    assert_eq!(uniforms.vec2_values[0], [1.0, -1.0, 1.0, 0.0]);
    assert_eq!(uniforms.vec2_values[1], [1.0, 0.0, 1.0, 0.0]);
    assert_eq!(uniforms.vec2_values[2], [0.2, 0.8, 1.0, 0.0]);
}
```

Add tests to `par-term-render/src/custom_shader_renderer/transpiler.rs`:

```rust
#[test]
fn controlled_uniform_new_declarations_are_replaced_with_custom_macros() {
    let source = r#"
// control int min=1 max=12 step=1
uniform int iOctaves;
// control select options="soft,hard"
uniform int iBlend;
// control vec2 min=-1 max=1 step=0.01
uniform vec2 iFlow;
// control point
uniform vec2 iOrigin;
// control range min=0 max=1 step=0.01
uniform vec2 iRange;
// control angle unit=degrees
uniform float iRotation;
// control channel options="0,4"
uniform int iSource;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(iFlow + iOrigin + iRange, float(iOctaves + iBlend + iSource) + iRotation);
}
"#;

    let preprocessed = preprocess_custom_control_uniforms(source);

    assert!(!preprocessed.contains("uniform int iOctaves;"));
    assert!(!preprocessed.contains("uniform vec2 iFlow;"));
    assert!(preprocessed.contains("#define iOctaves iCustomIntUniforms[0].x"));
    assert!(preprocessed.contains("#define iBlend iCustomIntUniforms[0].y"));
    assert!(preprocessed.contains("#define iFlow iCustomVec2Uniforms[0].xy"));
    assert!(preprocessed.contains("#define iOrigin iCustomVec2Uniforms[1].xy"));
    assert!(preprocessed.contains("#define iRange iCustomVec2Uniforms[2].xy"));
    assert!(preprocessed.contains("#define iRotation iCustomFloatUniforms[0].x"));
    assert!(preprocessed.contains("#define iSource iCustomIntUniforms[0].z"));
}
```

- [ ] **Step 2: Run failing renderer tests**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-render custom_shader_renderer::types::custom_uniform_tests::uploads_int_vec2_angle_and_channel_control_slots -- --nocapture
cargo test -p par-term-render custom_shader_renderer::transpiler::tests::controlled_uniform_new_declarations_are_replaced_with_custom_macros -- --nocapture
```

Expected: fail to compile until Task 2 implementation exists.

- [ ] **Step 3: Implement uniform slots and normalization**

In `types.rs`:

- Add constants:

```rust
pub(crate) const MAX_CUSTOM_INT_UNIFORMS: usize = 16;
pub(crate) const MAX_CUSTOM_VEC2_UNIFORMS: usize = 16;
```

- Extend `CustomShaderControlUniforms`:

```rust
pub int_values: [[i32; 4]; 4],
pub vec2_values: [[f32; 4]; 16],
```

- Initialize both fields in `from_controls`.
- Add local helpers with these exact signatures:

```rust
fn snap_i32_to_step(value: i32, min: i32, max: i32, step: i32) -> i32;
fn normalized_int_value(control: &ShaderControlKind, value: Option<&ShaderUniformValue>) -> i32;
fn normalized_vec2_value(control: &ShaderControlKind, value: Option<&ShaderUniformValue>) -> [f32; 2];
fn normalized_float_value(control: &ShaderControlKind, value: Option<&ShaderUniformValue>) -> f32;
```

Required behavior:

- `snap_i32_to_step` clamps to `min..=max`, snaps to the nearest step offset from `min`, and uses `step.max(1)` defensively.
- `normalized_int_value` handles int/select/channel controls, including select index clamping and channel fallback to `options[0]`.
- `normalized_vec2_value` handles vec2/point/range controls, including point `0..=1` clamping and range low/high sorting.
- `normalized_float_value` handles slider and angle controls, including degree-to-radian upload with `std::f32::consts::PI / 180.0`.

Update size assertion to the new exact size after compiling once. The expected layout with `float_values` 64 bytes, `bool_values` 64 bytes, `color_values` 256 bytes, `int_values` 64 bytes, and `vec2_values` 256 bytes is 704 bytes.

- [ ] **Step 4: Implement transpiler macros and fallbacks**

In `transpiler.rs`:

- Extend `ActiveCustomControl` with `Int { index: usize }` and `Vec2 { index: usize }`.
- Count active controls by slot class:
  - Slider and Angle -> float.
  - Int, Select, Channel -> int.
  - Vec2, Point, Range -> vec2.
- Extend `active_custom_control_define`:

```rust
(ActiveCustomControl::Int { index }, "int") => Some(format!(
    "#define {} iCustomIntUniforms[{}].{}\n",
    name, index / 4, ["x", "y", "z", "w"][index % 4]
)),
(ActiveCustomControl::Vec2 { index }, "vec2") => Some(format!(
    "#define {} iCustomVec2Uniforms[{}].xy\n",
    name, index
)),
```

- Add safe fallback literals:
  - `int` -> `0`
  - `vec2` -> `vec2(0.0)` unless control-specific fallback is available.
  - point fallback -> `vec2(0.5)`.
  - range fallback -> `vec2(min, max)`.
  - angle fallback -> `0.0`.
- Update wrapper GLSL uniform block:

```glsl
ivec4 iCustomIntUniforms[4];
vec4 iCustomVec2Uniforms[16];
```

- Strip attached declarations for new supported control/uniform pairs.

- [ ] **Step 5: Run renderer tests**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-render custom_shader_renderer -- --nocapture
```

Expected: all custom shader renderer tests pass.

- [ ] **Step 6: Commit renderer work**

```bash
git add par-term-render/src/custom_shader_renderer/types.rs par-term-render/src/custom_shader_renderer/transpiler.rs
git commit -m "feat(render): upload expanded shader controls"
```

---

## Task 3: Settings UI widgets and shared normalization

**Files:**
- Modify: `par-term-settings-ui/src/background_tab/shader_settings.rs`

- [ ] **Step 1: Add failing settings normalization tests**

Add tests in the existing tests module in `shader_settings.rs`:

```rust
#[test]
fn shader_uniform_override_normalizes_int_select_channel_and_angle_values() {
    let int_control = par_term_config::ShaderControl {
        name: "iOctaves".to_string(),
        kind: par_term_config::ShaderControlKind::Int { min: 1, max: 12, step: 2, label: None },
    };
    let select_control = par_term_config::ShaderControl {
        name: "iBlend".to_string(),
        kind: par_term_config::ShaderControlKind::Select { options: vec!["a".to_string(), "b".to_string()], label: None },
    };
    let channel_control = par_term_config::ShaderControl {
        name: "iChannel".to_string(),
        kind: par_term_config::ShaderControlKind::Channel { options: vec![1, 4], label: None },
    };
    let angle_control = par_term_config::ShaderControl {
        name: "iRotation".to_string(),
        kind: par_term_config::ShaderControlKind::Angle { unit: par_term_config::AngleUnit::Degrees, label: None },
    };
    let mut override_config = par_term_config::ShaderConfig::default();
    override_config.uniforms.insert("iOctaves".to_string(), par_term_config::ShaderUniformValue::Int(20));
    override_config.uniforms.insert("iBlend".to_string(), par_term_config::ShaderUniformValue::Int(8));
    override_config.uniforms.insert("iChannel".to_string(), par_term_config::ShaderUniformValue::Int(2));
    override_config.uniforms.insert("iRotation".to_string(), par_term_config::ShaderUniformValue::Float(450.0));

    assert_eq!(normalized_effective_uniform_value(&int_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Int(11));
    assert_eq!(normalized_effective_uniform_value(&select_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Int(1));
    assert_eq!(normalized_effective_uniform_value(&channel_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Int(1));
    assert_eq!(normalized_effective_uniform_value(&angle_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Float(450.0));
}

#[test]
fn shader_uniform_override_normalizes_vec2_point_and_range_values() {
    let vec2_control = par_term_config::ShaderControl {
        name: "iFlow".to_string(),
        kind: par_term_config::ShaderControlKind::Vec2 { min: -1.0, max: 1.0, step: 0.1, label: None },
    };
    let point_control = par_term_config::ShaderControl {
        name: "iOrigin".to_string(),
        kind: par_term_config::ShaderControlKind::Point { label: None },
    };
    let range_control = par_term_config::ShaderControl {
        name: "iRange".to_string(),
        kind: par_term_config::ShaderControlKind::Range { min: 0.0, max: 1.0, step: 0.01, label: None },
    };
    let mut override_config = par_term_config::ShaderConfig::default();
    override_config.uniforms.insert("iFlow".to_string(), par_term_config::ShaderUniformValue::Vec2([2.0, -2.0]));
    override_config.uniforms.insert("iOrigin".to_string(), par_term_config::ShaderUniformValue::Vec2([1.5, -0.5]));
    override_config.uniforms.insert("iRange".to_string(), par_term_config::ShaderUniformValue::Vec2([0.8, 0.2]));

    assert_eq!(normalized_effective_uniform_value(&vec2_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Vec2([1.0, -1.0]));
    assert_eq!(normalized_effective_uniform_value(&point_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Vec2([1.0, 0.0]));
    assert_eq!(normalized_effective_uniform_value(&range_control, Some(&override_config), None), par_term_config::ShaderUniformValue::Vec2([0.2, 0.8]));
}
```

- [ ] **Step 2: Run failing settings tests**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-settings-ui shader_uniform_override_normalizes_int_select_channel_and_angle_values -- --nocapture
cargo test -p par-term-settings-ui shader_uniform_override_normalizes_vec2_point_and_range_values -- --nocapture
```

Expected: fail until settings normalization handles new variants.

- [ ] **Step 3: Implement normalization helpers**

In `normalize_uniform_value_for_control`, handle:

- Slider accepts `Float` and compatible `Int`, clamps to range.
- Int accepts `Int` and integral `Float`, clamps and snaps.
- Select accepts `Int` and integral `Float`, clamps to `0..options.len()-1`.
- Channel accepts `Int` and integral `Float`, returns stored value only if in options, otherwise first option.
- Vec2 accepts `Vec2`, clamps components.
- Point accepts `Vec2`, clamps components to `0..1`.
- Range accepts `Vec2`, clamps and sorts low/high.
- Angle accepts `Float` or `Int`, stores declared UI unit unchanged.

Use small helpers near `normalized_shader_color_value` with these exact signatures:

```rust
fn integral_uniform_value(value: &par_term_config::ShaderUniformValue) -> Option<i32>;
fn float_uniform_value(value: &par_term_config::ShaderUniformValue) -> Option<f32>;
fn snap_i32_to_step(value: i32, min: i32, max: i32, step: i32) -> i32;
fn normalize_vec2_components(value: [f32; 2], min: f32, max: f32) -> [f32; 2];
```

Required behavior:

- `integral_uniform_value` accepts `Int` and finite integral `Float` values.
- `float_uniform_value` accepts `Float` and `Int` values.
- `snap_i32_to_step` clamps to `min..=max`, snaps to the nearest step offset from `min`, and uses `step.max(1)` defensively.
- `normalize_vec2_components` clamps both components to `min..=max`.

- [ ] **Step 4: Implement UI widgets**

In the `ui.horizontal(|ui| match &control.kind { ... })` block:

- Slider: keep existing slider, add `.logarithmic(matches!(scale, SliderScale::Log))`, and use `label.as_deref().unwrap_or(&control.name)` for `.text(...)`.
- Checkbox: update match to `Checkbox { label }` and display label.
- Int: use `egui::Slider::new(&mut int_value, *min..=*max).step_by(*step as f64).text(label_text)`.
- Select: use `egui::ComboBox::from_label(label_text)` and `ui.selectable_value(&mut selected, index as i32, option)`.
- Vec2: display label, two `DragValue`s or sliders for x/y with `.speed(*step as f64)` and clamp range.
- Point: display label, two normalized sliders `0.0..=1.0`, and a `Center` button setting `[0.5, 0.5]`.
- Range: display label, two sliders over `min..=max`; after editing, sort low/high before saving.
- Angle: display label, `DragValue` with suffix `°` for degrees or ` rad` for radians.
- Channel: combo box labels `iChannel0` through `iChannel4` for allowed options.

Every changed widget must call `set_shader_uniform_override(...)`, set `settings.has_changes = true` through that helper, and set `*changes_this_frame = true`. Reset uses `show_reset_button` and `clear_shader_uniform_override`.

- [ ] **Step 5: Run settings tests**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-settings-ui background_tab::shader_settings::tests -- --nocapture
```

Expected: all settings shader tests pass.

- [ ] **Step 6: Commit settings work**

```bash
git add par-term-settings-ui/src/background_tab/shader_settings.rs
git commit -m "feat(settings): add expanded shader control widgets"
```

---

## Task 4: Documentation and assistant guidance

**Files:**
- Modify: `docs/CUSTOM_SHADERS.md`
- Modify: `docs/ASSISTANT_PANEL.md`
- Modify: `src/ai_inspector/shader_context/context_builder.rs`
- Modify: `src/ai_inspector/shader_context/tests.rs`
- Modify: `tests/shader_context_tests.rs`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add failing assistant context assertions**

Update shader context tests to assert the prompt contains the new terms. In `tests/shader_context_tests.rs` and `src/ai_inspector/shader_context/tests.rs`, add assertions near existing shader-control assertions:

```rust
assert!(context.contains("// control int min=1 max=12 step=1"));
assert!(context.contains("// control select options=\"soft,hard,screen,add\""));
assert!(context.contains("// control vec2 min=-1 max=1 step=0.01"));
assert!(context.contains("// control point label=\"Origin\""));
assert!(context.contains("// control range min=0 max=1 step=0.01"));
assert!(context.contains("scale=log"));
assert!(context.contains("// control angle unit=degrees"));
assert!(context.contains("// control channel options=\"0,1,2,3,4\""));
assert!(context.contains("Use `select` for discrete shader modes"));
assert!(context.contains("Use `channel` only to choose among existing `iChannel0`..`iChannel4` sources"));
```

- [ ] **Step 2: Run failing shader context tests**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test shader_context -- --nocapture
```

Expected: fail until context builder is updated.

- [ ] **Step 3: Update assistant guidance**

In `src/ai_inspector/shader_context/context_builder.rs`, replace the Shader Uniform Controls example with a compact full example containing all supported controls and guidance bullets:

```text
- Use `slider` for continuous linear amounts.
- Use `slider scale=log` for frequency/exposure/gain/radius values spanning orders of magnitude.
- Use `int` for counts, iterations, samples, octaves, and quantization levels.
- Use `select` for discrete shader modes; the shader receives a zero-based option index.
- Use `vec2` for directions, offsets, scales, and velocities.
- Use `point` for normalized origins/focal points in 0..1 UV space.
- Use `range` for min/max thresholds and bands; the shader receives vec2(low, high).
- Use `angle` for rotation/direction; defaults are authored in the declared unit and shaders receive radians.
- Use `channel` only to choose among existing `iChannel0`..`iChannel4` sources; it does not create texture bindings.
```

- [ ] **Step 4: Update user documentation**

In `docs/CUSTOM_SHADERS.md`, expand the Shader Uniform Controls section with:

- Syntax table for all controls.
- Defaults example from the design spec.
- Value type rules for ints and vec2 arrays.
- Limits: 16 float, 16 int, 16 vec2, 16 bool, 16 color.
- Channel note: selector only, no resource binding.
- Angle note: config unit -> shader radians.

In `docs/ASSISTANT_PANEL.md`, update the shader assistant guidance summary to say it knows how to recommend slider, log slider, checkbox, color, int, select, vec2, point, range, angle, and channel controls.

In `CHANGELOG.md`, add an Unreleased bullet:

```markdown
- Added expanded background shader uniform controls: int sliders, select dropdowns, vec2/point/range controls, log sliders, angle controls, and channel selectors.
```

- [ ] **Step 5: Run docs/guidance tests**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test shader_context -- --nocapture
```

Expected: pass.

- [ ] **Step 6: Commit docs/guidance work**

```bash
git add docs/CUSTOM_SHADERS.md docs/ASSISTANT_PANEL.md src/ai_inspector/shader_context/context_builder.rs src/ai_inspector/shader_context/tests.rs tests/shader_context_tests.rs CHANGELOG.md
git commit -m "docs: document expanded shader controls"
```

---

## Task 5: Integration verification and cleanup

**Files:**
- Review all modified files from Tasks 1-4.

- [ ] **Step 1: Run focused crate checks**

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-control-types
cargo test -p par-term-config shader_controls -- --nocapture
cargo test -p par-term-render custom_shader_renderer -- --nocapture
cargo test -p par-term-settings-ui background_tab::shader_settings::tests -- --nocapture
cargo test shader_context -- --nocapture
```

Expected: all pass.

- [ ] **Step 2: Run formatting**

```bash
cargo fmt --all
```

Expected: completes without output.

- [ ] **Step 3: Inspect diff for accidental scope creep**

```bash
git diff --stat main...HEAD
git diff --check
```

Expected: only planned shader-control, docs, and guidance files changed; `git diff --check` reports no whitespace errors.

- [ ] **Step 4: Run final verification**

```bash
make checkall
```

Expected: `All quality checks passed!`

- [ ] **Step 5: Commit formatting fixes if any**

If `cargo fmt --all` changed files, commit them:

```bash
git status --short
git add par-term-config/src par-term-render/src par-term-settings-ui/src src/ai_inspector docs CHANGELOG.md
git commit -m "style: format shader control types"
```

If `git status --short` is empty, do not create a commit.

- [ ] **Step 6: Report completion**

Report:

- Branch: `feature/shader-control-types`
- Worktree: `.worktrees/shader-control-types`
- Commits created
- Final verification command and result
