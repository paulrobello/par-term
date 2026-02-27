use crate::error::RenderError;
use crate::gpu_utils;
use par_term_config::ImageScalingMode;
use std::collections::HashMap;
use std::time::Instant;
use wgpu::*;

/// Maximum number of textures to cache before evicting least-recently-used entries.
/// This prevents unbounded GPU memory growth when displaying many inline images.
const MAX_TEXTURE_CACHE_SIZE: usize = 100;

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

        // Create instance buffer (initial capacity for 32 images)
        let initial_capacity = 32;
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
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        Ok(device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Sixel Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<SixelInstance>() as u64,
                    step_mode: VertexStepMode::Instance,
                    attributes: &vertex_attr_array![
                        0 => Float32x2,  // position
                        1 => Float32x4,  // tex_coords
                        2 => Float32x2,  // size
                        3 => Float32,    // alpha
                    ],
                }],
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
            multiview: None,
            cache: None,
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
    /// * `graphics` - Slice of sixel graphics to render with their positions
    ///   Each tuple contains: (id, row, col, width_in_cells, height_in_cells, alpha, scroll_offset_rows)
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut RenderPass,
        graphics: &[(u64, isize, usize, usize, usize, f32, usize)],
        window_width: f32,
        window_height: f32,
    ) -> Result<(), RenderError> {
        if graphics.is_empty() {
            return Ok(());
        }

        // Build instance data
        let mut instances = Vec::with_capacity(graphics.len());
        for &(id, row, col, _width_cells, _height_cells, alpha, scroll_offset_rows) in graphics {
            // Check if texture exists and update LRU timestamp
            if let Some(cached) = self.texture_cache.get_mut(&id) {
                cached.last_used = Instant::now();
                let tex_info = &cached.texture;

                // Calculate screen position (normalized 0-1, origin top-left)
                // When scroll_offset_rows > 0, the image is partially scrolled off the top.
                // Advance the y position by scroll_offset_rows so the visible portion
                // starts at the correct screen row instead of above the viewport.
                let adjusted_row = row + scroll_offset_rows as isize;
                let x =
                    (self.window_padding + self.content_offset_x + col as f32 * self.cell_width)
                        / window_width;
                let y = (self.window_padding
                    + self.content_offset_y
                    + adjusted_row as f32 * self.cell_height)
                    / window_height;

                // Calculate texture V offset for scrolled graphics
                // scroll_offset_rows = terminal rows scrolled off top
                // Each terminal row = cell_height pixels
                let tex_v_start = if scroll_offset_rows > 0 && tex_info.height > 0 {
                    let pixels_scrolled = scroll_offset_rows as f32 * self.cell_height;
                    (pixels_scrolled / tex_info.height as f32).min(0.99)
                } else {
                    0.0
                };
                let tex_v_height = 1.0 - tex_v_start;

                // Calculate display size based on aspect ratio preservation setting
                let (width, height) = if self.preserve_aspect_ratio {
                    // Use actual texture pixel dimensions to preserve aspect ratio
                    // Rather than converting pixels→cells→pixels (which distorts non-square cells)
                    let visible_height_pixels = if scroll_offset_rows > 0 {
                        (tex_info.height as f32 * tex_v_height).max(1.0)
                    } else {
                        tex_info.height as f32
                    };
                    (
                        tex_info.width as f32 / window_width,
                        visible_height_pixels / window_height,
                    )
                } else {
                    // Stretch to fill cell grid (ignore image aspect ratio)
                    let cell_w = _width_cells as f32 * self.cell_width / window_width;
                    let visible_cell_rows = if scroll_offset_rows > 0 {
                        (_height_cells as f32 * tex_v_height).max(0.0)
                    } else {
                        _height_cells as f32
                    };
                    let cell_h = visible_cell_rows * self.cell_height / window_height;
                    (cell_w, cell_h)
                };

                instances.push(SixelInstance {
                    position: [x, y],
                    tex_coords: [0.0, tex_v_start, 1.0, tex_v_height], // Crop from top
                    size: [width, height],
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
        for &(id, _, _, _, _, _, _) in graphics {
            if let Some(cached) = self.texture_cache.get(&id) {
                render_pass.set_bind_group(0, &cached.texture.bind_group, &[]);
                render_pass.draw(0..4, instance_idx..(instance_idx + 1));
                instance_idx += 1;
            }
        }

        Ok(())
    }

    /// Render sixel graphics for a specific pane using explicit origin coordinates.
    ///
    /// Identical to [`render`] but uses `pane_origin_x`/`pane_origin_y` for positioning
    /// instead of the global `window_padding + content_offset` values, so graphics are
    /// placed relative to the pane rather than the full window.
    ///
    /// # Arguments
    /// * `device` - WGPU device for creating buffers
    /// * `queue` - WGPU queue for writing buffer data
    /// * `render_pass` - Active render pass to render into
    /// * `graphics` - Slice of sixel graphics to render with their positions
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    /// * `pane_origin_x` - X pixel coordinate of the pane's content origin
    /// * `pane_origin_y` - Y pixel coordinate of the pane's content origin
    #[allow(clippy::too_many_arguments)]
    pub fn render_for_pane(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut RenderPass,
        graphics: &[(u64, isize, usize, usize, usize, f32, usize)],
        window_width: f32,
        window_height: f32,
        pane_origin_x: f32,
        pane_origin_y: f32,
    ) -> Result<(), RenderError> {
        if graphics.is_empty() {
            return Ok(());
        }

        // Build instance data
        let mut instances = Vec::with_capacity(graphics.len());
        for &(id, row, col, _width_cells, _height_cells, alpha, scroll_offset_rows) in graphics {
            // Check if texture exists and update LRU timestamp
            if let Some(cached) = self.texture_cache.get_mut(&id) {
                cached.last_used = Instant::now();
                let tex_info = &cached.texture;

                // Calculate screen position using the pane's content origin.
                let adjusted_row = row + scroll_offset_rows as isize;
                let x = (pane_origin_x + col as f32 * self.cell_width) / window_width;
                let y = (pane_origin_y + adjusted_row as f32 * self.cell_height) / window_height;

                // Calculate texture V offset for scrolled graphics
                let tex_v_start = if scroll_offset_rows > 0 && tex_info.height > 0 {
                    let pixels_scrolled = scroll_offset_rows as f32 * self.cell_height;
                    (pixels_scrolled / tex_info.height as f32).min(0.99)
                } else {
                    0.0
                };
                let tex_v_height = 1.0 - tex_v_start;

                // Calculate display size based on aspect ratio preservation setting
                let (width, height) = if self.preserve_aspect_ratio {
                    let visible_height_pixels = if scroll_offset_rows > 0 {
                        (tex_info.height as f32 * tex_v_height).max(1.0)
                    } else {
                        tex_info.height as f32
                    };
                    (
                        tex_info.width as f32 / window_width,
                        visible_height_pixels / window_height,
                    )
                } else {
                    let cell_w = _width_cells as f32 * self.cell_width / window_width;
                    let visible_cell_rows = if scroll_offset_rows > 0 {
                        (_height_cells as f32 * tex_v_height).max(0.0)
                    } else {
                        _height_cells as f32
                    };
                    let cell_h = visible_cell_rows * self.cell_height / window_height;
                    (cell_w, cell_h)
                };

                instances.push(SixelInstance {
                    position: [x, y],
                    tex_coords: [0.0, tex_v_start, 1.0, tex_v_height],
                    size: [width, height],
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
        for &(id, _, _, _, _, _, _) in graphics {
            if let Some(cached) = self.texture_cache.get(&id) {
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
