# Per-Pane Background Image Rendering

**Date**: 2026-02-14
**Issue**: #148
**Status**: Design approved

## Summary

Extend par-term's background image rendering to support per-pane backgrounds. Each split pane can have its own background image, display mode (fit/fill/stretch/tile/center), and opacity, overriding the global background for that pane. Falls back to the global background when no per-pane image is configured.

## Current State

- `Pane` struct has an unused `background_image: Option<String>` field
- Background rendering uses a single global texture + bind group + uniform buffer
- The `background_image.wgsl` shader computes texture coordinates from `window_size` uniforms
- `render_split_panes()` renders the global background full-screen, then renders each pane's cells within scissor rects

## Approach: Per-Pane Bind Groups

Each pane with a custom background gets its own GPU texture + bind group + uniform buffer. The shader stays unchanged — we write pane dimensions into the `window_size` uniform and the existing scissor rect clips rendering to pane bounds.

### Why This Approach

- Minimal shader changes (none to `background_image.wgsl`)
- Aligns with how `render_pane_to_view` already works (scissor rect per pane)
- Texture cache handles deduplication naturally (same image path = shared texture)
- Clean separation between global and per-pane backgrounds

## Data Model

### PaneBackground struct

Replace the existing `background_image: Option<String>` on `Pane` with:

```rust
pub struct PaneBackground {
    pub image_path: Option<String>,
    pub mode: BackgroundImageMode,
    pub opacity: f32,
}
```

Each pane has full control over its own background mode and opacity.

### PaneRenderInfo extension

```rust
pub struct PaneRenderInfo<'a> {
    // ... existing fields ...
    pub background: Option<PaneBackground>,
}
```

## Texture Cache

Add a texture cache to `CellRenderer` keyed by image path:

```rust
pub(crate) pane_bg_cache: HashMap<String, PaneBackgroundEntry>,

struct PaneBackgroundEntry {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    width: u32,
    height: u32,
}
```

Multiple panes using the same image path share the texture data. Each pane gets its own bind group and uniform buffer at render time (since mode/opacity/pane-size differ).

## Rendering Flow

In `render_split_panes()`:

1. Custom shader renders full-screen (unchanged)
2. Global background image renders full-screen (unchanged)
3. **NEW**: For each pane with a per-pane background, render that background within the pane's scissor rect
4. Each pane's cells render on top (unchanged)

In `render_pane_to_view()`:

- Accept optional `PaneBackground` data
- If provided, render the pane-specific background instead of the global one
- Write pane dimensions into the uniform buffer's `window_size` field
- The existing scissor rect clips to pane bounds — no shader changes needed

## Shader

**No changes to `background_image.wgsl`**. The shader already computes texture coordinates from `image_size` and `window_size` uniforms. For per-pane rendering, we substitute pane dimensions into `window_size`. The shader doesn't need to know the difference.

## Configuration

```yaml
pane_backgrounds:
  - index: 0
    image: "~/images/bg1.png"
    mode: fill
    opacity: 0.8
  - index: 1
    image: "~/images/bg2.png"
    mode: fit
    opacity: 1.0
```

Settings persist across sessions via config.yaml.

## Settings UI

Add per-pane background controls to the existing background settings tab:

- Image path picker (for the currently focused pane)
- Mode dropdown (fit/fill/stretch/tile/center)
- Opacity slider (0.0-1.0)
- Clear button (revert to global background)

Search keywords: "pane background", "per-pane", "split background"

## Custom Shader Interaction

Custom shaders remain full-screen post-processing effects. They are unaware of pane boundaries. Per-pane backgrounds render between the custom shader layer and the cell layer:

```
1. Custom background shader (full-screen)
2. Global background image (full-screen)
3. Per-pane background images (scissor-clipped to each pane)
4. Per-pane terminal cells/text
5. Dividers, titles, focus indicators
6. Cursor shader (full-screen)
7. Sixel, egui, overlays
```

## Files to Modify

- `src/pane/types.rs` — Replace `background_image: Option<String>` with `PaneBackground` struct
- `src/cell_renderer/background.rs` — Add texture cache, per-pane loading/rendering methods
- `src/cell_renderer/mod.rs` — Add cache fields to `CellRenderer`
- `src/cell_renderer/render.rs` — Update `render_pane_to_view` to accept per-pane background
- `src/renderer/mod.rs` — Update `render_split_panes` to pass per-pane backgrounds, update `PaneRenderInfo`
- `src/config/mod.rs` — Add `PaneBackgroundConfig` and `pane_backgrounds` field
- `src/settings_ui/background_tab.rs` — Add per-pane background controls
- `src/settings_ui/sidebar.rs` — Add search keywords
- `src/tab/mod.rs` — Wire pane background into `PaneRenderInfo` construction
