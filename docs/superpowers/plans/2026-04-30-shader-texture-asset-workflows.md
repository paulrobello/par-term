# Shader Texture Asset Workflows Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Texture and asset workflows ideas with repo-local texture packs, shader bundle manifests, built-in noise channels, background blend modes, cubemap showcase shaders, and documentation.

**Architecture:** Keep existing shader path/config systems intact. Add small additive config/parser modules, keep generated textures behind `ChannelTexture`, expose blend mode through the existing custom-shader uniform block, and use bundled files plus manifest tracking for texture packs and showcase shaders.

**Tech Stack:** Rust 2024, serde/serde_yaml_ng, wgpu, image crate, existing `make` verification commands, GLSL shader assets.

---

## Worktree and Baseline

Worktree: `/Users/probello/Repos/par-term/.worktrees/shader-texture-assets`

Baseline command already run:

```bash
make test
```

Baseline result: passed.

Spec: `docs/superpowers/specs/2026-04-30-shader-texture-asset-workflows-design.md`

---

## File Structure

- `par-term-config/src/types/shader.rs` — add `ShaderBackgroundBlendMode`, per-shader override field, resolved field.
- `par-term-config/src/config/config_struct/global_shader_config.rs` — add global blend-mode config field and default.
- `par-term-config/src/defaults/shader.rs` and `defaults/mod.rs` — add blend-mode default helper if needed by existing default pattern.
- `par-term-config/src/shader_config.rs` — resolve blend mode across user override, metadata defaults, and global config.
- `par-term-config/src/shader_bundle.rs` — new bundle manifest parser/validator.
- `par-term-config/src/lib.rs` — export bundle manifest APIs and `ShaderBackgroundBlendMode`.
- `par-term-render/src/custom_shader_renderer/builtin_textures.rs` — new deterministic built-in texture generation helpers.
- `par-term-render/src/custom_shader_renderer/textures.rs` — load `builtin://noise/...` before filesystem textures.
- `par-term-render/src/custom_shader_renderer/types.rs` — add blend-mode uniform storage.
- `par-term-render/src/custom_shader_renderer/uniforms.rs` — populate blend-mode uniform.
- `par-term-render/src/custom_shader_renderer/transpiler.rs` — expose `iBackgroundBlendMode` and constants in GLSL wrapper.
- `par-term-render/src/custom_shader_renderer/mod.rs` — store resolved blend mode and pass it from construction/update paths.
- `src/app/render_pipeline/post_render.rs` and related shader-state call sites — pass resolved blend mode into renderer when creating/updating background shaders.
- `par-term-settings-ui/src/background_tab/global_channels.rs` — add built-in noise choices and blend-mode dropdown.
- `par-term-settings-ui/src/settings_ui/mod.rs`, `state.rs` — add temporary blend-mode state only if needed.
- `shaders/textures/packs/**` — add small deterministic texture-pack PNGs.
- `shaders/cubemap-metallic-ambience.glsl`, `shaders/cubemap-neon-room.glsl`, `shaders/cubemap-atmospheric-sky.glsl` — add showcase shaders.
- `shaders/manifest.json` — add new shaders/textures.
- `docs/CUSTOM_SHADERS.md`, `docs/SHADERS.md`, `docs/CONFIG_REFERENCE.md`, `docs/INTEGRATIONS.md` — document the workflows.
- `ideas.md` — remove completed Texture and asset workflows items.

---

### Task 1: Config Blend Mode and Bundle Manifest

**Files:**
- Modify: `par-term-config/src/types/shader.rs`
- Modify: `par-term-config/src/types/mod.rs`
- Modify: `par-term-config/src/config/config_struct/global_shader_config.rs`
- Modify: `par-term-config/src/defaults/shader.rs`
- Modify: `par-term-config/src/defaults/mod.rs`
- Modify: `par-term-config/src/shader_config.rs`
- Create: `par-term-config/src/shader_bundle.rs`
- Modify: `par-term-config/src/lib.rs`

- [ ] **Step 1: Write failing config tests**

Add tests in `par-term-config/src/shader_config.rs` under the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn resolves_background_channel0_blend_mode_from_global_default() {
    let config = Config::default();
    let resolved = resolve_shader_config(None, None, &config);
    assert_eq!(
        resolved.background_channel0_blend_mode,
        ShaderBackgroundBlendMode::Replace
    );
}

#[test]
fn resolves_background_channel0_blend_mode_override_over_metadata() {
    let config = Config::default();
    let metadata = ShaderMetadata {
        name: Some("Blend Metadata".to_string()),
        author: None,
        description: None,
        version: None,
        defaults: ShaderConfig {
            background_channel0_blend_mode: Some(ShaderBackgroundBlendMode::Multiply),
            ..Default::default()
        },
        safety: Default::default(),
    };
    let override_config = ShaderConfig {
        background_channel0_blend_mode: Some(ShaderBackgroundBlendMode::Screen),
        ..Default::default()
    };

    let resolved = resolve_shader_config(Some(&override_config), Some(&metadata), &config);

    assert_eq!(
        resolved.background_channel0_blend_mode,
        ShaderBackgroundBlendMode::Screen
    );
}
```

Add tests in new `par-term-config/src/shader_bundle.rs`:

```rust
#[test]
fn bundle_manifest_requires_author_and_description() {
    let json = r#"{
        "shader": "shader.glsl",
        "name": "Missing Fields",
        "license": "MIT"
    }"#;

    let err = ShaderBundleManifest::from_json_str(json)
        .expect_err("missing author and description should fail");

    assert!(err.contains("author"));
    assert!(err.contains("description"));
}

#[test]
fn validates_bundle_manifest_paths_relative_to_bundle_dir() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("shader.glsl"), "void mainImage(out vec4 c, in vec2 p){c=vec4(0.0);}").unwrap();
    std::fs::create_dir_all(temp.path().join("textures")).unwrap();
    std::fs::write(temp.path().join("textures/noise.png"), b"fake").unwrap();

    let manifest = ShaderBundleManifest {
        shader: "shader.glsl".to_string(),
        name: "Valid Bundle".to_string(),
        author: "par-term".to_string(),
        description: "A valid test bundle.".to_string(),
        license: "MIT".to_string(),
        textures: vec!["textures/noise.png".to_string()],
        cubemaps: Vec::new(),
        screenshot: None,
    };

    manifest.validate_paths(temp.path()).unwrap();
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p par-term-config shader_config::tests::resolves_background_channel0_blend_mode --lib
cargo test -p par-term-config shader_bundle --lib
```

Expected: fail because `ShaderBackgroundBlendMode` / bundle module do not exist.

- [ ] **Step 3: Implement blend-mode enum and config resolution**

In `par-term-config/src/types/shader.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderBackgroundBlendMode {
    Replace,
    Multiply,
    Screen,
    Overlay,
    LuminanceMask,
}

impl Default for ShaderBackgroundBlendMode {
    fn default() -> Self {
        Self::Replace
    }
}

impl ShaderBackgroundBlendMode {
    pub fn as_uniform_int(self) -> i32 {
        match self {
            Self::Replace => 0,
            Self::Multiply => 1,
            Self::Screen => 2,
            Self::Overlay => 3,
            Self::LuminanceMask => 4,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::LuminanceMask => "Luminance mask",
        }
    }

    pub const ALL: [Self; 5] = [
        Self::Replace,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::LuminanceMask,
    ];
}
```

Add to `ShaderConfig`:

```rust
/// Blend mode hint for shaders using the app background as iChannel0.
pub background_channel0_blend_mode: Option<ShaderBackgroundBlendMode>,
```

Add to `ResolvedShaderConfig` and its `Default`:

```rust
pub background_channel0_blend_mode: ShaderBackgroundBlendMode,
```

Update `par-term-config/src/types/mod.rs` and `par-term-config/src/lib.rs` re-exports to include `ShaderBackgroundBlendMode`.

In `GlobalShaderConfig`, add:

```rust
#[serde(default = "crate::defaults::background_channel0_blend_mode")]
pub custom_shader_background_channel0_blend_mode: crate::types::ShaderBackgroundBlendMode,
```

Add default helper in `defaults/shader.rs`:

```rust
pub fn background_channel0_blend_mode() -> crate::types::ShaderBackgroundBlendMode {
    crate::types::ShaderBackgroundBlendMode::Replace
}
```

Export it from `defaults/mod.rs` using the existing pattern.

In `resolve_shader_config`, resolve the field with precedence user override > metadata > global:

```rust
let background_channel0_blend_mode = user_override
    .and_then(|u| u.background_channel0_blend_mode)
    .or_else(|| meta_defaults.and_then(|m| m.background_channel0_blend_mode))
    .unwrap_or(config.shader.custom_shader_background_channel0_blend_mode);
```

Set it in the returned `ResolvedShaderConfig`.

- [ ] **Step 4: Implement bundle manifest parser**

Create `par-term-config/src/shader_bundle.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShaderBundleManifest {
    pub shader: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    #[serde(default)]
    pub textures: Vec<String>,
    #[serde(default)]
    pub cubemaps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

impl ShaderBundleManifest {
    pub fn from_json_str(input: &str) -> Result<Self, String> {
        let manifest: Self = serde_json::from_str(input)
            .map_err(|e| format!("parse shader bundle manifest: {e}"))?;
        manifest.validate_required_fields()?;
        Ok(manifest)
    }

    pub fn validate_required_fields(&self) -> Result<(), String> {
        let mut missing = Vec::new();
        if self.shader.trim().is_empty() { missing.push("shader"); }
        if self.name.trim().is_empty() { missing.push("name"); }
        if self.author.trim().is_empty() { missing.push("author"); }
        if self.description.trim().is_empty() { missing.push("description"); }
        if self.license.trim().is_empty() { missing.push("license"); }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("missing required shader bundle manifest field(s): {}", missing.join(", ")))
        }
    }

    pub fn validate_paths(&self, bundle_dir: &Path) -> Result<(), String> {
        self.validate_required_fields()?;
        validate_relative_path("shader", &self.shader)?;
        if !self.shader.ends_with(".glsl") {
            return Err("shader bundle manifest field `shader` must point to a .glsl file".to_string());
        }
        ensure_exists(bundle_dir, "shader", &self.shader)?;

        for texture in &self.textures {
            validate_relative_path("textures", texture)?;
            ensure_exists(bundle_dir, "textures", texture)?;
        }
        for cubemap in &self.cubemaps {
            validate_relative_path("cubemaps", cubemap)?;
            ensure_cubemap_faces_exist(bundle_dir, cubemap)?;
        }
        if let Some(screenshot) = &self.screenshot {
            validate_relative_path("screenshot", screenshot)?;
            ensure_exists(bundle_dir, "screenshot", screenshot)?;
        }
        Ok(())
    }
}

fn validate_relative_path(field: &str, value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() || value.contains("..") {
        return Err(format!("shader bundle manifest field `{field}` must be a non-empty relative path without '..'"));
    }
    Ok(())
}

fn ensure_exists(bundle_dir: &Path, field: &str, value: &str) -> Result<(), String> {
    let path = bundle_dir.join(value);
    if path.exists() {
        Ok(())
    } else {
        Err(format!("shader bundle manifest field `{field}` path does not exist: {value}"))
    }
}

fn ensure_cubemap_faces_exist(bundle_dir: &Path, prefix: &str) -> Result<(), String> {
    const SUFFIXES: [&str; 6] = ["px", "nx", "py", "ny", "pz", "nz"];
    const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "hdr"];
    for suffix in SUFFIXES {
        let found = EXTENSIONS.iter().any(|ext| {
            bundle_dir.join(format!("{prefix}-{suffix}.{ext}")).exists()
        });
        if !found {
            return Err(format!("missing cubemap face for prefix `{prefix}` and suffix `{suffix}`"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Keep the two tests from Step 1 here.
}
```

Add `pub mod shader_bundle;` and re-export `ShaderBundleManifest` in `lib.rs`.

- [ ] **Step 5: Run GREEN tests and commit**

Run:

```bash
cargo test -p par-term-config shader_config::tests::resolves_background_channel0_blend_mode --lib
cargo test -p par-term-config shader_bundle --lib
cargo test -p par-term-config shader_config::tests::test_resolve_with_metadata_defaults --lib
```

Expected: pass.

Commit:

```bash
git add par-term-config/src/types/shader.rs par-term-config/src/types/mod.rs par-term-config/src/config/config_struct/global_shader_config.rs par-term-config/src/defaults/shader.rs par-term-config/src/defaults/mod.rs par-term-config/src/shader_config.rs par-term-config/src/shader_bundle.rs par-term-config/src/lib.rs
git commit -m "feat: add shader bundle manifests and blend config"
```

---

### Task 2: Renderer Built-In Noise and Blend Uniform

**Files:**
- Create: `par-term-render/src/custom_shader_renderer/builtin_textures.rs`
- Modify: `par-term-render/src/custom_shader_renderer/mod.rs`
- Modify: `par-term-render/src/custom_shader_renderer/textures.rs`
- Modify: `par-term-render/src/custom_shader_renderer/types.rs`
- Modify: `par-term-render/src/custom_shader_renderer/uniforms.rs`
- Modify: `par-term-render/src/custom_shader_renderer/transpiler.rs`
- Modify likely call sites that construct/update `CustomShaderRenderer`.

- [ ] **Step 1: Write failing renderer tests**

In new `builtin_textures.rs`, include tests:

```rust
#[test]
fn parses_supported_builtin_noise_ids() {
    let spec = BuiltinTextureSpec::parse("builtin://noise/fbm-512").unwrap();
    assert_eq!(spec.kind, BuiltinTextureKind::Fbm);
    assert_eq!(spec.size, 512);
}

#[test]
fn rejects_unknown_builtin_noise_id() {
    let err = BuiltinTextureSpec::parse("builtin://noise/marble-256").unwrap_err();
    assert!(err.contains("unknown built-in texture"));
}

#[test]
fn generated_builtin_noise_is_deterministic() {
    let spec = BuiltinTextureSpec::parse("builtin://noise/value-128").unwrap();
    let a = spec.generate_rgba8();
    let b = spec.generate_rgba8();
    assert_eq!(a.width, 128);
    assert_eq!(a.height, 128);
    assert_eq!(a.pixels, b.pixels);
}
```

In `transpiler.rs` tests, add:

```rust
#[test]
fn wrapper_exposes_background_blend_mode_uniform_and_constants() {
    let source = "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(float(iBackgroundBlendMode)); }";
    let wgsl = transpile_glsl_to_wgsl(source).expect("transpile should succeed");
    assert!(wgsl.contains("iBackgroundBlendMode") || wgsl.contains("background_blend_mode"));
}
```

Adjust the exact transpiler helper name to match existing tests in the file.

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p par-term-render builtin_textures --lib
cargo test -p par-term-render wrapper_exposes_background_blend_mode_uniform_and_constants --lib
```

Expected: fail because module/uniform do not exist.

- [ ] **Step 3: Implement built-in texture generation**

Create `par-term-render/src/custom_shader_renderer/builtin_textures.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltinTextureKind {
    Value,
    Fbm,
    Cellular,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinTextureSpec {
    pub kind: BuiltinTextureKind,
    pub size: u32,
}

pub(crate) struct GeneratedTexture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl BuiltinTextureSpec {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let id = value.strip_prefix("builtin://noise/")
            .ok_or_else(|| format!("not a built-in noise texture: {value}"))?;
        let (kind, size) = match id {
            "value-128" => (BuiltinTextureKind::Value, 128),
            "value-256" => (BuiltinTextureKind::Value, 256),
            "fbm-256" => (BuiltinTextureKind::Fbm, 256),
            "fbm-512" => (BuiltinTextureKind::Fbm, 512),
            "cellular-256" => (BuiltinTextureKind::Cellular, 256),
            _ => return Err(format!("unknown built-in texture: {value}")),
        };
        Ok(Self { kind, size })
    }

    pub(crate) fn generate_rgba8(self) -> GeneratedTexture {
        let mut pixels = Vec::with_capacity((self.size * self.size * 4) as usize);
        for y in 0..self.size {
            for x in 0..self.size {
                let v = match self.kind {
                    BuiltinTextureKind::Value => value_noise(x, y, self.size),
                    BuiltinTextureKind::Fbm => fbm_noise(x, y, self.size),
                    BuiltinTextureKind::Cellular => cellular_noise(x, y, self.size),
                };
                pixels.extend_from_slice(&[v, v, v, 255]);
            }
        }
        GeneratedTexture { width: self.size, height: self.size, pixels }
    }
}

fn hash2(x: u32, y: u32) -> u32 {
    let mut n = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7FEB_352D);
    n ^= n >> 15;
    n = n.wrapping_mul(0x846C_A68B);
    n ^ (n >> 16)
}

fn value_noise(x: u32, y: u32, _size: u32) -> u8 {
    (hash2(x, y) & 0xff) as u8
}

fn fbm_noise(x: u32, y: u32, size: u32) -> u8 {
    let mut total = 0.0f32;
    let mut amp = 0.5f32;
    let mut scale = 1u32;
    for _ in 0..5 {
        let sx = (x / scale.max(1)).min(size - 1);
        let sy = (y / scale.max(1)).min(size - 1);
        total += value_noise(sx, sy, size) as f32 / 255.0 * amp;
        amp *= 0.5;
        scale *= 2;
    }
    (total.clamp(0.0, 1.0) * 255.0) as u8
}

fn cellular_noise(x: u32, y: u32, size: u32) -> u8 {
    let cell = (size / 16).max(8);
    let cx = x / cell;
    let cy = y / cell;
    let mut best = f32::MAX;
    for oy in 0..=1 {
        for ox in 0..=1 {
            let hx = hash2(cx + ox, cy + oy);
            let px = ((cx + ox) * cell + (hx & 0xff) % cell) as f32;
            let py = ((cy + oy) * cell + ((hx >> 8) & 0xff) % cell) as f32;
            let dx = x as f32 - px;
            let dy = y as f32 - py;
            best = best.min((dx * dx + dy * dy).sqrt());
        }
    }
    ((best / cell as f32).clamp(0.0, 1.0) * 255.0) as u8
}
```

Wire `pub(crate) mod builtin_textures;` in `custom_shader_renderer/mod.rs`.

In `ChannelTexture`, add `from_builtin` that creates a `Texture` from generated bytes. In `from_file`, detect string paths that start with `builtin://noise/` before `image::open` by checking `path.to_string_lossy()`.

- [ ] **Step 4: Implement blend uniform**

In `types.rs`, append a vec4 to avoid std140 scalar padding churn:

```rust
/// Background channel options [blendMode, reserved, reserved, reserved] - offset 368
pub background_channel: [f32; 4],
```

Update total size comment to 384 bytes.

In `uniforms.rs`, set:

```rust
background_channel: [
    self.background_channel0_blend_mode.as_uniform_int() as f32,
    0.0,
    0.0,
    0.0,
],
```

In renderer state (`mod.rs`), add:

```rust
pub(crate) background_channel0_blend_mode: par_term_config::ShaderBackgroundBlendMode,
```

Set default to `Replace` in constructor params and update any constructor params struct to include `background_channel0_blend_mode`.

In `transpiler.rs` GLSL uniform wrapper, add after `iReadability`:

```glsl
vec4 iBackgroundChannel;  // offset 368, size 16 - x=background-as-channel0 blend mode
```

Then define:

```glsl
int iBackgroundBlendMode = int(iBackgroundChannel.x + 0.5);
const int BACKGROUND_BLEND_REPLACE = 0;
const int BACKGROUND_BLEND_MULTIPLY = 1;
const int BACKGROUND_BLEND_SCREEN = 2;
const int BACKGROUND_BLEND_OVERLAY = 3;
const int BACKGROUND_BLEND_LUMINANCE_MASK = 4;
```

- [ ] **Step 5: Update renderer call sites**

Find creation/update sites:

```bash
rg "CustomShaderRenderer|ResolvedShaderConfig|use_background_as_channel0|auto_dim_under_text" src par-term-render par-term-config
```

Pass `resolved.background_channel0_blend_mode` wherever renderer parameters are assembled. Do not change cursor shader behavior unless the shared constructor requires it; cursor shaders can use `Replace`.

- [ ] **Step 6: Run GREEN tests and commit**

Run:

```bash
cargo test -p par-term-render builtin_textures --lib
cargo test -p par-term-render wrapper_exposes_background_blend_mode_uniform_and_constants --lib
cargo check --workspace
```

Commit:

```bash
git add par-term-render/src/custom_shader_renderer par-term-config src
git commit -m "feat: add built-in shader textures and blend uniform"
```

---

### Task 3: Settings UI Controls

**Files:**
- Modify: `par-term-settings-ui/src/background_tab/global_channels.rs`
- Modify: `par-term-settings-ui/src/settings_ui/mod.rs` if temporary state is needed.
- Modify: `par-term-settings-ui/src/settings_ui/state.rs` if temporary state is needed.

- [ ] **Step 1: Write or extend focused UI tests**

If there is no direct egui test harness for this section, add pure helper tests in `global_channels.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_noise_choices_are_stable() {
        assert_eq!(
            builtin_noise_choices(),
            [
                "builtin://noise/value-128",
                "builtin://noise/value-256",
                "builtin://noise/fbm-256",
                "builtin://noise/fbm-512",
                "builtin://noise/cellular-256",
            ]
        );
    }

    #[test]
    fn blend_mode_labels_are_user_readable() {
        assert_eq!(
            par_term_config::ShaderBackgroundBlendMode::Overlay.display_name(),
            "Overlay"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p par-term-settings-ui builtin_noise_choices_are_stable --lib
```

Expected: fail because helper does not exist.

- [ ] **Step 3: Add built-in noise choices to channel controls**

Add helper in `global_channels.rs`:

```rust
pub(super) fn builtin_noise_choices() -> [&'static str; 5] {
    [
        "builtin://noise/value-128",
        "builtin://noise/value-256",
        "builtin://noise/fbm-256",
        "builtin://noise/fbm-512",
        "builtin://noise/cellular-256",
    ]
}
```

For each channel row, add a `Builtin…` combo or menu next to Browse that sets the corresponding `temp_shader_channelN` and `config.shader.custom_shader_channelN`. Keep the existing text field and Browse button unchanged.

Use a small helper to avoid duplicating assignment logic if the file already has a suitable pattern:

```rust
fn set_channel_path(settings: &mut SettingsUI, channel: usize, value: String) {
    match channel {
        0 => {
            settings.temp_shader_channel0 = value.clone();
            settings.config.shader.custom_shader_channel0 = Some(value);
        }
        1 => {
            settings.temp_shader_channel1 = value.clone();
            settings.config.shader.custom_shader_channel1 = Some(value);
        }
        2 => {
            settings.temp_shader_channel2 = value.clone();
            settings.config.shader.custom_shader_channel2 = Some(value);
        }
        3 => {
            settings.temp_shader_channel3 = value.clone();
            settings.config.shader.custom_shader_channel3 = Some(value);
        }
        _ => unreachable!("only iChannel0-3 are supported"),
    }
}
```

- [ ] **Step 4: Add blend-mode dropdown**

Near “Use background as iChannel0”, add:

```rust
ui.horizontal(|ui| {
    ui.label("Background blend mode:");
    egui::ComboBox::from_id_salt("background_channel0_blend_mode")
        .selected_text(settings.config.shader.custom_shader_background_channel0_blend_mode.display_name())
        .show_ui(ui, |ui| {
            for mode in par_term_config::ShaderBackgroundBlendMode::ALL {
                if ui
                    .selectable_value(
                        &mut settings.config.shader.custom_shader_background_channel0_blend_mode,
                        mode,
                        mode.display_name(),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            }
        });
});
```

It may be always visible; keep hover text clear that shaders read the mode via `iBackgroundBlendMode`.

- [ ] **Step 5: Run GREEN tests and commit**

Run:

```bash
cargo test -p par-term-settings-ui builtin_noise_choices_are_stable --lib
cargo check -p par-term-settings-ui
```

Commit:

```bash
git add par-term-settings-ui/src/background_tab/global_channels.rs par-term-settings-ui/src/settings_ui/mod.rs par-term-settings-ui/src/settings_ui/state.rs
git commit -m "feat: add shader texture workflow controls"
```

---

### Task 4: Bundled Texture Packs and Cubemap Showcase Shaders

**Files:**
- Create: `shaders/textures/packs/noise/*.png`
- Create: `shaders/textures/packs/gradients/*.png`
- Create: `shaders/textures/packs/paper/*.png`
- Create: `shaders/textures/packs/metal/*.png`
- Create: `shaders/textures/packs/starfields/*.png`
- Create: `shaders/cubemap-metallic-ambience.glsl`
- Create: `shaders/cubemap-neon-room.glsl`
- Create: `shaders/cubemap-atmospheric-sky.glsl`
- Modify: `shaders/manifest.json`

- [ ] **Step 1: Generate small deterministic pack textures**

Use Python with Pillow if available; otherwise use the repository's existing image dependency only for runtime and generate simple PNGs with Python stdlib + zlib. Prefer 128x128 PNGs.

Create this temporary script and run it from repo root:

```bash
python3 - <<'PY'
from pathlib import Path
import math, random, struct, zlib

ROOT = Path('shaders/textures/packs')

def write_png(path, width, height, rgba):
    path.parent.mkdir(parents=True, exist_ok=True)
    raw = b''.join(b'\x00' + bytes(rgba[y*width:(y+1)*width]) for y in range(height))
    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data) & 0xffffffff)
    path.write_bytes(
        b'\x89PNG\r\n\x1a\n'
        + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
        + chunk(b'IDAT', zlib.compress(raw, 9))
        + chunk(b'IEND', b'')
    )

def tex(width, height, fn):
    return [fn(x, y) for y in range(height) for x in range(width)]

random.seed(7)
write_png(ROOT/'noise'/'soft-value-128.png', 128, 128, tex(128, 128, lambda x,y: (v:= (x*37 ^ y*57 ^ (x*y)) & 63, v, v, 255)))
write_png(ROOT/'gradients'/'deep-violet-128.png', 128, 128, tex(128, 128, lambda x,y: (18+x//10, 10+y//12, 35+(x+y)//16, 255)))
write_png(ROOT/'paper'/'warm-paper-128.png', 128, 128, tex(128, 128, lambda x,y: (34+((x*13+y*3)&7), 30+((x*5+y*11)&7), 24+((x*7+y*17)&7), 255)))
write_png(ROOT/'metal'/'brushed-metal-128.png', 128, 128, tex(128, 128, lambda x,y: (38+(x%17), 40+(x%13), 43+(x%11), 255)))
write_png(ROOT/'starfields'/'dim-stars-128.png', 128, 128, tex(128, 128, lambda x,y: ((b:= 120 if ((x*928371 + y*1237) % 997) < 4 else 6), b, b+8 if b > 10 else b, 255)))
PY
```

- [ ] **Step 2: Add cubemap showcase shaders**

Each shader must include metadata. Example structure for `cubemap-atmospheric-sky.glsl`:

```glsl
/*! par-term shader metadata
name: Cubemap Atmospheric Sky
author: par-term
description: Low-distraction atmospheric cubemap gradient tuned for terminal readability.
version: 1.0.0
defaults:
  animation_speed: 0.2
  brightness: 0.35
  text_opacity: 1.0
  full_content: false
  cubemap: textures/cubemaps/env-outside
  cubemap_enabled: true
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = uv * 2.0 - 1.0;
    p.x *= iResolution.x / max(iResolution.y, 1.0);
    float t = iTime * 0.03;
    vec3 dir = normalize(vec3(p.x * 0.35 + sin(t) * 0.08, p.y * 0.25, 1.0));
    vec3 env = texture(iCubemap, dir).rgb;
    vec3 base = mix(vec3(0.015, 0.018, 0.032), vec3(0.06, 0.08, 0.12), smoothstep(-0.8, 1.0, p.y));
    vec3 color = mix(base, env * 0.18, 0.35);
    fragColor = vec4(color, 1.0);
}
```

Use similar conservative shaders for metallic ambience and neon room. They should sample `iCubemap`, use slow `iTime`, and keep output dark.

- [ ] **Step 3: Update `shaders/manifest.json`**

Run or write a small Python script that recomputes SHA256 entries for every file under `shaders/` that should be bundled. Preserve the existing schema:

```json
{
  "path": "textures/packs/noise/soft-value-128.png",
  "sha256": "...",
  "type": "texture",
  "category": "texture-pack-noise"
}
```

Set new shader entries to `type: "shader"`, category `effects` or `cubemap` if accepted by existing docs/tools.

- [ ] **Step 4: Run shader lint and commit**

Run:

```bash
cargo run -- shader-lint shaders/cubemap-metallic-ambience.glsl --no-prompt
cargo run -- shader-lint shaders/cubemap-neon-room.glsl --no-prompt
cargo run -- shader-lint shaders/cubemap-atmospheric-sky.glsl --no-prompt
```

Expected: pass without metadata/cubemap errors.

Commit:

```bash
git add shaders
git commit -m "feat: add shader texture packs and cubemap showcases"
```

---

### Task 5: Documentation and Ideas Cleanup

**Files:**
- Modify: `docs/CUSTOM_SHADERS.md`
- Modify: `docs/SHADERS.md`
- Modify: `docs/CONFIG_REFERENCE.md`
- Modify: `docs/INTEGRATIONS.md`
- Modify: `ideas.md`

- [ ] **Step 1: Update custom shader docs**

In `docs/CUSTOM_SHADERS.md`, document:

```markdown
### Built-in Noise Textures

Shader channels can use deterministic built-in textures without external image files:

```yaml
custom_shader_channel0: "builtin://noise/value-256"
custom_shader_channel1: "builtin://noise/fbm-512"
```

Supported values: `builtin://noise/value-128`, `builtin://noise/value-256`, `builtin://noise/fbm-256`, `builtin://noise/fbm-512`, `builtin://noise/cellular-256`.
```

Also document bundle manifests with required `author` and `description`, texture packs, and `iBackgroundBlendMode` constants.

- [ ] **Step 2: Update shader gallery docs**

In `docs/SHADERS.md`, add:

- new cubemap shaders in Cubemap-Based table.
- `Included Texture Packs` section listing noise, gradients, paper, metal, starfields.

- [ ] **Step 3: Update config and integration docs**

In `docs/CONFIG_REFERENCE.md`, add:

```markdown
| `custom_shader_background_channel0_blend_mode` | `enum` | `replace` | Blend-mode hint exposed as `iBackgroundBlendMode` when using background as `iChannel0`; values: `replace`, `multiply`, `screen`, `overlay`, `luminance_mask` |
```

In `docs/INTEGRATIONS.md`, update install-shaders text to say bundled shaders, cubemaps, and texture packs are installed and tracked by `manifest.json`.

- [ ] **Step 4: Clean up ideas.md**

Remove completed bullet items under `## Texture and asset workflows`. If the section becomes empty, remove the section header as well. Preserve unrelated ideas sections.

- [ ] **Step 5: Commit docs**

Run:

```bash
git diff -- docs/CUSTOM_SHADERS.md docs/SHADERS.md docs/CONFIG_REFERENCE.md docs/INTEGRATIONS.md ideas.md
```

Commit:

```bash
git add docs/CUSTOM_SHADERS.md docs/SHADERS.md docs/CONFIG_REFERENCE.md docs/INTEGRATIONS.md ideas.md
git commit -m "docs: document shader texture asset workflows"
```

---

### Task 6: Final Verification and Fixes

**Files:**
- Modify only files needed to fix verification failures.

- [ ] **Step 1: Run focused tests**

Run:

```bash
cargo test -p par-term-config shader_config --lib
cargo test -p par-term-config shader_bundle --lib
cargo test -p par-term-render builtin_textures --lib
cargo test -p par-term-settings-ui builtin_noise_choices_are_stable --lib
```

Expected: pass.

- [ ] **Step 2: Run canonical project verification**

Run:

```bash
make checkall
```

Expected: format, clippy, typecheck, and tests pass.

- [ ] **Step 3: Fix any failures with TDD**

For any behavior failure, write or adjust a focused failing test first, verify RED, then implement the smallest fix and rerun the failing command.

- [ ] **Step 4: Commit verification fixes if needed**

If fixes were needed:

```bash
git add <changed-files>
git commit -m "fix: address shader texture workflow verification"
```

If no fixes were needed, do not create an empty commit.
