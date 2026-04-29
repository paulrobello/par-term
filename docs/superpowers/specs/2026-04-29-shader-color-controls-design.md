# Shader Color Controls Design

## Summary

Extend shader uniform controls with color picker support for background shaders. Color controls attach to explicit `uniform vec3` or `uniform vec4` declarations and use the same metadata/defaults/override pipeline as slider and checkbox controls.

## Syntax

```glsl
// control color label="Tint"
uniform vec3 iTint;

// control color alpha=true label="Overlay"
uniform vec4 iOverlay;
```

Rules:

- `color` controls support `uniform vec3` and `uniform vec4` only.
- `vec3` controls are RGB-only.
- `vec4` controls support alpha; `alpha` defaults to `true` for `vec4` and `false` for `vec3`.
- `alpha=true` on `vec3` is invalid and skipped with a warning.
- `label="..."` is optional; the UI falls back to the uniform name.
- v1 supports up to 16 color controls per shader. Extras are non-fatal and use safe fallback behavior in shader compilation.

## Defaults and Overrides

Defaults live under existing shader metadata `defaults.uniforms`:

```yaml
defaults:
  uniforms:
    iTint: "#ff8800"
    iOverlay: "#ff8800cc"
```

Numeric arrays are also accepted:

```yaml
defaults:
  uniforms:
    iTint: [1.0, 0.53, 0.0]
    iOverlay: [1.0, 0.53, 0.0, 0.8]
```

Hex is the documented preferred format. User edits persist as per-shader overrides and take precedence over metadata defaults. Internally, colors are normalized to RGBA floats; `vec3` uploads alpha as `1.0`.

## Runtime

The renderer extends the custom controls uniform block with color `vec4` slots. The transpiler maps shader uniform names to color slots, preserving explicit shader uniform declarations in source but removing/replacing them before GLSL-to-WGSL transpilation.

## Settings UI

The Shader Controls section shows an egui color picker for color controls:

- RGB picker for `vec3` or `alpha=false` controls.
- RGBA picker for `vec4` with alpha enabled.
- Per-control reset clears only that uniform override.
- Save Defaults to Shader writes the effective normalized color to `defaults.uniforms`.
- Parser warnings are shown in both settings and the shader editor warning area.

## Assistant Guidance

After implementation, update the shader assistant context and Assistant Panel docs to teach agents the `// control color` syntax, defaults format, and vec3/vec4 alpha rules.

## Verification

Add tests for parser behavior, metadata hex/array parsing, renderer slot mapping, transpiler fallback behavior, settings effective-value normalization, and shader assistant context output. Run `make checkall` before completion.
