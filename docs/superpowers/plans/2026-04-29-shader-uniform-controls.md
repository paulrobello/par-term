# Shader Uniform Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add shader-comment-driven float slider and bool checkbox controls for custom background shader uniforms.

**Architecture:** Parse `// control ...` comments in `par-term-config`, store custom uniform defaults/overrides in the existing `ShaderConfig`, render controls in `par-term-settings-ui`, and upload resolved values through a separate fixed-size custom uniform buffer in `par-term-render`. Controlled GLSL uniform declarations remain explicit in shader source, but the transpiler strips those declarations and maps each name to a slot in the custom uniform block.

**Tech Stack:** Rust 2024, serde/serde_yaml_ng, egui, wgpu, naga GLSL→WGSL transpilation, cargo/make verification.

---

## Working Directory

Use the isolated worktree created for this feature:

```bash
cd /Users/probello/Repos/par-term/.worktrees/shader-uniform-controls
```

Baseline already verified with:

```bash
make test
```

Expected baseline: all non-ignored tests pass.

---

## File Map

- Create `par-term-config/src/shader_controls.rs`
  - Owns parsing `// control ...` comments and nearby `uniform` declarations.
  - Exposes control metadata and warnings to settings UI and renderer.
- Modify `par-term-config/src/types/shader.rs`
  - Add `ShaderUniformValue` and `ShaderConfig::uniforms`.
- Modify `par-term-config/src/shader_config.rs`
  - Merge metadata/user custom uniform maps into `ResolvedShaderConfig`.
- Modify `par-term-config/src/lib.rs`
  - Export shader control parser/types.
- Modify `par-term-config/src/shader_metadata/parsing.rs` and `par-term-config/src/shader_metadata/mod.rs`
  - Add tests for `defaults.uniforms` YAML round-tripping if needed.
- Modify `par-term-render/src/custom_shader_renderer/types.rs`
  - Add fixed-size custom uniform GPU block using `vec4`-aligned arrays.
- Modify `par-term-render/src/custom_shader_renderer/transpiler.rs`
  - Parse controls, strip controlled uniform declarations, generate `#define` mappings, and expose binding 13.
- Modify `par-term-render/src/custom_shader_renderer/pipeline.rs`
  - Add binding 13 to the bind group layout and bind group.
- Modify `par-term-render/src/custom_shader_renderer/mod.rs`
  - Store custom control layout/value map, create/upload custom uniform buffer, and update values at runtime.
- Modify `par-term-render/src/renderer/shaders/mod.rs`, `background.rs`, and `cursor.rs`
  - Thread custom uniform values for background shaders; pass empty values for cursor shaders.
- Modify `src/app/window_state/renderer_init.rs` and `src/app/window_manager/config_renderer_apply.rs`
  - Use resolved custom uniform maps when creating/updating renderers.
- Modify `par-term-settings-ui/src/background_tab/shader_settings.rs`
  - Add `Shader Controls` UI subsection.
- Modify `par-term-settings-ui/src/background_tab/shader_metadata.rs`
  - Include custom uniforms when saving defaults to shader metadata.
- Modify `docs/CUSTOM_SHADERS.md`
  - Document syntax, defaults, persistence, limits, and examples.

---

### Task 1: Add custom uniform value schema and control parser

**Files:**
- Create: `par-term-config/src/shader_controls.rs`
- Modify: `par-term-config/src/types/shader.rs`
- Modify: `par-term-config/src/lib.rs`
- Test: `par-term-config/src/shader_controls.rs`
- Test: `par-term-config/src/shader_metadata/mod.rs`

- [ ] **Step 1: Write failing parser and schema tests**

Add this new file with tests first. Keep implementation stubs minimal until Step 3.

```rust
// par-term-config/src/shader_controls.rs
use crate::types::ShaderUniformValue;

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderControlKind {
    Slider { min: f32, max: f32, step: f32 },
    Checkbox,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderControl {
    pub name: String,
    pub kind: ShaderControlKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderControlWarning {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaderControlParseResult {
    pub controls: Vec<ShaderControl>,
    pub warnings: Vec<ShaderControlWarning>,
}

pub fn parse_shader_controls(_source: &str) -> ShaderControlParseResult {
    ShaderControlParseResult::default()
}

pub fn fallback_value_for_control(control: &ShaderControl) -> ShaderUniformValue {
    match control.kind {
        ShaderControlKind::Slider { min, .. } => ShaderUniformValue::Float(min),
        ShaderControlKind::Checkbox => ShaderUniformValue::Bool(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_slider_attached_to_float_uniform() {
        let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"#;

        let result = parse_shader_controls(source);

        assert_eq!(result.warnings, Vec::<ShaderControlWarning>::new());
        assert_eq!(
            result.controls,
            vec![ShaderControl {
                name: "iGlow".to_string(),
                kind: ShaderControlKind::Slider {
                    min: 0.0,
                    max: 1.0,
                    step: 0.01,
                },
            }]
        );
    }

    #[test]
    fn parses_checkbox_attached_to_bool_uniform() {
        let source = r#"
// control checkbox
uniform bool iEnabled;
"#;

        let result = parse_shader_controls(source);

        assert!(result.warnings.is_empty());
        assert_eq!(
            result.controls,
            vec![ShaderControl {
                name: "iEnabled".to_string(),
                kind: ShaderControlKind::Checkbox,
            }]
        );
    }

    #[test]
    fn warns_and_skips_slider_missing_step() {
        let source = r#"
// control slider min=0 max=1
uniform float iGlow;
"#;

        let result = parse_shader_controls(source);

        assert!(result.controls.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("step"));
    }

    #[test]
    fn warns_and_skips_slider_on_bool_uniform() {
        let source = r#"
// control slider min=0 max=1 step=0.1
uniform bool iGlow;
"#;

        let result = parse_shader_controls(source);

        assert!(result.controls.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("uniform float"));
    }

    #[test]
    fn warns_and_skips_duplicate_uniform_control() {
        let source = r#"
// control slider min=0 max=1 step=0.1
uniform float iGlow;
// control slider min=0 max=2 step=0.2
uniform float iGlow;
"#;

        let result = parse_shader_controls(source);

        assert_eq!(result.controls.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("Duplicate"));
    }

    #[test]
    fn warns_and_skips_control_not_followed_by_uniform() {
        let source = r#"
// control checkbox
vec3 not_a_uniform;
"#;

        let result = parse_shader_controls(source);

        assert!(result.controls.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("uniform"));
    }
}
```

In `par-term-config/src/types/shader.rs`, add the enum and field expected by metadata/config tests:

```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShaderUniformValue {
    Float(f32),
    Bool(bool),
}
```

Add to `ShaderConfig`:

```rust
/// Custom shader uniform values for `// control ...` declarations.
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub uniforms: BTreeMap<String, ShaderUniformValue>,
```

Add a metadata test in `par-term-config/src/shader_metadata/mod.rs`:

```rust
#[test]
fn test_parse_metadata_with_custom_uniform_defaults() {
    let source = r#"/*! par-term shader metadata
name: "Controlled Shader"
defaults:
  uniforms:
    iGlow: 0.5
    iEnabled: true
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"#;

    let metadata = parse_shader_metadata(source).expect("Should parse metadata");
    assert_eq!(
        metadata.defaults.uniforms.get("iGlow"),
        Some(&crate::types::ShaderUniformValue::Float(0.5))
    );
    assert_eq!(
        metadata.defaults.uniforms.get("iEnabled"),
        Some(&crate::types::ShaderUniformValue::Bool(true))
    );
}
```

- [ ] **Step 2: Run tests and verify RED**

```bash
cargo test -p par-term-config shader_controls -- --nocapture
cargo test -p par-term-config test_parse_metadata_with_custom_uniform_defaults -- --nocapture
```

Expected: parser tests fail because `parse_shader_controls()` returns no controls. Metadata test may fail until the schema field/import is fully wired.

- [ ] **Step 3: Implement parser and exports**

Implement `parse_shader_controls()` in `par-term-config/src/shader_controls.rs` with this behavior:

```rust
use std::collections::{BTreeMap, HashSet};

fn parse_uniform_declaration(line: &str) -> Option<(&str, &str)> {
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

fn parse_key_values(tokens: &[&str]) -> BTreeMap<String, String> {
    tokens
        .iter()
        .filter_map(|token| {
            let (key, value) = token.split_once('=')?;
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

pub fn parse_shader_controls(source: &str) -> ShaderControlParseResult {
    let lines: Vec<&str> = source.lines().collect();
    let mut controls = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("// control ") else {
            continue;
        };

        let line_number = index + 1;
        let tokens: Vec<&str> = rest.split_whitespace().collect();
        let Some(control_type) = tokens.first().copied() else {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: "Control comment is missing a control type".to_string(),
            });
            continue;
        };

        let Some(next_line) = lines.get(index + 1) else {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: "Control comment must be immediately followed by a uniform declaration".to_string(),
            });
            continue;
        };

        let Some((uniform_type, uniform_name)) = parse_uniform_declaration(next_line) else {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: "Control comment must be immediately followed by a uniform declaration".to_string(),
            });
            continue;
        };

        if !seen.insert(uniform_name.to_string()) {
            warnings.push(ShaderControlWarning {
                line: line_number,
                message: format!("Duplicate control for uniform `{}` ignored", uniform_name),
            });
            continue;
        }

        let key_values = parse_key_values(&tokens[1..]);
        let kind = match control_type {
            "slider" => {
                if uniform_type != "float" {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!("Slider control for `{}` must attach to `uniform float`", uniform_name),
                    });
                    continue;
                }
                let parse_required = |key: &str| -> Result<f32, String> {
                    key_values
                        .get(key)
                        .ok_or_else(|| format!("missing `{}`", key))?
                        .parse::<f32>()
                        .map_err(|_| format!("invalid `{}`", key))
                };
                let min = match parse_required("min") {
                    Ok(value) => value,
                    Err(e) => {
                        warnings.push(ShaderControlWarning { line: line_number, message: format!("Slider `{}` {}", uniform_name, e) });
                        continue;
                    }
                };
                let max = match parse_required("max") {
                    Ok(value) => value,
                    Err(e) => {
                        warnings.push(ShaderControlWarning { line: line_number, message: format!("Slider `{}` {}", uniform_name, e) });
                        continue;
                    }
                };
                let step = match parse_required("step") {
                    Ok(value) => value,
                    Err(e) => {
                        warnings.push(ShaderControlWarning { line: line_number, message: format!("Slider `{}` {}", uniform_name, e) });
                        continue;
                    }
                };
                if max < min || step <= 0.0 {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!("Slider `{}` must have max >= min and step > 0", uniform_name),
                    });
                    continue;
                }
                ShaderControlKind::Slider { min, max, step }
            }
            "checkbox" => {
                if uniform_type != "bool" {
                    warnings.push(ShaderControlWarning {
                        line: line_number,
                        message: format!("Checkbox control for `{}` must attach to `uniform bool`", uniform_name),
                    });
                    continue;
                }
                ShaderControlKind::Checkbox
            }
            other => {
                warnings.push(ShaderControlWarning {
                    line: line_number,
                    message: format!("Unsupported control type `{}`", other),
                });
                continue;
            }
        };

        controls.push(ShaderControl {
            name: uniform_name.to_string(),
            kind,
        });
    }

    ShaderControlParseResult { controls, warnings }
}
```

Export from `par-term-config/src/lib.rs`:

```rust
pub mod shader_controls;
pub use shader_controls::{
    fallback_value_for_control, parse_shader_controls, ShaderControl, ShaderControlKind,
    ShaderControlParseResult, ShaderControlWarning,
};
```

- [ ] **Step 4: Run tests and verify GREEN**

```bash
cargo test -p par-term-config shader_controls -- --nocapture
cargo test -p par-term-config test_parse_metadata_with_custom_uniform_defaults -- --nocapture
```

Expected: all new tests pass.

- [ ] **Step 5: Commit Task 1**

```bash
git add par-term-config/src/shader_controls.rs par-term-config/src/types/shader.rs par-term-config/src/lib.rs par-term-config/src/shader_metadata/mod.rs
git commit -m "feat(config): parse shader uniform controls"
```

---

### Task 2: Resolve custom uniform values through the existing shader config chain

**Files:**
- Modify: `par-term-config/src/types/shader.rs`
- Modify: `par-term-config/src/shader_config.rs`
- Test: `par-term-config/src/shader_config.rs`

- [ ] **Step 1: Write failing resolution tests**

Add tests in the existing `#[cfg(test)] mod tests` in `par-term-config/src/shader_config.rs`:

```rust
#[test]
fn test_resolve_custom_uniforms_prefers_user_override() {
    use crate::types::ShaderUniformValue;
    use std::collections::BTreeMap;

    let config = Config::default();
    let mut user = ShaderConfig::default();
    user.uniforms.insert("iGlow".to_string(), ShaderUniformValue::Float(0.8));

    let mut metadata = ShaderMetadata::default();
    metadata.defaults.uniforms.insert("iGlow".to_string(), ShaderUniformValue::Float(0.5));

    let resolved = resolve_shader_config(Some(&user), Some(&metadata), &config);

    let expected = BTreeMap::from([("iGlow".to_string(), ShaderUniformValue::Float(0.8))]);
    assert_eq!(resolved.custom_uniforms, expected);
}

#[test]
fn test_resolve_custom_uniforms_uses_metadata_when_no_override() {
    use crate::types::ShaderUniformValue;
    use std::collections::BTreeMap;

    let config = Config::default();
    let mut metadata = ShaderMetadata::default();
    metadata.defaults.uniforms.insert("iGlow".to_string(), ShaderUniformValue::Float(0.5));
    metadata.defaults.uniforms.insert("iEnabled".to_string(), ShaderUniformValue::Bool(true));

    let resolved = resolve_shader_config(None, Some(&metadata), &config);

    let expected = BTreeMap::from([
        ("iGlow".to_string(), ShaderUniformValue::Float(0.5)),
        ("iEnabled".to_string(), ShaderUniformValue::Bool(true)),
    ]);
    assert_eq!(resolved.custom_uniforms, expected);
}

#[test]
fn test_shader_config_uniforms_yaml_roundtrip() {
    use crate::types::ShaderUniformValue;

    let yaml = r#"
uniforms:
  iGlow: 0.25
  iEnabled: true
"#;

    let parsed: ShaderConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(parsed.uniforms.get("iGlow"), Some(&ShaderUniformValue::Float(0.25)));
    assert_eq!(parsed.uniforms.get("iEnabled"), Some(&ShaderUniformValue::Bool(true)));

    let serialized = serde_yaml_ng::to_string(&parsed).unwrap();
    assert!(serialized.contains("uniforms:"));
    assert!(serialized.contains("iGlow"));
    assert!(serialized.contains("iEnabled"));
}
```

- [ ] **Step 2: Run tests and verify RED**

```bash
cargo test -p par-term-config resolve_custom_uniforms -- --nocapture
cargo test -p par-term-config test_shader_config_uniforms_yaml_roundtrip -- --nocapture
```

Expected: resolution tests fail because `ResolvedShaderConfig::custom_uniforms` does not exist yet.

- [ ] **Step 3: Implement resolution field and merge**

In `par-term-config/src/types/shader.rs`, import `BTreeMap` once near the top and add to `ResolvedShaderConfig`:

```rust
/// Resolved custom uniform values from per-shader overrides and metadata defaults.
pub custom_uniforms: BTreeMap<String, ShaderUniformValue>,
```

Add the default value in `impl Default for ResolvedShaderConfig`:

```rust
custom_uniforms: BTreeMap::new(),
```

In `par-term-config/src/shader_config.rs`, build the merged map before constructing `ResolvedShaderConfig`:

```rust
let mut custom_uniforms = meta_defaults
    .map(|m| m.uniforms.clone())
    .unwrap_or_default();
if let Some(user_uniforms) = user_override.map(|o| &o.uniforms) {
    for (name, value) in user_uniforms {
        custom_uniforms.insert(name.clone(), value.clone());
    }
}
```

Include it in `ResolvedShaderConfig { ... }`:

```rust
custom_uniforms,
```

For cursor shader base resolved config, set:

```rust
custom_uniforms: Default::default(),
```

- [ ] **Step 4: Run tests and verify GREEN**

```bash
cargo test -p par-term-config resolve_custom_uniforms -- --nocapture
cargo test -p par-term-config test_shader_config_uniforms_yaml_roundtrip -- --nocapture
cargo test -p par-term-config shader_config::tests -- --nocapture
```

Expected: all shader config tests pass.

- [ ] **Step 5: Commit Task 2**

```bash
git add par-term-config/src/types/shader.rs par-term-config/src/shader_config.rs
git commit -m "feat(config): resolve shader custom uniforms"
```

---

### Task 3: Add renderer custom uniform block and transpiler mapping

**Files:**
- Modify: `par-term-render/src/custom_shader_renderer/types.rs`
- Modify: `par-term-render/src/custom_shader_renderer/transpiler.rs`
- Modify: `par-term-render/src/custom_shader_renderer/pipeline.rs`
- Modify: `par-term-render/src/custom_shader_renderer/mod.rs`
- Test: `par-term-render/src/custom_shader_renderer/transpiler.rs`
- Test: `par-term-render/src/custom_shader_renderer/types.rs`

- [ ] **Step 1: Write failing renderer/transpiler tests**

Add tests in `par-term-render/src/custom_shader_renderer/transpiler.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_uniform_declarations_are_replaced_with_custom_block_macros() {
        let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow;
// control checkbox
uniform bool iEnabled;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), iEnabled ? 1.0 : 0.0);
}
"#;

        let preprocessed = preprocess_custom_control_uniforms(source);

        assert!(!preprocessed.contains("uniform float iGlow;"));
        assert!(!preprocessed.contains("uniform bool iEnabled;"));
        assert!(preprocessed.contains("#define iGlow iCustomFloatUniforms[0].x"));
        assert!(preprocessed.contains("#define iEnabled (iCustomBoolUniforms[0].x != 0)"));
    }

    #[test]
    fn transpiled_controlled_uniform_shader_mentions_custom_uniform_block() {
        let source = r#"
// control slider min=0 max=1 step=0.01
uniform float iGlow;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(vec3(iGlow), 1.0);
}
"#;

        let wgsl = transpile_glsl_to_wgsl_source(source, "controlled_test").unwrap();

        assert!(wgsl.contains("iCustomFloatUniforms") || wgsl.contains("custom"));
    }
}
```

Add tests in `par-term-render/src/custom_shader_renderer/types.rs`:

```rust
#[cfg(test)]
mod custom_uniform_tests {
    use super::*;

    #[test]
    fn custom_shader_control_uniforms_are_vec4_aligned() {
        assert_eq!(std::mem::size_of::<CustomShaderControlUniforms>(), 128);
    }

    #[test]
    fn builds_control_uniforms_with_clamped_slider_and_bool_slots() {
        use par_term_config::{ShaderControl, ShaderControlKind, ShaderUniformValue};
        use std::collections::BTreeMap;

        let controls = vec![
            ShaderControl {
                name: "iGlow".to_string(),
                kind: ShaderControlKind::Slider { min: 0.0, max: 1.0, step: 0.1 },
            },
            ShaderControl {
                name: "iEnabled".to_string(),
                kind: ShaderControlKind::Checkbox,
            },
        ];
        let values = BTreeMap::from([
            ("iGlow".to_string(), ShaderUniformValue::Float(2.0)),
            ("iEnabled".to_string(), ShaderUniformValue::Bool(true)),
        ]);

        let uniforms = CustomShaderControlUniforms::from_controls(&controls, &values);

        assert_eq!(uniforms.float_values[0][0], 1.0);
        assert_eq!(uniforms.bool_values[0][0], 1);
    }
}
```

- [ ] **Step 2: Run tests and verify RED**

```bash
cargo test -p par-term-render custom_uniform -- --nocapture
cargo test -p par-term-render controlled_uniform -- --nocapture
```

Expected: tests fail because the custom uniform block and preprocessor do not exist.

- [ ] **Step 3: Implement GPU data type**

In `par-term-render/src/custom_shader_renderer/types.rs`, add:

```rust
pub(crate) const MAX_CUSTOM_FLOAT_UNIFORMS: usize = 16;
pub(crate) const MAX_CUSTOM_BOOL_UNIFORMS: usize = 16;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CustomShaderControlUniforms {
    /// 16 float slots stored as 4 vec4s for std140 array alignment.
    pub float_values: [[f32; 4]; 4],
    /// 16 bool slots stored as 4 uvec4s/ivec4s for std140 array alignment.
    pub bool_values: [[u32; 4]; 4],
}

impl CustomShaderControlUniforms {
    pub(crate) fn from_controls(
        controls: &[par_term_config::ShaderControl],
        values: &std::collections::BTreeMap<String, par_term_config::ShaderUniformValue>,
    ) -> Self {
        let mut uniforms = Self::zeroed();
        let mut float_index = 0usize;
        let mut bool_index = 0usize;

        for control in controls {
            match control.kind {
                par_term_config::ShaderControlKind::Slider { min, max, .. } => {
                    if float_index >= MAX_CUSTOM_FLOAT_UNIFORMS {
                        continue;
                    }
                    let value = match values.get(&control.name) {
                        Some(par_term_config::ShaderUniformValue::Float(value)) => *value,
                        _ => min,
                    }
                    .clamp(min, max);
                    uniforms.float_values[float_index / 4][float_index % 4] = value;
                    float_index += 1;
                }
                par_term_config::ShaderControlKind::Checkbox => {
                    if bool_index >= MAX_CUSTOM_BOOL_UNIFORMS {
                        continue;
                    }
                    let value = matches!(
                        values.get(&control.name),
                        Some(par_term_config::ShaderUniformValue::Bool(true))
                    );
                    uniforms.bool_values[bool_index / 4][bool_index % 4] = u32::from(value);
                    bool_index += 1;
                }
            }
        }

        uniforms
    }
}

const _: () = assert!(
    std::mem::size_of::<CustomShaderControlUniforms>() == 128,
    "CustomShaderControlUniforms must be exactly 128 bytes"
);
```

- [ ] **Step 4: Implement transpiler custom-control preprocessing**

In `par-term-render/src/custom_shader_renderer/transpiler.rs`, add helpers before `glsl_wrapper_template()`:

```rust
fn custom_control_defines(source: &str) -> String {
    let result = par_term_config::parse_shader_controls(source);
    let mut float_index = 0usize;
    let mut bool_index = 0usize;
    let mut defines = String::new();

    for control in result.controls {
        match control.kind {
            par_term_config::ShaderControlKind::Slider { .. } => {
                if float_index < crate::custom_shader_renderer::types::MAX_CUSTOM_FLOAT_UNIFORMS {
                    defines.push_str(&format!(
                        "#define {} iCustomFloatUniforms[{}].{}\n",
                        control.name,
                        float_index / 4,
                        ["x", "y", "z", "w"][float_index % 4]
                    ));
                    float_index += 1;
                }
            }
            par_term_config::ShaderControlKind::Checkbox => {
                if bool_index < crate::custom_shader_renderer::types::MAX_CUSTOM_BOOL_UNIFORMS {
                    defines.push_str(&format!(
                        "#define {} (iCustomBoolUniforms[{}].{} != 0)\n",
                        control.name,
                        bool_index / 4,
                        ["x", "y", "z", "w"][bool_index % 4]
                    ));
                    bool_index += 1;
                }
            }
        }
    }

    defines
}

fn preprocess_custom_control_uniforms(source: &str) -> String {
    let parse_result = par_term_config::parse_shader_controls(source);
    let controlled_names: std::collections::HashSet<String> = parse_result
        .controls
        .iter()
        .map(|control| control.name.clone())
        .collect();

    let mut output = String::new();
    output.push_str(&custom_control_defines(source));

    for line in source.lines() {
        let trimmed = line.trim();
        let should_strip = controlled_names.iter().any(|name| {
            trimmed == format!("uniform float {};", name)
                || trimmed == format!("uniform bool {};", name)
        });
        if !should_strip {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}
```

Add this block to `glsl_wrapper_template()` after the cubemap binding and before `#define iChannel0`:

```glsl
// Custom shader controls generated from `// control ...` comments.
layout(set = 0, binding = 13) uniform CustomShaderControls {
    vec4 iCustomFloatUniforms[4];
    ivec4 iCustomBoolUniforms[4];
};
```

Change `transpile_impl()` preprocessing order:

```rust
let glsl_source = preprocess_custom_control_uniforms(glsl_source);
let glsl_source = preprocess_glsl_for_shadertoy(&glsl_source);
```

- [ ] **Step 5: Add binding 13 and renderer buffer plumbing**

In `par-term-render/src/custom_shader_renderer/pipeline.rs`:

1. Add binding 13 to `create_bind_group_layout()`:

```rust
BindGroupLayoutEntry {
    binding: 13,
    visibility: ShaderStages::FRAGMENT,
    ty: BindingType::Buffer {
        ty: BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    },
    count: None,
},
```

2. Add `custom_uniform_buffer: &Buffer` argument to `create_bind_group()` and include:

```rust
BindGroupEntry {
    binding: 13,
    resource: custom_uniform_buffer.as_entire_binding(),
},
```

In `par-term-render/src/custom_shader_renderer/mod.rs`:

1. Add fields:

```rust
pub(crate) custom_uniform_buffer: Buffer,
pub(crate) custom_controls: Vec<par_term_config::ShaderControl>,
pub(crate) custom_uniform_values: std::collections::BTreeMap<String, par_term_config::ShaderUniformValue>,
```

2. Add config field:

```rust
pub custom_uniforms: &'a std::collections::BTreeMap<String, par_term_config::ShaderUniformValue>,
```

3. In `new()`, parse controls and create buffer:

```rust
let control_parse = par_term_config::parse_shader_controls(&glsl_source);
for warning in &control_parse.warnings {
    log::warn!("Shader control warning line {}: {}", warning.line, warning.message);
}
let custom_controls = control_parse.controls;
let custom_uniform_values = custom_uniforms.clone();
let custom_uniform_data = crate::custom_shader_renderer::types::CustomShaderControlUniforms::from_controls(
    &custom_controls,
    &custom_uniform_values,
);
let custom_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label: Some("Custom Shader Control Uniform Buffer"),
    contents: bytemuck::cast_slice(&[custom_uniform_data]),
    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
});
```

Ensure `wgpu::util::DeviceExt` is imported if not already.

4. Pass `&custom_uniform_buffer` to `create_bind_group()`.

5. In `render_with_clear_color()`, upload before drawing:

```rust
let custom_uniforms = crate::custom_shader_renderer::types::CustomShaderControlUniforms::from_controls(
    &self.custom_controls,
    &self.custom_uniform_values,
);
queue.write_buffer(&self.custom_uniform_buffer, 0, bytemuck::cast_slice(&[custom_uniforms]));
```

6. Add setter:

```rust
pub fn set_custom_uniform_values(
    &mut self,
    values: std::collections::BTreeMap<String, par_term_config::ShaderUniformValue>,
) {
    self.custom_uniform_values = values;
}
```

- [ ] **Step 6: Run renderer tests and fix compile errors**

```bash
cargo test -p par-term-render custom_uniform -- --nocapture
cargo test -p par-term-render controlled_uniform -- --nocapture
```

Expected: all new renderer tests pass.

- [ ] **Step 7: Commit Task 3**

```bash
git add par-term-render/src/custom_shader_renderer/types.rs par-term-render/src/custom_shader_renderer/transpiler.rs par-term-render/src/custom_shader_renderer/pipeline.rs par-term-render/src/custom_shader_renderer/mod.rs
git commit -m "feat(render): upload shader control uniforms"
```

---

### Task 4: Thread resolved custom uniforms through app renderer initialization and updates

**Files:**
- Modify: `par-term-render/src/renderer/shaders/mod.rs`
- Modify: `par-term-render/src/renderer/shaders/background.rs`
- Modify: `par-term-render/src/renderer/shaders/cursor.rs`
- Modify: `src/app/window_state/renderer_init.rs`
- Modify: `src/app/window_manager/config_renderer_apply.rs`
- Test: compile/typecheck via cargo checks

- [ ] **Step 1: Add failing compile target by extending parameter structs**

Edit `par-term-render/src/renderer/shaders/mod.rs` and add custom uniform maps to background-only parameter structs:

```rust
pub custom_uniforms: &'a std::collections::BTreeMap<String, par_term_config::ShaderUniformValue>,
```

Add it to both `CustomShaderInitParams<'a>` and `CustomShaderEnableParams<'a>`.

Run:

```bash
cargo check -p par-term-render
```

Expected: compile fails at construction sites that do not provide `custom_uniforms` yet.

- [ ] **Step 2: Pass values to renderer creation/update**

In `par-term-render/src/renderer/shaders/background.rs`:

1. Destructure `custom_uniforms` from init params and pass it into `CustomShaderRendererConfig`.
2. Destructure `custom_uniforms` from enable params.
3. In the existing same-path update branch, call:

```rust
renderer.set_custom_uniform_values(custom_uniforms.clone());
```

4. Pass `custom_uniforms` into the new renderer config in the reload/create branch.

In `par-term-render/src/renderer/shaders/cursor.rs`, pass an empty map to cursor shader renderer config:

```rust
let empty_custom_uniforms = std::collections::BTreeMap::new();
```

Use `custom_uniforms: &empty_custom_uniforms` in each cursor renderer config.

- [ ] **Step 3: Thread resolved maps from app startup and settings apply**

In `src/app/window_state/renderer_init.rs`, `RendererInitParams` already resolves `resolved`. Add a field to the params struct if needed:

```rust
custom_shader_custom_uniforms: resolved.custom_uniforms.clone(),
```

Then pass `&params.custom_shader_custom_uniforms` to `CustomShaderInitParams` where renderer initialization is called.

In `src/app/window_manager/config_renderer_apply.rs`, pass `&resolved.custom_uniforms` into `CustomShaderEnableParams`.

- [ ] **Step 4: Run compile and focused tests**

```bash
cargo check -p par-term-render
cargo check --workspace
cargo test -p par-term-render custom_uniform -- --nocapture
cargo test -p par-term-config resolve_custom_uniforms -- --nocapture
```

Expected: checks/tests pass.

- [ ] **Step 5: Commit Task 4**

```bash
git add par-term-render/src/renderer/shaders/mod.rs par-term-render/src/renderer/shaders/background.rs par-term-render/src/renderer/shaders/cursor.rs src/app/window_state/renderer_init.rs src/app/window_manager/config_renderer_apply.rs
git commit -m "feat(app): thread shader control values to renderer"
```

---

### Task 5: Render Shader Controls in settings UI and save defaults

**Files:**
- Modify: `par-term-settings-ui/src/background_tab/shader_settings.rs`
- Modify: `par-term-settings-ui/src/background_tab/shader_metadata.rs`
- Test: `par-term-settings-ui/src/background_tab/shader_settings.rs` helper tests if practical

- [ ] **Step 1: Write failing helper tests for value mutation**

At the bottom of `par-term-settings-ui/src/background_tab/shader_settings.rs`, add pure helper tests. First add helper function signatures above tests and leave stubs until Step 3:

```rust
fn set_shader_uniform_override(
    settings: &mut SettingsUI,
    shader_name: &str,
    uniform_name: &str,
    value: par_term_config::ShaderUniformValue,
) {
    let _ = (settings, shader_name, uniform_name, value);
}

fn clear_shader_uniform_override(settings: &mut SettingsUI, shader_name: &str, uniform_name: &str) {
    let _ = (settings, shader_name, uniform_name);
}
```

Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_shader_uniform_override_creates_per_shader_entry() {
        let mut settings = SettingsUI::new(par_term_config::Config::default());

        set_shader_uniform_override(
            &mut settings,
            "controlled.glsl",
            "iGlow",
            par_term_config::ShaderUniformValue::Float(0.75),
        );

        assert_eq!(
            settings
                .config
                .shader_configs
                .get("controlled.glsl")
                .and_then(|config| config.uniforms.get("iGlow")),
            Some(&par_term_config::ShaderUniformValue::Float(0.75))
        );
        assert!(settings.has_changes);
    }

    #[test]
    fn clear_shader_uniform_override_removes_one_value() {
        let mut settings = SettingsUI::new(par_term_config::Config::default());
        set_shader_uniform_override(
            &mut settings,
            "controlled.glsl",
            "iGlow",
            par_term_config::ShaderUniformValue::Float(0.75),
        );

        settings.has_changes = false;
        clear_shader_uniform_override(&mut settings, "controlled.glsl", "iGlow");

        assert!(
            settings
                .config
                .shader_configs
                .get("controlled.glsl")
                .is_none_or(|config| !config.uniforms.contains_key("iGlow"))
        );
        assert!(settings.has_changes);
    }
}
```

- [ ] **Step 2: Run tests and verify RED**

```bash
cargo test -p par-term-settings-ui shader_uniform_override -- --nocapture
```

Expected: tests fail because helpers are stubs.

- [ ] **Step 3: Implement settings helpers and UI**

Implement helpers:

```rust
fn set_shader_uniform_override(
    settings: &mut SettingsUI,
    shader_name: &str,
    uniform_name: &str,
    value: par_term_config::ShaderUniformValue,
) {
    let override_entry = settings.config.get_or_create_shader_override(shader_name);
    override_entry.uniforms.insert(uniform_name.to_string(), value);
    settings.has_changes = true;
}

fn clear_shader_uniform_override(settings: &mut SettingsUI, shader_name: &str, uniform_name: &str) {
    if let Some(override_entry) = settings.config.shader_configs.get_mut(shader_name) {
        override_entry.uniforms.remove(uniform_name);
    }
    settings.has_changes = true;
}
```

Add helpers for effective values:

```rust
fn effective_uniform_value(
    control: &par_term_config::ShaderControl,
    current_override: Option<&par_term_config::ShaderConfig>,
    metadata: Option<&par_term_config::ShaderMetadata>,
) -> par_term_config::ShaderUniformValue {
    current_override
        .and_then(|config| config.uniforms.get(&control.name).cloned())
        .or_else(|| metadata.and_then(|meta| meta.defaults.uniforms.get(&control.name).cloned()))
        .unwrap_or_else(|| par_term_config::fallback_value_for_control(control))
}
```

Add a `show_shader_uniform_controls()` function in `shader_settings.rs` and call it inside `show_per_shader_settings()` before `Reset All Overrides`:

```rust
fn show_shader_uniform_controls(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    shader_name: &str,
    metadata: &Option<par_term_config::ShaderMetadata>,
    changes_this_frame: &mut bool,
) {
    let shader_path = par_term_config::Config::shader_path(shader_name);
    let source = match std::fs::read_to_string(&shader_path) {
        Ok(source) => source,
        Err(_) => return,
    };
    let parsed = par_term_config::parse_shader_controls(&source);

    if parsed.controls.is_empty() && parsed.warnings.is_empty() {
        return;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label("Shader Controls");

    for warning in parsed.warnings {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 80),
            format!("Line {}: {}", warning.line, warning.message),
        );
    }

    let current_override = settings.config.shader_configs.get(shader_name).cloned();

    for control in parsed.controls {
        let has_override = current_override
            .as_ref()
            .is_some_and(|config| config.uniforms.contains_key(&control.name));
        let value = effective_uniform_value(&control, current_override.as_ref(), metadata.as_ref());

        ui.horizontal(|ui| match control.kind {
            par_term_config::ShaderControlKind::Slider { min, max, step } => {
                let mut slider_value = match value {
                    par_term_config::ShaderUniformValue::Float(value) => value.clamp(min, max),
                    _ => min,
                };
                ui.label(format!("{}:", control.name));
                let response = ui.add(
                    egui::Slider::new(&mut slider_value, min..=max)
                        .step_by(step as f64),
                );
                if response.changed() {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Float(slider_value),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
            par_term_config::ShaderControlKind::Checkbox => {
                let mut checked = matches!(value, par_term_config::ShaderUniformValue::Bool(true));
                if ui.checkbox(&mut checked, &control.name).changed() {
                    set_shader_uniform_override(
                        settings,
                        shader_name,
                        &control.name,
                        par_term_config::ShaderUniformValue::Bool(checked),
                    );
                    *changes_this_frame = true;
                }
                if show_reset_button(ui, has_override) {
                    clear_shader_uniform_override(settings, shader_name, &control.name);
                    *changes_this_frame = true;
                }
            }
        });
    }
}
```

Call site inside `show_per_shader_settings()`:

```rust
show_shader_uniform_controls(ui, settings, shader_name, metadata, changes_this_frame);
```

`Reset All Overrides` already calls `remove_shader_override(shader_name)`, which clears `uniforms` too.

- [ ] **Step 4: Include custom uniforms in Save Defaults to Shader**

In `par-term-settings-ui/src/background_tab/shader_metadata.rs`, update `build_metadata_from_settings()` after standard defaults are set:

```rust
let shader_path = par_term_config::Config::shader_path(shader_name);
if let Ok(source) = std::fs::read_to_string(&shader_path) {
    let parsed = par_term_config::parse_shader_controls(&source);
    for control in parsed.controls {
        let value = current_override
            .and_then(|o| o.uniforms.get(&control.name).cloned())
            .or_else(|| meta_defaults.and_then(|m| m.uniforms.get(&control.name).cloned()))
            .unwrap_or_else(|| par_term_config::fallback_value_for_control(&control));
        new_defaults.uniforms.insert(control.name, value);
    }
}
```

- [ ] **Step 5: Run settings UI tests and compile**

```bash
cargo test -p par-term-settings-ui shader_uniform_override -- --nocapture
cargo check -p par-term-settings-ui
```

Expected: tests and check pass.

- [ ] **Step 6: Commit Task 5**

```bash
git add par-term-settings-ui/src/background_tab/shader_settings.rs par-term-settings-ui/src/background_tab/shader_metadata.rs
git commit -m "feat(settings): show shader uniform controls"
```

---

### Task 6: Documentation, formatting, and full verification

**Files:**
- Modify: `docs/CUSTOM_SHADERS.md`
- Verify: workspace

- [ ] **Step 1: Update documentation**

Add a section under `docs/CUSTOM_SHADERS.md` near custom shader creation/settings:

```markdown
### Shader Uniform Controls

Background shaders can declare settings-page controls for custom uniforms by placing a `// control ...` comment immediately before a supported uniform declaration.

```glsl
/*! par-term shader metadata
name: "Controlled Glow"
defaults:
  uniforms:
    iGlow: 0.5
    iEnabled: true
*/

// control slider min=0 max=1 step=0.01
uniform float iGlow;

// control checkbox
uniform bool iEnabled;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec3 color = vec3(iGlow * uv.x);
    if (iEnabled) {
        color += vec3(0.1, 0.2, 0.4);
    }
    fragColor = vec4(color, 1.0);
}
```

Supported v1 controls:

| Comment | Uniform | UI |
|---------|---------|----|
| `// control slider min=0 max=1 step=0.01` | `uniform float name;` | Slider |
| `// control checkbox` | `uniform bool name;` | Checkbox |

Defaults live in the shader metadata block under `defaults.uniforms`. User edits are saved as per-shader overrides in `config.yaml` and take precedence over metadata defaults. If no override or metadata default exists, sliders use their `min` value and checkboxes use `false`.

Limits: up to 16 float slider controls and 16 bool checkbox controls per shader. Extra valid controls are ignored with warnings. Malformed control comments produce warnings in settings but do not prevent shader editing or loading unless GLSL compilation fails.
```

- [ ] **Step 2: Format**

```bash
make fmt
```

Expected: rustfmt completes successfully.

- [ ] **Step 3: Run focused tests**

```bash
cargo test -p par-term-config shader_controls -- --nocapture
cargo test -p par-term-config resolve_custom_uniforms -- --nocapture
cargo test -p par-term-render custom_uniform -- --nocapture
cargo test -p par-term-render controlled_uniform -- --nocapture
cargo test -p par-term-settings-ui shader_uniform_override -- --nocapture
```

Expected: all focused tests pass.

- [ ] **Step 4: Run full verification**

```bash
make checkall
```

Expected: format check, lint, typecheck, and tests pass.

- [ ] **Step 5: Commit Task 6**

```bash
git add docs/CUSTOM_SHADERS.md
git commit -m "docs: document shader uniform controls"
```

If `make fmt` changed Rust files that were not committed in prior tasks, include them in the task commit where they belong before committing docs.

---

## Self-Review Notes

- Spec coverage: parser, schema, config resolution, runtime block, UI, save defaults, warnings, docs, and verification are covered.
- Placeholder scan: no banned placeholder phrases remain in implementation steps.
- Type consistency: plan consistently uses `ShaderUniformValue`, `ShaderControl`, `ShaderControlKind`, and `ResolvedShaderConfig::custom_uniforms`.
