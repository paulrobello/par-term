// ARC-009 TODO: When this file exceeds the 800-line limit, extract into a
// graphics_renderer/ sub-module directory:
//
//   upload.rs    — Texture upload / cache invalidation logic
//   layout.rs    — Graphics placement and scaling calculations
//
// Tracking: Issue ARC-009 in AUDIT.md.

use crate::error::RenderError;
use crate::gpu_utils;
use crate::wgpu_conversions::ImageScalingModeWgpu;
use par_term_config::ImageScalingMode;
use std::collections::HashMap;
use std::time::Instant;
use wgpu::*;

/// Maximum number of textures to cache before evicting least-recently-used entries.
/// This prevents unbounded GPU memory growth when displaying many inline images.
const MAX_TEXTURE_CACHE_SIZE: usize = 100;

/// Initial capacity of the graphics instance buffer (number of simultaneous inline images).
/// The buffer will grow automatically if more images are needed.
const INITIAL_GRAPHICS_INSTANCE_CAPACITY: usize = 32;

/// Instance data for a single sixel graphic
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SixelInstance {
    position: [f32; 2],   // Screen position (normalized 0-1)
    tex_coords: [f32; 4], // Texture coordinates (x, y, w, h) - normalized 0-1
    size: [f32; 2],       // Image size in screen space (normalized 0-1)
    alpha: f32,           // Global alpha multiplier
    _padding: f32,        // Padding to align to 16 bytes
}

/// Window and pane geometry for a single [`GraphicsRenderer::render_for_pane`] call.
#[derive(Debug, Clone, Copy)]
pub struct PaneRenderGeometry {
    pub window_width: f32,
    pub window_height: f32,
    pub pane_origin_x: f32,
    pub pane_origin_y: f32,
}

/// Compute UV coordinates and display size for an inline graphic instance.
///
/// Maps the source crop rectangle to the destination cell extent, applying
/// scroll clipping in destination space (so a scrolled row removes the same
/// fraction of source and destination). Returns `(tex_coords, size)`.
#[allow(clippy::too_many_arguments)]
fn compute_graphic_geometry(
    tex_w: f32,
    tex_h: f32,
    crop: [u32; 4],
    width_cells: usize,
    height_cells: usize,
    cell_w: f32,
    cell_h: f32,
    clip_px: f32,
    has_cols: bool,
    has_rows: bool,
    preserve_aspect: bool,
    is_virtual: bool,
    window_w: f32,
    window_h: f32,
) -> ([f32; 4], [f32; 2]) {
    let has_crop = crop != [0, 0, 0, 0];

    // Effective source rectangle (zero extents normalize to texture edges).
    let (sx, sy, sw, sh) = if has_crop && tex_w > 0.0 && tex_h > 0.0 {
        let x = (crop[0] as f32).min(tex_w);
        let y = (crop[1] as f32).min(tex_h);
        let w = if crop[2] > 0 {
            (crop[2] as f32).min(tex_w - x)
        } else {
            tex_w - x
        };
        let h = if crop[3] > 0 {
            (crop[3] as f32).min(tex_h - y)
        } else {
            tex_h - y
        };
        (x, y, w.max(0.0), h.max(0.0))
    } else {
        (0.0, 0.0, tex_w, tex_h)
    };

    // Un-clipped destination size in pixels, chosen per axis:
    // - Virtual / both c+r: exact cell rectangle.
    // - c-only: cell width × aspect-derived exact height (no cell rounding).
    // - r-only: cell height × aspect-derived exact width.
    // - neither: natural source crop size (or full texture when
    //   preserve_aspect, else cell-derived fallback).
    // Empty crop intersection (crop at the texture edge yields sw/sh=0)
    // produces nothing to draw; return zero size immediately.
    if sw <= 0.0 || sh <= 0.0 {
        return ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0]);
    }
    let aspect = sw / sh;
    let (dest_w, dest_h) = if is_virtual || (has_cols && has_rows) {
        (width_cells as f32 * cell_w, height_cells as f32 * cell_h)
    } else if has_cols && !has_rows {
        let dw = width_cells as f32 * cell_w;
        (dw, dw / aspect)
    } else if has_rows && !has_cols {
        let dh = height_cells as f32 * cell_h;
        (dh * aspect, dh)
    } else if has_crop && sw > 0.0 && sh > 0.0 {
        (sw, sh)
    } else if preserve_aspect && tex_w > 0.0 && tex_h > 0.0 {
        (tex_w, tex_h)
    } else {
        (width_cells as f32 * cell_w, height_cells as f32 * cell_h)
    };

    // Destination clip fraction.
    let visible_frac = if dest_h > 0.0 {
        ((dest_h - clip_px) / dest_h).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let scrolled_frac = if dest_h > 0.0 {
        (clip_px / dest_h).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // UV: map scrolled/visible destination fractions onto the source rect.
    let uv = if sw > 0.0 && sh > 0.0 && tex_w > 0.0 && tex_h > 0.0 {
        [
            sx / tex_w,
            (sy + sh * scrolled_frac) / tex_h,
            sw / tex_w,
            (sh * visible_frac) / tex_h,
        ]
    } else {
        [0.0, 0.0, 1.0, 1.0]
    };

    let size = (dest_w / window_w, dest_h * visible_frac / window_h);

    (uv, size.into())
}

#[cfg(test)]
mod geometry_tests {
    use super::compute_graphic_geometry;

    const WW: f32 = 800.0;
    const WH: f32 = 600.0;
    const CW: f32 = 10.0;
    const CH: f32 = 20.0;

    /// 100px source in 40px dest (r=2), scrolled 1 row (20px):
    /// clip fraction 0.5, UV starts at pixel 50, 50px visible.
    #[test]
    fn no_crop_both_cells_uses_dest_fraction_for_uv() {
        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            10,
            2,
            CW,
            CH,
            20.0, // clip_px
            true,
            true, // has_cols, has_rows
            false,
            false, // preserve_aspect, is_virtual
            WW,
            WH,
        );
        let expected_uv_y = 50.0 / 100.0;
        let expected_uv_h = 50.0 / 100.0;
        assert!((uv[1] - expected_uv_y).abs() < 1e-5);
        assert!((uv[3] - expected_uv_h).abs() < 1e-5);
        assert!((size[1] - 20.0 / WH).abs() < 1e-5);
    }

    /// 25px source crop, no c/r, scrolled 20px: dest_h=25, 5px visible.
    #[test]
    fn natural_crop_without_cells_uses_crop_height_for_dest() {
        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 25],
            1,
            1,
            CW,
            CH,
            20.0,
            false,
            false,
            false,
            false,
            WW,
            WH,
        );
        let expected_uv_y = (0.0 + 25.0 * 0.8) / 100.0;
        let expected_uv_h = (25.0 * 0.2) / 100.0;
        assert!((uv[1] - expected_uv_y).abs() < 1e-5);
        assert!((uv[3] - expected_uv_h).abs() < 1e-5);
        assert!((size[1] - 5.0 / WH).abs() < 1e-5);
    }

    /// row=-1, Y=5: clip 15px, UV reflects 15/60 scrolled fraction.
    #[test]
    fn y_offset_produces_sub_row_clip() {
        let top_px = -1.0 * CH + 5.0;
        let clip_px = (-top_px).max(0.0);
        assert_eq!(clip_px, 15.0);

        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            10,
            3,
            CW,
            CH,
            clip_px,
            true,
            true,
            false,
            false,
            WW,
            WH,
        );
        assert!((uv[1] - 25.0 / 100.0).abs() < 1e-5);
        assert!((uv[3] - 75.0 / 100.0).abs() < 1e-5);
        assert!((size[1] - 45.0 / WH).abs() < 1e-5);
    }

    /// 100×100 source, c=5 only, cell 10×20: dest_w=50, dest_h=50 (aspect 1:1).
    #[test]
    fn c_only_computes_exact_height_from_aspect() {
        let (_uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            5,
            3,
            CW,
            CH,
            0.0,
            true,
            false, // has_cols only
            false,
            false,
            WW,
            WH,
        );
        // dest_h = 50px / 600px (aspect-derived, not cell-rounded)
        assert!((size[0] - 50.0 / WW).abs() < 1e-5);
        assert!((size[1] - 50.0 / WH).abs() < 1e-5);
    }

    /// 100×100 source, r=2 only, cell 10×20: dest_h=40, dest_w=40 (aspect 1:1).
    #[test]
    fn r_only_computes_exact_width_from_aspect() {
        let (_uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            4,
            2,
            CW,
            CH,
            0.0,
            false,
            true, // has_rows only
            false,
            false,
            WW,
            WH,
        );
        assert!((size[0] - 40.0 / WW).abs() < 1e-5);
        assert!((size[1] - 40.0 / WH).abs() < 1e-5);
    }

    /// 100×50 source (2:1), c=5, cell 10×20: dest_w=50, dest_h=25.
    #[test]
    fn c_only_wide_source_computes_proportional_height() {
        let (_uv, size) = compute_graphic_geometry(
            100.0,
            50.0,
            [0, 0, 0, 0],
            5,
            1,
            CW,
            CH,
            0.0,
            true,
            false,
            false,
            false,
            WW,
            WH,
        );
        assert!((size[0] - 50.0 / WW).abs() < 1e-5);
        assert!((size[1] - 25.0 / WH).abs() < 1e-5);
    }

    /// Crop at the texture edge (source_x=100 on 100px image) yields sw=0.
    /// Zero-size intersection must return zero output, not a full-image
    /// fallback or NaN from aspect division.
    #[test]
    fn zero_size_crop_at_edge_returns_zero_output() {
        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [100, 0, 0, 0],
            5,
            3,
            CW,
            CH,
            0.0,
            true,
            false,
            false,
            false,
            WW,
            WH,
        );
        // Zero crop → zero UV and zero size
        assert_eq!(uv, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(size, [0.0, 0.0]);
    }
}

/// Parameters describing a single inline graphic to render.
///
/// Passed as a slice to [`GraphicsRenderer::render`] and
/// [`GraphicsRenderer::render_for_pane`] so that callers use named fields
/// rather than a positional 7-element tuple.
#[derive(Debug, Clone, Copy)]
pub struct GraphicRenderInfo {
    /// Unique identifier for this graphic (used to look up the cached texture)
    pub id: u64,
    /// Screen row at which the graphic starts (can be negative when scrolled partially off top)
    pub screen_row: isize,
    /// Screen column at which the graphic starts
    pub col: usize,
    /// Width of the graphic in terminal cells
    pub width_cells: usize,
    /// Height of the graphic in terminal cells
    pub height_cells: usize,
    /// Global alpha multiplier (0.0 = fully transparent, 1.0 = fully opaque)
    pub alpha: f32,
    /// Number of rows clipped from the top when the graphic is partially scrolled off-screen
    pub scroll_offset_rows: usize,
    /// Kitty destination pixel offsets within the first cell.
    pub destination_offset_x: u32,
    pub destination_offset_y: u32,
    /// Source crop rectangle in native texture pixels: x, y, width, height.
    pub source_crop: [u32; 4],
    /// Whether Kitty supplied `c=` (columns) in the placement.
    pub has_cols: bool,
    /// Whether Kitty supplied `r=` (rows) in the placement.
    pub has_rows: bool,
}

/// Metadata for a cached sixel texture
struct SixelTextureInfo {
    texture: Texture,
    #[allow(dead_code)] // GPU lifetime: must outlive the bind_group which references this view
    view: TextureView,
    bind_group: BindGroup,
    width: u32,
    height: u32,
}

/// Cached texture wrapper with LRU tracking
struct CachedTexture {
    texture: SixelTextureInfo,
    /// Timestamp of last access for LRU eviction
    last_used: Instant,
}

/// Graphics renderer for sixel images
pub struct GraphicsRenderer {
    // Rendering pipeline
    pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
    sampler: Sampler,

    // Instance buffer
    instance_buffer: Buffer,
    instance_capacity: usize,

    // Texture cache: maps sixel ID to texture info with LRU tracking
    texture_cache: HashMap<u64, CachedTexture>,

    // Cell dimensions for positioning
    cell_width: f32,
    cell_height: f32,
    window_padding: f32,
    /// Vertical offset for content (e.g., tab bar height)
    content_offset_y: f32,
    /// Horizontal offset for content (e.g., tab bar on left)
    content_offset_x: f32,

    /// Global config: whether to preserve aspect ratio when rendering images
    preserve_aspect_ratio: bool,
}

impl GraphicsRenderer {
    /// Create a new graphics renderer
    pub fn new(
        device: &Device,
        surface_format: TextureFormat,
        cell_width: f32,
        cell_height: f32,
        window_padding: f32,
        scaling_mode: ImageScalingMode,
        preserve_aspect_ratio: bool,
    ) -> Result<Self, RenderError> {
        // Create bind group layout for sixel textures
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sixel Bind Group Layout"),
            entries: &[
                // Sixel texture
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create sampler with configured filter mode
        let sampler = gpu_utils::create_sampler_with_filter(
            device,
            scaling_mode.to_filter_mode(),
            Some("Sixel Sampler"),
        );

        // Create rendering pipeline
        let pipeline = Self::create_pipeline(device, surface_format, &bind_group_layout)?;

        // Create instance buffer (initial capacity for INITIAL_GRAPHICS_INSTANCE_CAPACITY images)
        let initial_capacity = INITIAL_GRAPHICS_INSTANCE_CAPACITY;
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sixel Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<SixelInstance>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            pipeline,
            bind_group_layout,
            sampler,
            instance_buffer,
            instance_capacity: initial_capacity,
            texture_cache: HashMap::new(),
            cell_width,
            cell_height,
            window_padding,
            content_offset_y: 0.0,
            content_offset_x: 0.0,
            preserve_aspect_ratio,
        })
    }

    /// Create the sixel rendering pipeline
    fn create_pipeline(
        device: &Device,
        format: TextureFormat,
        bind_group_layout: &BindGroupLayout,
    ) -> Result<RenderPipeline, RenderError> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Sixel Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/sixel.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Sixel Pipeline Layout"),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });

        Ok(device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Sixel Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(VertexBufferLayout {
                    array_stride: std::mem::size_of::<SixelInstance>() as u64,
                    step_mode: VertexStepMode::Instance,
                    attributes: &vertex_attr_array![
                        0 => Float32x2,  // position
                        1 => Float32x4,  // tex_coords
                        2 => Float32x2,  // size
                        3 => Float32,    // alpha
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    // Use premultiplied alpha blending since shader outputs premultiplied colors
                    blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            cache: None,
            multiview_mask: None,
        }))
    }

    /// Create or get a cached texture for a sixel graphic
    ///
    /// # Arguments
    /// * `device` - WGPU device for creating textures
    /// * `queue` - WGPU queue for writing texture data
    /// * `id` - Unique identifier for this sixel graphic
    /// * `rgba_data` - RGBA pixel data (width * height * 4 bytes)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    pub fn get_or_create_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        id: u64,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        // Check if texture already exists in cache
        // For animations, we need to update the texture data even if it exists
        if let Some(cached) = self.texture_cache.get_mut(&id) {
            // Update LRU timestamp on cache hit
            cached.last_used = Instant::now();

            // Kitty TGP virtual placements (high-bit flag set on the cache id;
            // see par-term-render/src/renderer/graphics.rs) reuse the same
            // image data every frame — they're static placements anchored by
            // grid placeholder cells, not animations. Re-uploading the
            // pixels per frame here costs ~640 KB × 60 fps for a 400×400
            // image, saturating the GPU command queue and freezing the pane.
            // For these IDs, treat the cache hit as final.
            const VIRTUAL_PLACEMENT_ID_FLAG: u64 = 1u64 << 63;
            if id & VIRTUAL_PLACEMENT_ID_FLAG != 0 {
                return Ok(());
            }

            // Texture exists - update it if the data might have changed
            // Validate data size
            let expected_size = (width * height * 4) as usize;
            if rgba_data.len() != expected_size {
                return Err(RenderError::InvalidTextureData {
                    expected: expected_size,
                    actual: rgba_data.len(),
                });
            }

            // Update existing texture with new pixel data (for animations)
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &cached.texture.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                rgba_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );

            return Ok(());
        }

        // Validate data size
        let expected_size = (width * height * 4) as usize;
        if rgba_data.len() != expected_size {
            return Err(RenderError::InvalidTextureData {
                expected: expected_size,
                actual: rgba_data.len(),
            });
        }

        // Evict least-recently-used texture if cache is full
        if self.texture_cache.len() >= MAX_TEXTURE_CACHE_SIZE
            && let Some((&lru_id, _)) = self
                .texture_cache
                .iter()
                .min_by_key(|(_, cached)| cached.last_used)
        {
            log::debug!(
                "[GRAPHICS] Evicting LRU texture: id={}, cache_size={}",
                lru_id,
                self.texture_cache.len()
            );
            self.texture_cache.remove(&lru_id);
        }

        // Create texture
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("Sixel Texture {}", id)),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Write RGBA data to texture
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            rgba_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&TextureViewDescriptor::default());

        // Create bind group for this texture
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("Sixel Bind Group {}", id)),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // Cache texture info with current timestamp
        self.texture_cache.insert(
            id,
            CachedTexture {
                texture: SixelTextureInfo {
                    texture,
                    view,
                    bind_group,
                    width,
                    height,
                },
                last_used: Instant::now(),
            },
        );

        log::debug!(
            "[GRAPHICS] Created sixel texture: id={}, size={}x{}, cache_size={}/{}",
            id,
            width,
            height,
            self.texture_cache.len(),
            MAX_TEXTURE_CACHE_SIZE
        );

        Ok(())
    }

    /// Render sixel graphics
    ///
    /// # Arguments
    /// * `device` - WGPU device for creating buffers
    /// * `queue` - WGPU queue for writing buffer data
    /// * `render_pass` - Active render pass to render into
    /// * `graphics` - Slice of [`GraphicRenderInfo`] describing each graphic's position and dimensions
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut RenderPass,
        graphics: &[GraphicRenderInfo],
        window_width: f32,
        window_height: f32,
    ) -> Result<(), RenderError> {
        if graphics.is_empty() {
            return Ok(());
        }

        // Build instance data
        let mut instances = Vec::with_capacity(graphics.len());
        for g in graphics {
            let (
                id,
                row,
                col,
                _width_cells,
                _height_cells,
                alpha,
                _scroll_offset_rows,
                dest_off_x,
                dest_off_y,
                crop,
                has_cols,
                has_rows,
            ) = (
                g.id,
                g.screen_row,
                g.col,
                g.width_cells,
                g.height_cells,
                g.alpha,
                g.scroll_offset_rows,
                g.destination_offset_x,
                g.destination_offset_y,
                g.source_crop,
                g.has_cols,
                g.has_rows,
            );
            // Check if texture exists and update LRU timestamp
            if let Some(cached) = self.texture_cache.get_mut(&id) {
                cached.last_used = Instant::now();
                let tex_info = &cached.texture;

                // Signed pixel-space top relative to content area. A Y
                // offset can place the top at a non-row-aligned position,
                // so clipping must be computed in pixels, not integer rows.
                let top_px = row as f32 * self.cell_height + dest_off_y as f32;
                let clip_px = (-top_px).max(0.0);
                let draw_y_px = top_px.max(0.0);
                let x = (self.window_padding
                    + self.content_offset_x
                    + col as f32 * self.cell_width
                    + dest_off_x as f32)
                    / window_width;
                let y = (self.window_padding + self.content_offset_y + draw_y_px) / window_height;

                const VIRTUAL_PLACEMENT_ID_FLAG: u64 = 1u64 << 63;
                let is_virtual_placement = id & VIRTUAL_PLACEMENT_ID_FLAG != 0;
                let (tex_coords, size) = compute_graphic_geometry(
                    tex_info.width as f32,
                    tex_info.height as f32,
                    crop,
                    _width_cells,
                    _height_cells,
                    self.cell_width,
                    self.cell_height,
                    clip_px,
                    has_cols,
                    has_rows,
                    self.preserve_aspect_ratio,
                    is_virtual_placement,
                    window_width,
                    window_height,
                );

                instances.push(SixelInstance {
                    position: [x, y],
                    tex_coords,
                    size,
                    alpha,
                    _padding: 0.0,
                });
            }
        }

        if instances.is_empty() {
            return Ok(());
        }

        // Debug: log sixel rendering
        log::debug!(
            "[GRAPHICS] Rendering {} sixel graphics (from {} total graphics provided)",
            instances.len(),
            graphics.len()
        );

        // Resize instance buffer if needed
        let required_capacity = instances.len();
        if required_capacity > self.instance_capacity {
            let new_capacity = (required_capacity * 2).max(32);
            self.instance_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Sixel Instance Buffer"),
                size: (new_capacity * std::mem::size_of::<SixelInstance>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_capacity;
        }

        // Write instance data to buffer
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        // Set pipeline
        render_pass.set_pipeline(&self.pipeline);

        // Render each graphic with its specific bind group
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

        // Use separate counter for instance index since we filtered out graphics without textures
        let mut instance_idx = 0u32;
        for g in graphics {
            if let Some(cached) = self.texture_cache.get(&g.id) {
                render_pass.set_bind_group(0, &cached.texture.bind_group, &[]);
                render_pass.draw(0..4, instance_idx..(instance_idx + 1));
                instance_idx += 1;
            }
        }

        Ok(())
    }

    /// Render sixel graphics for a specific pane using explicit origin coordinates.
    ///
    /// Identical to [`Self::render`] but uses `pane_origin_x`/`pane_origin_y` for positioning
    /// instead of the global `window_padding + content_offset` values, so graphics are
    /// placed relative to the pane rather than the full window.
    ///
    /// # Arguments
    /// * `device` - WGPU device for creating buffers
    /// * `queue` - WGPU queue for writing buffer data
    /// * `render_pass` - Active render pass to render into
    /// * `graphics` - Slice of [`GraphicRenderInfo`] describing each graphic's position and dimensions
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    /// * `pane_origin_x` - X pixel coordinate of the pane's content origin
    /// * `pane_origin_y` - Y pixel coordinate of the pane's content origin
    pub fn render_for_pane(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut RenderPass,
        graphics: &[GraphicRenderInfo],
        pane_geometry: PaneRenderGeometry,
    ) -> Result<(), RenderError> {
        let PaneRenderGeometry {
            window_width,
            window_height,
            pane_origin_x,
            pane_origin_y,
        } = pane_geometry;
        if graphics.is_empty() {
            return Ok(());
        }

        // Build instance data
        let mut instances = Vec::with_capacity(graphics.len());
        for g in graphics {
            let (
                id,
                row,
                col,
                _width_cells,
                _height_cells,
                alpha,
                _scroll_offset_rows,
                dest_off_x,
                dest_off_y,
                crop,
                has_cols,
                has_rows,
            ) = (
                g.id,
                g.screen_row,
                g.col,
                g.width_cells,
                g.height_cells,
                g.alpha,
                g.scroll_offset_rows,
                g.destination_offset_x,
                g.destination_offset_y,
                g.source_crop,
                g.has_cols,
                g.has_rows,
            );
            // Check if texture exists and update LRU timestamp
            if let Some(cached) = self.texture_cache.get_mut(&id) {
                cached.last_used = Instant::now();
                let tex_info = &cached.texture;

                let top_px = row as f32 * self.cell_height + dest_off_y as f32;
                let clip_px = (-top_px).max(0.0);
                let draw_y_px = top_px.max(0.0);
                let x = (pane_origin_x + col as f32 * self.cell_width + dest_off_x as f32)
                    / window_width;
                let y = (pane_origin_y + draw_y_px) / window_height;

                const VIRTUAL_PLACEMENT_ID_FLAG: u64 = 1u64 << 63;
                let is_virtual_placement = id & VIRTUAL_PLACEMENT_ID_FLAG != 0;
                let (tex_coords, size) = compute_graphic_geometry(
                    tex_info.width as f32,
                    tex_info.height as f32,
                    crop,
                    _width_cells,
                    _height_cells,
                    self.cell_width,
                    self.cell_height,
                    clip_px,
                    has_cols,
                    has_rows,
                    self.preserve_aspect_ratio,
                    is_virtual_placement,
                    window_width,
                    window_height,
                );

                instances.push(SixelInstance {
                    position: [x, y],
                    tex_coords,
                    size,
                    alpha,
                    _padding: 0.0,
                });
            }
        }

        if instances.is_empty() {
            return Ok(());
        }

        // Resize instance buffer if needed
        let required_capacity = instances.len();
        if required_capacity > self.instance_capacity {
            let new_capacity = (required_capacity * 2).max(32);
            self.instance_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("Sixel Instance Buffer"),
                size: (new_capacity * std::mem::size_of::<SixelInstance>()) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_capacity = new_capacity;
        }

        // Write instance data to buffer
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));

        // Set pipeline
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

        let mut instance_idx = 0u32;
        for g in graphics {
            if let Some(cached) = self.texture_cache.get(&g.id) {
                render_pass.set_bind_group(0, &cached.texture.bind_group, &[]);
                render_pass.draw(0..4, instance_idx..(instance_idx + 1));
                instance_idx += 1;
            }
        }

        Ok(())
    }

    /// Remove a texture from the cache
    pub fn remove_texture(&mut self, id: u64) {
        self.texture_cache.remove(&id);
    }

    /// Clear all cached textures
    pub fn clear_cache(&mut self) {
        self.texture_cache.clear();
    }

    /// Get the number of cached textures
    pub fn cache_size(&self) -> usize {
        self.texture_cache.len()
    }

    /// Update cell dimensions (called when window is resized)
    pub fn update_cell_dimensions(
        &mut self,
        cell_width: f32,
        cell_height: f32,
        window_padding: f32,
    ) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.window_padding = window_padding;
    }

    /// Set vertical content offset (e.g., tab bar height)
    pub fn set_content_offset_y(&mut self, offset: f32) {
        self.content_offset_y = offset;
    }

    /// Set horizontal content offset (e.g., tab bar on left)
    pub fn set_content_offset_x(&mut self, offset: f32) {
        self.content_offset_x = offset;
    }

    /// Update the global aspect ratio preservation setting.
    pub fn set_preserve_aspect_ratio(&mut self, preserve: bool) {
        self.preserve_aspect_ratio = preserve;
    }

    /// Update the texture scaling mode (nearest vs linear filtering).
    ///
    /// This recreates the sampler and invalidates all cached textures
    /// since their bind groups reference the old sampler.
    pub fn update_scaling_mode(&mut self, device: &Device, scaling_mode: ImageScalingMode) {
        self.sampler = gpu_utils::create_sampler_with_filter(
            device,
            scaling_mode.to_filter_mode(),
            Some("Sixel Sampler"),
        );
        // Clear texture cache since bind groups reference the old sampler
        self.texture_cache.clear();
    }
}
