# Per-Pane Background Image Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable each split pane to have its own background image with independent mode and opacity, falling back to the global background when not set.

**Architecture:** Extend the existing background image pipeline with a per-path texture cache on `CellRenderer`. Each pane carries a `PaneBackground` struct through `PaneRenderInfo`. The `render_pane_to_view` method renders the per-pane background within the existing scissor rect. No shader changes needed — we write pane dimensions into the existing `window_size` uniform.

**Tech Stack:** Rust, wgpu, egui (settings UI), serde (config persistence)

---

### Task 1: Add PaneBackground Data Model

**Files:**
- Modify: `src/pane/types.rs:155` (replace `background_image` field)
- Modify: `src/pane/types.rs:282,369` (update constructors)
- Modify: `src/pane/types.rs:412-418` (update getter/setter)
- Modify: `src/config/types.rs` (add PaneBackgroundConfig)
- Modify: `src/config/mod.rs` (add pane_backgrounds field)

**Step 1: Define PaneBackground struct in `src/pane/types.rs`**

Add above the `Pane` struct definition:

```rust
/// Per-pane background image configuration
#[derive(Debug, Clone, Default)]
pub struct PaneBackground {
    /// Path to the background image (None = use global background)
    pub image_path: Option<String>,
    /// Display mode (fit/fill/stretch/tile/center)
    pub mode: crate::config::BackgroundImageMode,
    /// Opacity (0.0-1.0)
    pub opacity: f32,
}

impl PaneBackground {
    /// Create a new PaneBackground with default settings
    pub fn new() -> Self {
        Self {
            image_path: None,
            mode: crate::config::BackgroundImageMode::default(),
            opacity: 1.0,
        }
    }

    /// Returns true if this pane has a custom background image set
    pub fn has_image(&self) -> bool {
        self.image_path.is_some()
    }
}
```

**Step 2: Replace `background_image` field on Pane**

In `src/pane/types.rs`, replace line 155:
```rust
// OLD:
pub background_image: Option<String>,

// NEW:
/// Per-pane background settings (overrides global config if image_path is set)
pub background: PaneBackground,
```

Update both constructors (`new` at line 282 and `new_for_tmux` at line 369):
```rust
// OLD:
background_image: None,

// NEW:
background: PaneBackground::new(),
```

**Step 3: Update getter/setter methods**

Replace the existing getter/setter at lines 412-418:
```rust
// OLD:
pub fn set_background_image(&mut self, path: Option<String>) {
    self.background_image = path;
}
pub fn get_background_image(&self) -> Option<&str> {
    self.background_image.as_deref()
}

// NEW:
pub fn set_background(&mut self, background: PaneBackground) {
    self.background = background;
}

pub fn background(&self) -> &PaneBackground {
    &self.background
}

pub fn set_background_image(&mut self, path: Option<String>) {
    self.background.image_path = path;
}

pub fn get_background_image(&self) -> Option<&str> {
    self.background.image_path.as_deref()
}
```

**Step 4: Add PaneBackgroundConfig to config**

In `src/config/types.rs`, add after the `BackgroundMode` enum (~line 198):
```rust
/// Per-pane background image configuration (for config persistence)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneBackgroundConfig {
    /// Pane index (0-based)
    pub index: usize,
    /// Image path
    pub image: String,
    /// Display mode
    #[serde(default)]
    pub mode: BackgroundImageMode,
    /// Opacity
    #[serde(default = "super::defaults::background_image_opacity")]
    pub opacity: f32,
}
```

In `src/config/mod.rs`, add a field after `background_mode` (~line 363):
```rust
/// Per-pane background image configurations
#[serde(default)]
pub pane_backgrounds: Vec<crate::config::PaneBackgroundConfig>,
```

And in the `Default` impl (~line 1843):
```rust
pane_backgrounds: Vec::new(),
```

**Step 5: Fix any compilation errors from the field rename**

Search for all uses of `background_image` on Pane and update them. Run `cargo check` to find any remaining references.

**Step 6: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 7: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 8: Commit**

```bash
git add src/pane/types.rs src/config/types.rs src/config/mod.rs
git commit -m "feat(pane): add PaneBackground data model (#148)"
```

---

### Task 2: Add Texture Cache to CellRenderer

**Files:**
- Modify: `src/cell_renderer/mod.rs:21-180` (add cache fields)
- Modify: `src/cell_renderer/background.rs` (add cache methods)

**Step 1: Define PaneBackgroundEntry struct**

In `src/cell_renderer/background.rs`, add at the top of the file (after imports):

```rust
/// Cached GPU texture for a per-pane background image
pub(crate) struct PaneBackgroundEntry {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) width: u32,
    pub(crate) height: u32,
}
```

**Step 2: Add cache field to CellRenderer**

In `src/cell_renderer/mod.rs`, add after the `solid_bg_color` field (~line 156):

```rust
/// Cache of per-pane background textures keyed by image path
pub(crate) pane_bg_cache: HashMap<String, PaneBackgroundEntry>,
```

**Step 3: Initialize cache in CellRenderer constructor**

Find the CellRenderer constructor (in `src/cell_renderer/pipeline.rs` or wherever `CellRenderer { ... }` is built) and add:
```rust
pane_bg_cache: HashMap::new(),
```

**Step 4: Add cache load/evict methods to background.rs**

Add these methods to `impl CellRenderer` in `src/cell_renderer/background.rs`:

```rust
/// Load a per-pane background image into the texture cache.
/// Returns Ok(true) if the image was newly loaded, Ok(false) if already cached.
pub(crate) fn load_pane_background(&mut self, path: &str) -> Result<bool> {
    if self.pane_bg_cache.contains_key(path) {
        return Ok(false);
    }

    log::info!("Loading per-pane background image: {}", path);
    let img = image::open(path)
        .map_err(|e| {
            log::error!("Failed to open pane background image '{}': {}", path, e);
            e
        })?
        .to_rgba8();

    let (width, height) = img.dimensions();
    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pane bg image"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    self.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &img,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    self.pane_bg_cache.insert(
        path.to_string(),
        PaneBackgroundEntry {
            texture,
            view,
            sampler,
            width,
            height,
        },
    );

    Ok(true)
}

/// Remove a cached pane background texture
pub(crate) fn evict_pane_background(&mut self, path: &str) {
    self.pane_bg_cache.remove(path);
}

/// Clear all cached pane background textures
pub(crate) fn clear_pane_bg_cache(&mut self) {
    self.pane_bg_cache.clear();
}

/// Create a bind group and write uniforms for a per-pane background render.
/// Returns the bind group to use in the render pass.
pub(crate) fn create_pane_bg_bind_group(
    &self,
    entry: &PaneBackgroundEntry,
    pane_width: f32,
    pane_height: f32,
    mode: crate::config::BackgroundImageMode,
    opacity: f32,
) -> (wgpu::BindGroup, wgpu::Buffer) {
    // Create a uniform buffer with pane-specific dimensions
    let mut data = [0u8; 32];
    // image_size
    data[0..4].copy_from_slice(&(entry.width as f32).to_le_bytes());
    data[4..8].copy_from_slice(&(entry.height as f32).to_le_bytes());
    // window_size (actually pane size — shader doesn't know the difference)
    data[8..12].copy_from_slice(&pane_width.to_le_bytes());
    data[12..16].copy_from_slice(&pane_height.to_le_bytes());
    // mode
    data[16..20].copy_from_slice(&(mode as u32).to_le_bytes());
    // opacity (combine with window_opacity)
    let effective_opacity = opacity * self.window_opacity;
    data[20..24].copy_from_slice(&effective_opacity.to_le_bytes());

    let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("pane bg uniform buffer"),
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    self.queue.write_buffer(&uniform_buffer, 0, &data);

    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pane bg bind group"),
        layout: &self.bg_image_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&entry.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&entry.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniform_buffer.as_entire_binding(),
            },
        ],
    });

    (bind_group, uniform_buffer)
}
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 6: Commit**

```bash
git add src/cell_renderer/mod.rs src/cell_renderer/background.rs src/cell_renderer/pipeline.rs
git commit -m "feat(renderer): add per-pane background texture cache (#148)"
```

---

### Task 3: Wire Per-Pane Background Through Render Pipeline

**Files:**
- Modify: `src/renderer/mod.rs:46-65` (extend PaneRenderInfo)
- Modify: `src/renderer/mod.rs:1231-1382` (update render_split_panes)
- Modify: `src/cell_renderer/render.rs:1205-1328` (update render_pane_to_view)
- Modify: `src/app/window_state.rs:65-74` (extend PaneRenderData tuple)
- Modify: `src/app/window_state.rs:2544-2675` (pass pane background through data collection)
- Modify: `src/app/window_state.rs:3120-3162` (update render_split_panes_with_data)

**Step 1: Extend PaneRenderInfo**

In `src/renderer/mod.rs`, add to `PaneRenderInfo` struct (after `scroll_offset` at line 64):
```rust
/// Per-pane background image override (None = use global background)
pub background: Option<crate::pane::types::PaneBackground>,
```

**Step 2: Extend PaneRenderData type alias**

In `src/app/window_state.rs`, update the `PaneRenderData` type alias (~line 65) to add the pane background:
```rust
type PaneRenderData = (
    PaneViewport,
    Vec<crate::cell_renderer::Cell>,
    (usize, usize),
    Option<(usize, usize)>,
    f32,
    Vec<ScrollbackMark>,
    usize, // scrollback_len
    usize, // scroll_offset
    Option<crate::pane::types::PaneBackground>, // per-pane background
);
```

**Step 3: Pass pane background in data collection**

In `src/app/window_state.rs`, in the pane data collection loop (~line 2544), after reading `pane_scroll_offset` at line 2640, add:
```rust
let pane_background = if pane.background().has_image() {
    Some(pane.background().clone())
} else {
    None
};
```

Then update the `pane_data.push(...)` call (~line 2666) to include the background:
```rust
pane_data.push((
    viewport,
    cells,
    (cols, rows),
    cursor_pos,
    if is_focused { cursor_opacity } else { 0.0 },
    marks,
    pane_scrollback_len,
    pane_scroll_offset,
    pane_background,
));
```

**Step 4: Update render_split_panes_with_data destructuring**

In `src/app/window_state.rs`, update the destructuring in `render_split_panes_with_data` (~line 3136):
```rust
for (
    viewport,
    cells,
    grid_size,
    cursor_pos,
    cursor_opacity,
    marks,
    scrollback_len,
    scroll_offset,
    pane_background,
) in pane_data
```

And include it in `PaneRenderInfo` construction (~line 3151):
```rust
pane_render_infos.push(PaneRenderInfo {
    viewport,
    cells: unsafe { &*cells_ptr },
    grid_size,
    cursor_pos,
    cursor_opacity,
    show_scrollbar: false,
    marks,
    scrollback_len,
    scroll_offset,
    background: pane_background,
});
```

**Step 5: Update render_pane_to_view to accept and render per-pane background**

In `src/cell_renderer/render.rs`, update `render_pane_to_view` signature (~line 1205) to add a background parameter:
```rust
pub fn render_pane_to_view(
    &mut self,
    surface_view: &wgpu::TextureView,
    viewport: &PaneViewport,
    cells: &[Cell],
    cols: usize,
    rows: usize,
    cursor_pos: Option<(usize, usize)>,
    cursor_opacity: f32,
    show_scrollbar: bool,
    clear_first: bool,
    skip_background_image: bool,
    separator_marks: &[SeparatorMark],
    pane_background: Option<&crate::pane::types::PaneBackground>,
) -> Result<()> {
```

In the render pass section (~line 1293-1303), replace the global background rendering with per-pane-aware logic:
```rust
// Render background image within scissor rect
if !skip_background_image && !self.bg_is_solid_color {
    if let Some(pane_bg) = pane_background {
        // Per-pane background: load texture if needed, create bind group with pane dimensions
        if let Some(path) = &pane_bg.image_path {
            // Ensure texture is cached
            if !self.pane_bg_cache.contains_key(path.as_str()) {
                // We can't call load_pane_background here because we have &mut self
                // through the render pass. So we load it before the render pass.
                // This should already be loaded — log a warning if not.
                log::warn!("Pane background not pre-loaded: {}", path);
            }
            if let Some(entry) = self.pane_bg_cache.get(path.as_str()) {
                let (bind_group, _uniform_buf) = self.create_pane_bg_bind_group(
                    entry,
                    viewport.width,
                    viewport.height,
                    pane_bg.mode,
                    pane_bg.opacity,
                );
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }
        }
    } else if let Some(ref bg_bind_group) = self.bg_image_bind_group {
        // Fall back to global background
        render_pass.set_pipeline(&self.bg_image_pipeline);
        render_pass.set_bind_group(0, bg_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..4, 0..1);
    }
}
```

**Note:** The bind group and uniform buffer must be created **before** the render pass begins (wgpu doesn't allow resource creation during a render pass). Restructure so that the bind group is created before `encoder.begin_render_pass()`, then used inside. Store it as a local variable.

**Step 6: Pre-load pane background textures in render_split_panes**

In `src/renderer/mod.rs`, in `render_split_panes()` (~line 1231), before the pane rendering loop, add texture pre-loading:
```rust
// Pre-load any per-pane background textures that aren't cached yet
for pane in panes {
    if let Some(ref bg) = pane.background {
        if let Some(ref path) = bg.image_path {
            if let Err(e) = self.cell_renderer.load_pane_background(path) {
                log::error!("Failed to load pane background '{}': {}", path, e);
            }
        }
    }
}
```

**Step 7: Update the render_pane_to_view call in render_split_panes**

In `src/renderer/mod.rs`, update the call to `render_pane_to_view` (~line 1340) to pass the pane background:
```rust
self.cell_renderer.render_pane_to_view(
    &surface_view,
    &pane.viewport,
    pane.cells,
    pane.grid_size.0,
    pane.grid_size.1,
    pane.cursor_pos,
    pane.cursor_opacity,
    pane.show_scrollbar,
    false,
    has_background_image || has_custom_shader,
    &separator_marks,
    pane.background.as_ref(),
)?;
```

**Step 8: Update all other call sites of render_pane_to_view**

Search for all calls to `render_pane_to_view` and add `None` as the last argument (for the non-split-pane case).

**Step 9: Verify compilation and run tests**

Run: `cargo check && cargo test`
Expected: All pass

**Step 10: Commit**

```bash
git add src/renderer/mod.rs src/cell_renderer/render.rs src/app/window_state.rs
git commit -m "feat(renderer): wire per-pane backgrounds through render pipeline (#148)"
```

---

### Task 4: Handle Bind Group Lifecycle (Pre-Render Creation)

**Files:**
- Modify: `src/cell_renderer/render.rs:1205-1328` (restructure to create bind group before render pass)

Since wgpu doesn't allow creating buffers/bind groups during a render pass, the `create_pane_bg_bind_group` call must happen **before** `encoder.begin_render_pass()`.

**Step 1: Restructure render_pane_to_view**

Move bind group creation before the render pass:
```rust
// Pre-create per-pane background bind group if needed (must happen before render pass)
let pane_bg_bind_group = if !skip_background_image && !self.bg_is_solid_color {
    if let Some(pane_bg) = pane_background {
        if let Some(ref path) = pane_bg.image_path {
            self.pane_bg_cache.get(path.as_str()).map(|entry| {
                self.create_pane_bg_bind_group(
                    entry,
                    viewport.width,
                    viewport.height,
                    pane_bg.mode,
                    pane_bg.opacity,
                )
            })
        } else {
            None
        }
    } else {
        None
    }
} else {
    None
};
```

Then inside the render pass, use the pre-created bind group:
```rust
if !skip_background_image && !self.bg_is_solid_color {
    if let Some((ref bind_group, ref _buf)) = pane_bg_bind_group {
        render_pass.set_pipeline(&self.bg_image_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..4, 0..1);
    } else if pane_background.is_none() {
        // No per-pane bg, fall back to global
        if let Some(ref bg_bind_group) = self.bg_image_bind_group {
            render_pass.set_pipeline(&self.bg_image_pipeline);
            render_pass.set_bind_group(0, bg_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..4, 0..1);
        }
    }
}
```

**Step 2: Verify compilation and test**

Run: `cargo check && cargo test`

**Step 3: Commit**

```bash
git add src/cell_renderer/render.rs
git commit -m "fix(renderer): create pane bg bind groups before render pass (#148)"
```

---

### Task 5: Config Persistence for Pane Backgrounds

**Files:**
- Modify: `src/config/types.rs` (ensure PaneBackgroundConfig is complete)
- Modify: `src/config/mod.rs` (load/apply pane backgrounds on startup)
- Modify: `src/pane/types.rs` or `src/pane/manager.rs` (apply config to panes on creation)

**Step 1: Add config application in pane creation**

In `src/pane/types.rs`, in `Pane::new()` (~line 164), after creating the pane, apply per-pane background from config if available:

The caller (pane manager or tab) should apply the config. Add a method to Pane:
```rust
/// Apply a per-pane background configuration
pub fn apply_background_config(&mut self, config: &crate::config::PaneBackgroundConfig) {
    self.background = PaneBackground {
        image_path: Some(config.image.clone()),
        mode: config.mode,
        opacity: config.opacity,
    };
}
```

**Step 2: Apply pane backgrounds when creating splits**

In the pane manager or tab split logic, after creating a new pane, check the config's `pane_backgrounds` list and apply if the pane index matches.

**Step 3: Save pane backgrounds to config**

Add a method to collect current pane backgrounds and save them:
```rust
// In config or wherever appropriate
pub fn save_pane_backgrounds(&mut self, backgrounds: Vec<PaneBackgroundConfig>) {
    self.pane_backgrounds = backgrounds;
}
```

**Step 4: Verify compilation and test**

Run: `cargo check && cargo test`

**Step 5: Commit**

```bash
git add src/config/ src/pane/
git commit -m "feat(config): persist per-pane background settings (#148)"
```

---

### Task 6: Settings UI for Per-Pane Backgrounds

**Files:**
- Modify: `src/settings_ui/background_tab.rs` (add per-pane section)
- Modify: `src/settings_ui/sidebar.rs` (add search keywords)
- Modify: `src/settings_ui/mod.rs` (track focused pane for settings)

**Step 1: Add per-pane background controls to background_tab.rs**

Add a new collapsing header section in `show_background()`:

```rust
egui::CollapsingHeader::new("Per-Pane Background")
    .default_open(false)
    .show(ui, |ui| {
        ui.label("Set a background image for the currently focused pane.");
        ui.label("Overrides the global background for that pane only.");

        // Image path
        ui.horizontal(|ui| {
            ui.label("Image path:");
            let mut path = settings.focused_pane_bg_path.clone();
            if ui.text_edit_singleline(&mut path).changed() {
                settings.focused_pane_bg_path = path;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Mode dropdown
        ui.horizontal(|ui| {
            ui.label("Mode:");
            let modes = ["Fit", "Fill", "Stretch", "Tile", "Center"];
            let mut selected = settings.focused_pane_bg_mode as usize;
            egui::ComboBox::from_id_salt("pane_bg_mode")
                .selected_text(modes[selected])
                .show_ui(ui, |ui| {
                    for (i, mode) in modes.iter().enumerate() {
                        ui.selectable_value(&mut selected, i, *mode);
                    }
                });
            if selected != settings.focused_pane_bg_mode as usize {
                settings.focused_pane_bg_mode = match selected {
                    0 => BackgroundImageMode::Fit,
                    1 => BackgroundImageMode::Fill,
                    2 => BackgroundImageMode::Stretch,
                    3 => BackgroundImageMode::Tile,
                    4 => BackgroundImageMode::Center,
                    _ => BackgroundImageMode::default(),
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Opacity slider
        ui.horizontal(|ui| {
            ui.label("Opacity:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.focused_pane_bg_opacity,
                    0.0..=1.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Clear button
        if ui.button("Clear pane background").clicked() {
            settings.focused_pane_bg_path.clear();
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
```

**Step 2: Add fields to SettingsUI struct**

In `src/settings_ui/mod.rs`, add fields to track the focused pane's background settings:
```rust
pub(crate) focused_pane_bg_path: String,
pub(crate) focused_pane_bg_mode: BackgroundImageMode,
pub(crate) focused_pane_bg_opacity: f32,
```

Initialize them in the constructor.

**Step 3: Add search keywords**

In `src/settings_ui/sidebar.rs`, in the `tab_search_keywords()` for the Effects tab, add:
```rust
"per-pane background",
"pane image",
"split background",
```

**Step 4: Wire settings changes back to pane state**

In the settings action handler (where `SettingsWindowAction` is processed), handle changes to the focused pane's background by updating the pane's `PaneBackground` struct.

**Step 5: Verify compilation and test**

Run: `cargo check && cargo test`

**Step 6: Commit**

```bash
git add src/settings_ui/
git commit -m "feat(settings): add per-pane background controls (#148)"
```

---

### Task 7: Integration Testing and Polish

**Files:**
- Run: full test suite
- Modify: any files needed for bug fixes

**Step 1: Build release and manual test**

Run: `cargo build --release`

Test scenarios:
1. No per-pane backgrounds set — global background works as before
2. Set per-pane background on one split pane — only that pane shows the custom image
3. Multiple panes with different images — each shows its own
4. Same image on two panes with different modes — both render correctly
5. Clear per-pane background — falls back to global
6. Custom shader + per-pane background — shader renders full-screen, per-pane bg renders within pane bounds
7. Resize window with per-pane backgrounds — images scale correctly within pane bounds
8. Save config, restart — per-pane backgrounds persist

**Step 2: Run full check suite**

Run: `make checkall`
Expected: All pass

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat: per-pane background image rendering (#148)

Closes #148"
```

---

### Task 8: Create Pull Request

**Step 1: Push branch and create PR**

```bash
git push -u origin feat/per-pane-background-images
gh pr create --title "feat: implement per-pane background image rendering" --body "$(cat <<'EOF'
## Summary
- Each split pane can have its own background image with independent mode and opacity
- Falls back to global background when no per-pane image is configured
- Per-pane textures cached and deduplicated by image path
- No shader changes needed — reuses existing background_image.wgsl
- Settings UI for per-pane background controls
- Config persistence via pane_backgrounds list

Closes #148

## Test plan
- [ ] Global background still works without per-pane settings
- [ ] Per-pane background renders within pane scissor bounds
- [ ] Multiple panes with different images render correctly
- [ ] Same image on multiple panes with different modes works
- [ ] Clear per-pane background reverts to global
- [ ] Custom shader + per-pane background compositing works
- [ ] Window resize recalculates per-pane background coordinates
- [ ] Config saves and restores per-pane backgrounds
- [ ] All existing tests pass
EOF
)"
```
