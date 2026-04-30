# Shader Texture and Asset Workflows Design

## Summary

Complete the `ideas.md` Texture and asset workflows section with a pragmatic repo-local implementation. The feature set adds bundled texture packs, a documented per-shader asset bundle format, built-in generated noise texture channels, configurable background-as-channel0 blend modes, and several low-distraction cubemap showcase shaders.

## Goals

- Make `par-term install-shaders` install usable shader texture packs as part of the existing bundled shader asset ZIP.
- Define and parse a lightweight shader asset bundle manifest for directory-based shader packages.
- Let users select built-in procedural noise textures through `builtin://noise/...` channel paths.
- Extend `custom_shader_use_background_as_channel0` with a blend-mode setting exposed to GLSL.
- Add cubemap showcase shaders tuned for terminal readability.
- Document all user-facing behavior and remove completed Texture and asset workflow items from `ideas.md` after implementation.

## Non-Goals

- No external texture-pack marketplace or remote package manager.
- No new public hosting/dependency service.
- No true blur-behind implementation in this phase; it needs an additional blur pass and should be tracked separately if desired.
- No automatic screenshot generation pipeline for bundle manifests.

## Existing Context

- Shader installation already downloads and extracts the release shader ZIP via `src/shader_installer.rs` and `src/cli/install.rs`.
- Installed files are tracked by `shaders/manifest.json` and the manifest model in `par-term-update/src/manifest.rs`, including file type `texture`.
- Texture channels are resolved by config into `ResolvedShaderConfig.channel0..channel3` in `par-term-config/src/shader_config.rs` and loaded by `par-term-render/src/custom_shader_renderer/textures.rs`.
- Cubemaps are loaded by `par-term-render/src/custom_shader_renderer/cubemap.rs` and selected in Settings UI through `par-term-settings-ui/src/background_tab/global_channels.rs`.
- Shader metadata defaults already support channel paths, cubemap paths, and `use_background_as_channel0`.

## Feature Design

### 1. Bundled Texture Packs

Add organized texture pack directories under `shaders/textures/packs/`:

```text
shaders/textures/packs/noise/
shaders/textures/packs/gradients/
shaders/textures/packs/paper/
shaders/textures/packs/metal/
shaders/textures/packs/starfields/
```

The existing `install-shaders` command remains the installer. It installs these packs because release ZIP extraction and manifest tracking already include non-GLSL files. The root `shaders/manifest.json` should list the new pack files with `type: "texture"` and category values such as `texture-pack-noise`.

Texture pack assets should be small, terminal-readable, and suitable for repeated sampling. Prefer generated deterministic PNGs committed to the repo so release artifacts are reproducible without adding runtime image generation for pack files.

### 2. Per-Shader Asset Bundle Format

Support directory-based shader bundles inside the shader directory:

```text
~/.config/par-term/shaders/my-effect/
  shader.glsl
  manifest.json
  textures/noise.png
  cubemaps/studio-px.png
  cubemaps/studio-nx.png
  cubemaps/studio-py.png
  cubemaps/studio-ny.png
  cubemaps/studio-pz.png
  cubemaps/studio-nz.png
  screenshots/preview.png
  LICENSE
```

Bundle manifest schema:

```json
{
  "shader": "shader.glsl",
  "name": "My Effect",
  "author": "Jane Doe",
  "description": "Low-contrast animated paper texture tuned for terminal readability.",
  "license": "MIT",
  "textures": ["textures/noise.png"],
  "cubemaps": ["cubemaps/studio"],
  "screenshot": "screenshots/preview.png"
}
```

Required fields:

- `shader`
- `name`
- `author`
- `description`
- `license`

Optional fields:

- `textures`
- `cubemaps`
- `screenshot`

Validation rules:

- All paths are relative to the bundle directory.
- `shader` must point to an existing `.glsl` file.
- `textures` entries must point to existing image files.
- `cubemaps` entries are prefixes and must resolve to six cubemap face files using the existing cubemap suffix convention.
- `screenshot`, when set, must point to an existing image file.
- The bundle manifest describes package/distribution metadata; embedded shader metadata remains the source for runtime defaults.

Config may reference bundled shaders with existing relative paths such as `custom_shader: "my-effect/shader.glsl"`. No new config field is required for the shader path itself.

### 3. Generated Noise Channels

Add built-in texture path support for shader channels:

```yaml
custom_shader_channel0: "builtin://noise/value-256"
custom_shader_channel1: "builtin://noise/fbm-512"
custom_shader_channel2: "builtin://noise/cellular-256"
```

Supported built-in noise IDs:

- `builtin://noise/value-128`
- `builtin://noise/value-256`
- `builtin://noise/fbm-256`
- `builtin://noise/fbm-512`
- `builtin://noise/cellular-256`

Renderer behavior:

- Detect `builtin://noise/` paths before filesystem image loading.
- Generate deterministic RGBA8 textures on the CPU.
- Upload generated data with the same sampler behavior as file textures.
- Expose generated dimensions through `iChannelResolution`.
- Log an error and fall back to the transparent placeholder for unknown built-in IDs.

This keeps channel configuration compatible with existing metadata and per-shader overrides because built-in noise is represented as a string path.

### 4. Background Image Blend Modes

Keep the existing flag:

```yaml
custom_shader_use_background_as_channel0: true
```

Add a blend mode field:

```yaml
custom_shader_background_channel0_blend_mode: "replace"
```

Supported values:

- `replace` — current behavior; background texture is bound as `iChannel0`.
- `multiply`
- `screen`
- `overlay`
- `luminance_mask`

Expose the resolved mode to shaders as an integer uniform named `iBackgroundBlendMode`:

```glsl
const int BACKGROUND_BLEND_REPLACE = 0;
const int BACKGROUND_BLEND_MULTIPLY = 1;
const int BACKGROUND_BLEND_SCREEN = 2;
const int BACKGROUND_BLEND_OVERLAY = 3;
const int BACKGROUND_BLEND_LUMINANCE_MASK = 4;
```

The renderer still binds the background texture as `iChannel0` when `custom_shader_use_background_as_channel0` is enabled. The shader decides how to combine it with generated color or terminal content by reading `iBackgroundBlendMode`. This avoids adding an extra compositor pass and matches the existing shader-control model.

Settings UI should present the blend mode only near the existing “Use background as iChannel0” control.

### 5. Cubemap Showcase Shaders

Add three bundled background shaders:

- `cubemap-metallic-ambience.glsl` — slow metallic reflections with low contrast.
- `cubemap-neon-room.glsl` — subdued neon room ambience using cubemap sampling.
- `cubemap-atmospheric-sky.glsl` — slow atmospheric sky gradient from cubemap direction.

Each shader should:

- Include embedded shader metadata.
- Default to `textures/cubemaps/env-outside` or another bundled cubemap.
- Use `custom_shader_brightness`-friendly output with conservative defaults.
- Preserve terminal readability in normal background mode and full-content mode.
- Be listed in `docs/SHADERS.md` under the Cubemap-Based section.

## Code Structure

### Configuration

- Add `ShaderBackgroundBlendMode` enum in `par-term-config/src/types/shader.rs`.
- Add `custom_shader_background_channel0_blend_mode` to `GlobalShaderConfig`.
- Add per-shader override field `background_channel0_blend_mode` to `ShaderConfig` so metadata and user overrides can set it.
- Resolve the field into `ResolvedShaderConfig`.
- Update config reference docs.

### Renderer

- Extend custom shader uniforms to carry `iBackgroundBlendMode`.
- Add built-in texture generation helpers in `par-term-render/src/custom_shader_renderer/builtin_textures.rs` and call them from `textures.rs` before file loading.
- Keep file textures and generated textures behind the existing `ChannelTexture` abstraction.

### Bundle Manifest

- Add a small bundle manifest type and parser in `par-term-config/src/shader_bundle.rs` and export it from `par-term-config/src/lib.rs`.
- Use it for validation/discovery/documentation support. The initial implementation does not need a full install command because bundle directories are already usable by relative path.
- Add tests for required fields and path validation.

### Settings UI

- Add blend-mode dropdown near the existing “Use background as iChannel0” checkbox.
- Add built-in noise choices to channel controls while preserving free-text paths and Browse behavior.
- Keep UI changes narrow to `par-term-settings-ui/src/background_tab/global_channels.rs` and the existing temporary settings state fields in `par-term-settings-ui/src/settings_ui/mod.rs` / `state.rs` if the dropdown needs state mirroring.

### Documentation

Update:

- `docs/CUSTOM_SHADERS.md` — bundle format, built-in noise paths, blend mode uniform, texture packs.
- `docs/SHADERS.md` — texture pack list and new cubemap shaders.
- `docs/CONFIG_REFERENCE.md` — new config field and enum values.
- `docs/INTEGRATIONS.md` — clarify `install-shaders` installs bundled textures/texture packs.
- `ideas.md` — remove completed Texture and asset workflow items.

## Testing Strategy

- Config tests for default blend mode, YAML roundtrip, and per-shader override resolution.
- Renderer unit tests for built-in noise ID parsing and deterministic byte generation. GPU upload behavior can stay covered by existing renderer integration paths.
- Bundle manifest parser tests for required fields, missing paths, and valid relative bundle paths.
- Settings UI compile/typecheck coverage through existing workspace checks.
- Run `par-term shader-lint` on each new cubemap shader and fix metadata/channel/cubemap findings before final verification.

## Rollout

All changes are backward-compatible:

- Existing texture path strings continue to work.
- Existing `custom_shader_use_background_as_channel0` behavior is preserved by defaulting the blend mode to `replace`.
- Existing flat `.glsl` shader files remain supported.
- Bundle directories are additive.

## Risks and Mitigations

- **Risk:** Built-in noise generation could add startup cost.  
  **Mitigation:** Generate only configured channels and keep sizes small.

- **Risk:** Blend mode uniform may be misunderstood as automatic compositor blending.  
  **Mitigation:** Documentation will state that shaders apply the mode by sampling `iChannel0` and reading `iBackgroundBlendMode`.

- **Risk:** Bundle manifest overlaps with embedded shader metadata.  
  **Mitigation:** Manifest is package metadata; embedded shader metadata remains runtime defaults.

- **Risk:** Texture assets may bloat releases.  
  **Mitigation:** Keep pack images small and limited to practical terminal-readable defaults.
