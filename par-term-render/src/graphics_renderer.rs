use crate::error::RenderError;
use crate::gpu_utils;
use crate::wgpu_conversions::ImageScalingModeWgpu;
use par_term_config::ImageScalingMode;
use std::collections::HashMap;
use std::time::Instant;
use wgpu::*;

mod layout;
mod upload;

pub use layout::PaneRenderGeometry;
use layout::compute_graphic_geometry;
use upload::CachedTexture;

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
