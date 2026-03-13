# par-term-render

GPU-accelerated rendering engine for the par-term terminal emulator.

This crate provides the full rendering pipeline: cell-based GPU rendering with a glyph
atlas, inline graphics (Sixel, iTerm2, Kitty), custom GLSL post-processing shaders
(Shadertoy/Ghostty compatible), scrollbar rendering with mark overlays, and background
image support. All rendering is performed via wgpu.

## What This Crate Provides

- `CellRenderer` — main renderer for terminal cells; manages the glyph atlas, instance
  buffers, and per-frame draw call generation
- `PaneViewport` — viewport parameters for a single terminal pane
- `Renderer` — top-level compositor: manages split-pane layouts, dividers, pane titles,
  and orchestrates three-phase draw calls (backgrounds → text → cursor overlays)
- `RendererParams` — per-frame parameters passed to the renderer
- `CustomShaderRenderer` — GLSL-to-WGSL transpilation and post-processing pipeline
- `GraphicsRenderer` — inline graphics rendering (Sixel, iTerm2, Kitty protocol)
- `Scrollbar` — scrollbar rendering with scrollback mark overlays
- `RenderError` — error type for GPU initialization and render failures

## Rendering Architecture

The renderer uses three render phases per frame, enforced by `emit_three_phase_draw_calls()`:

1. **Backgrounds** — cell background quads (`cell_bg.wgsl`)
2. **Text** — glyph instances from the atlas (`cell_text.wgsl`)
3. **Cursor overlays** — beam, underline, and hollow cursor shapes

This ordering is mandatory: cursor overlays must render after text, or beam/underline
cursors are hidden under glyph quads.

## GPU Pipelines

| Pipeline | Shader | Purpose |
|----------|--------|---------|
| Background | `cell_bg.wgsl` | Cell background colors |
| Text | `cell_text.wgsl` | Glyph atlas sampling |
| Background image | `background_image.wgsl` | Full-window background image |
| Visual bell | `visual_bell.wgsl` | Flash overlay |
| Opaque alpha | `opaque_alpha.wgsl` | Alpha channel safeguard (macOS) |
| Custom shader | GLSL → WGSL | User post-processing effects |

## Workspace Position

Layer 3 in the dependency graph. Depends on `par-term-config` (Layer 1) and
`par-term-fonts` (Layer 2). Used directly by the root `par-term` crate.

## Related Documentation

- [Custom Shaders](../docs/CUSTOM_SHADERS.md) — background and cursor shader documentation
- [Shader Gallery](../docs/SHADERS.md) — included shader reference
- [GPU Compositor](../docs/COMPOSITOR.md) — render layer details
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
