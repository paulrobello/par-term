# Shader Color Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `// control color` support for background shader uniforms.

**Architecture:** Extend the existing shader controls pipeline: parser/schema in `par-term-config`, slot mapping and transpiler defines in `par-term-render`, controls UI/default saving in `par-term-settings-ui`, then docs and assistant shader guidance. Colors are stored as normalized RGBA floats, serialize to preferred hex, and accept hex or numeric arrays in metadata/config.

**Tech Stack:** Rust 2024, serde/serde_yaml_ng, egui color picker, wgpu uniform buffers, naga GLSL transpilation.

---

## Task 1: Config schema and parser

**Files:**
- `par-term-config/src/types/shader.rs`
- `par-term-config/src/shader_controls.rs`
- `par-term-config/src/shader_metadata/mod.rs`

Steps:
- [ ] Add tests first for `ShaderUniformValue` parsing hex `#ff8800`, hex alpha `#ff8800cc`, arrays `[1.0, 0.5, 0.0]` and `[1.0, 0.5, 0.0, 0.8]`.
- [ ] Add parser tests for:
  - `// control color label="Tint"` + `uniform vec3 iTint;`
  - `// control color alpha=true label="Overlay"` + `uniform vec4 iOverlay;`
  - `alpha=true` on vec3 warns/skips
  - invalid color default type falls back later rather than parser failure
  - over 16 colors warns/ignores extras
- [ ] Implement `ShaderColorValue`/`ShaderUniformValue::Color`, accepting hex and arrays, serializing to hex.
- [ ] Extend `ShaderControlKind::Color { alpha: bool, label: Option<String> }`.
- [ ] Extend `fallback_value_for_control()` to return opaque white or black? Use `[1.0, 1.0, 1.0, 1.0]` as neutral color fallback unless metadata/override exists.
- [ ] Run `cargo test -p par-term-config shader_controls -- --nocapture` and relevant metadata tests.
- [ ] Commit `feat(config): parse shader color controls`.

## Task 2: Renderer/transpiler color slots

**Files:**
- `par-term-render/src/custom_shader_renderer/types.rs`
- `par-term-render/src/custom_shader_renderer/transpiler.rs`

Steps:
- [ ] Add tests first for color slot upload and transpiler macro mapping.
- [ ] Add 16 color `vec4` slots to `CustomShaderControlUniforms` and size assertion.
- [ ] Map color controls to `iCustomColorUniforms[index]`; for vec3 use `.rgb` in macro, for vec4 use full vec4.
- [ ] For invalid/over-limit attached color controls, strip standalone uniform and define safe fallback (`vec3(1.0)` or `vec4(1.0)`) so compilation remains non-fatal.
- [ ] Run `cargo test -p par-term-render custom_uniform -- --nocapture` and `cargo test -p par-term-render controlled_uniform -- --nocapture`.
- [ ] Commit `feat(render): upload shader color controls`.

## Task 3: Settings UI and metadata save

**Files:**
- `par-term-settings-ui/src/background_tab/shader_settings.rs`
- `par-term-settings-ui/src/background_tab/shader_metadata.rs`

Steps:
- [ ] Add tests first for normalized effective color values: hex/array default, wrong type fallback, alpha handling.
- [ ] Add egui color picker UI for `ShaderControlKind::Color`; RGB for alpha false, RGBA for alpha true.
- [ ] Store changes as `ShaderUniformValue::Color` overrides; reset clears only that uniform.
- [ ] Save Defaults writes normalized effective color values to `defaults.uniforms`.
- [ ] Run `cargo test -p par-term-settings-ui shader_uniform_override -- --nocapture` and `cargo check -p par-term-settings-ui`.
- [ ] Commit `feat(settings): add shader color controls`.

## Task 4: Docs and assistant guidance

**Files:**
- `docs/CUSTOM_SHADERS.md`
- `docs/ASSISTANT_PANEL.md`
- `src/ai_inspector/shader_context/context_builder.rs`
- `src/ai_inspector/shader_context/tests.rs`
- `tests/shader_context_tests.rs`
- `CHANGELOG.md`

Steps:
- [ ] Document `// control color`, hex/array defaults, vec3/vec4 alpha rules, and 16-color limit.
- [ ] Update assistant shader context and tests with color picker guidance.
- [ ] Update changelog.
- [ ] Run `cargo test shader_context -- --nocapture`.
- [ ] Run `make checkall`.
- [ ] Commit `docs: document shader color controls`.
