use super::CellRenderer;
use crate::custom_shader_renderer::textures::ChannelTexture;
use crate::error::RenderError;
use par_term_config::color_u8_to_f32;

/// Cached GPU texture for a per-pane background image
pub(crate) struct PaneBackgroundEntry {
    #[allow(dead_code)] // GPU lifetime: must outlive the TextureView created from it
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl CellRenderer {
    pub(crate) fn load_background_image(&mut self, path: &str) -> Result<(), RenderError> {
        log::info!("Loading background image from: {}", path);
        let img = image::open(path)
            .map_err(|e| {
                log::error!("Failed to open background image '{}': {}", path, e);
                RenderError::ImageLoad {
                    path: path.to_string(),
                    source: e,
                }
            })?
            .to_rgba8();
        log::info!("Background image loaded: {}x{}", img.width(), img.height());
        let (width, height) = img.dimensions();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bg image"),
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

        self.pipelines.bg_image_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bg image bind group"),
                layout: &self.pipelines.bg_image_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.buffers.bg_image_uniform_buffer.as_entire_binding(),
                    },
                ],
            }));
        self.bg_state.bg_image_texture = Some(texture);
        self.bg_state.bg_image_width = width;
        self.bg_state.bg_image_height = height;
        self.bg_state.bg_is_solid_color = false; // This is an image, not a solid color
        self.update_bg_image_uniforms(None);
        Ok(())
    }

    /// Update the background image uniform buffer.
    ///
    /// # Arguments
    /// * `window_opacity_override` - If `Some(v)`, use `v` as the window opacity instead of
    ///   `self.window_opacity`. Pass `Some(1.0)` when rendering to an intermediate texture
    ///   so that window-level opacity is applied later by the shader wrapper, avoiding any
    ///   need to temporarily mutate `self.window_opacity`.
    pub(crate) fn update_bg_image_uniforms(&mut self, window_opacity_override: Option<f32>) {
        // Shader uniform struct layout (48 bytes):
        //   image_size: vec2<f32>    @ offset 0  (8 bytes)
        //   window_size: vec2<f32>   @ offset 8  (8 bytes)
        //   mode: u32                @ offset 16 (4 bytes)
        //   opacity: f32             @ offset 20 (4 bytes)
        //   pane_offset: vec2<f32>   @ offset 24 (8 bytes) - (0,0) for global
        //   surface_size: vec2<f32>  @ offset 32 (8 bytes) - same as window_size for global
        //   darken: f32              @ offset 40 (4 bytes) - 0.0 for global
        let mut data = [0u8; 48];

        let w = self.config.width as f32;
        let h = self.config.height as f32;

        // image_size (vec2<f32>)
        data[0..4].copy_from_slice(&(self.bg_state.bg_image_width as f32).to_le_bytes());
        data[4..8].copy_from_slice(&(self.bg_state.bg_image_height as f32).to_le_bytes());

        // window_size (vec2<f32>)
        data[8..12].copy_from_slice(&w.to_le_bytes());
        data[12..16].copy_from_slice(&h.to_le_bytes());

        // mode (u32)
        data[16..20].copy_from_slice(&(self.bg_state.bg_image_mode as u32).to_le_bytes());

        // opacity (f32) - combine bg_image_opacity with effective window_opacity
        let win_opacity = window_opacity_override.unwrap_or(self.window_opacity);
        let effective_opacity = self.bg_state.bg_image_opacity * win_opacity;
        data[20..24].copy_from_slice(&effective_opacity.to_le_bytes());

        // pane_offset (vec2<f32>) - (0,0) for global background
        // bytes 24..32 are already zeros

        // surface_size (vec2<f32>) - same as window_size for global
        data[32..36].copy_from_slice(&w.to_le_bytes());
        data[36..40].copy_from_slice(&h.to_le_bytes());

        // darken (f32) - 0.0 for global background (no darkening)
        // bytes 40..44 are already zeros

        self.queue
            .write_buffer(&self.buffers.bg_image_uniform_buffer, 0, &data);
    }

    pub fn set_background_image(
        &mut self,
        path: Option<&str>,
        mode: par_term_config::BackgroundImageMode,
        opacity: f32,
    ) {
        self.bg_state.bg_image_mode = mode;
        self.bg_state.bg_image_opacity = opacity;
        if let Some(p) = path {
            log::info!("Loading background image: {}", p);
            if let Err(e) = self.load_background_image(p) {
                log::error!("Failed to load background image '{}': {}", p, e);
            }
            // Note: bg_is_solid_color is set in load_background_image
        } else {
            self.bg_state.bg_image_texture = None;
            self.pipelines.bg_image_bind_group = None;
            self.bg_state.bg_image_width = 0;
            self.bg_state.bg_image_height = 0;
            self.bg_state.bg_is_solid_color = false;
        }
        self.update_bg_image_uniforms(None);
    }

    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.bg_state.bg_image_opacity = opacity;
        self.update_bg_image_uniforms(None);
    }

    pub fn update_background_image_opacity_only(&mut self, opacity: f32) {
        self.bg_state.bg_image_opacity = opacity;
        self.update_bg_image_uniforms(None);
    }

    /// Create a ChannelTexture from the current background image for use in custom shaders.
    ///
    /// Returns None if no background image is loaded.
    /// The returned ChannelTexture shares the same underlying texture data with the
    /// cell renderer's background image - no copy is made.
    pub fn get_background_as_channel_texture(&self) -> Option<ChannelTexture> {
        let texture = self.bg_state.bg_image_texture.as_ref()?;

        // Create a new view and sampler for use by the custom shader
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        Some(ChannelTexture::from_view(
            view,
            sampler,
            self.bg_state.bg_image_width,
            self.bg_state.bg_image_height,
        ))
    }

    /// Check if a background image is currently loaded.
    pub fn has_background_image(&self) -> bool {
        self.bg_state.bg_image_texture.is_some()
    }

    /// Check if a solid color background is currently set.
    pub fn is_solid_color_background(&self) -> bool {
        self.bg_state.bg_is_solid_color
    }

    /// Get the solid background color as normalized RGB values.
    /// Returns the color even if not in solid color mode.
    pub fn solid_background_color(&self) -> [f32; 3] {
        self.bg_state.solid_bg_color
    }

    /// Get the solid background color as a wgpu::Color with window_opacity applied.
    /// Returns None if not in solid color mode.
    pub fn get_solid_color_as_clear(&self) -> Option<wgpu::Color> {
        if self.bg_state.bg_is_solid_color {
            Some(wgpu::Color {
                r: self.bg_state.solid_bg_color[0] as f64 * self.window_opacity as f64,
                g: self.bg_state.solid_bg_color[1] as f64 * self.window_opacity as f64,
                b: self.bg_state.solid_bg_color[2] as f64 * self.window_opacity as f64,
                a: self.window_opacity as f64,
            })
        } else {
            None
        }
    }

    /// Create a solid color texture for use as background.
    ///
    /// Creates a small (4x4) texture filled with the specified color.
    /// Uses Stretch mode for solid colors to fill the entire window.
    /// Transparency is controlled by window_opacity, not the texture alpha.
    pub fn create_solid_color_texture(&mut self, color: [u8; 3]) {
        let norm = color_u8_to_f32(color);
        log::info!(
            "[BACKGROUND] create_solid_color_texture: RGB({}, {}, {}) -> normalized ({:.3}, {:.3}, {:.3})",
            color[0],
            color[1],
            color[2],
            norm[0],
            norm[1],
            norm[2]
        );
        let size = 4u32; // 4x4 for proper linear filtering
        let mut pixels = Vec::with_capacity((size * size * 4) as usize);
        for _ in 0..(size * size) {
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(255); // Fully opaque - window_opacity controls transparency
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bg solid color"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
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
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.pipelines.bg_image_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bg solid color bind group"),
                layout: &self.pipelines.bg_image_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.buffers.bg_image_uniform_buffer.as_entire_binding(),
                    },
                ],
            }));

        self.bg_state.bg_image_texture = Some(texture);
        self.bg_state.bg_image_width = size;
        self.bg_state.bg_image_height = size;
        // Use Stretch mode for solid colors to fill the window
        self.bg_state.bg_image_mode = par_term_config::BackgroundImageMode::Stretch;
        // Use 1.0 as base opacity - window_opacity is applied in update_bg_image_uniforms()
        self.bg_state.bg_image_opacity = 1.0;
        // Mark this as a solid color for tracking purposes
        self.bg_state.bg_is_solid_color = true;
        self.bg_state.solid_bg_color = color_u8_to_f32(color);
        self.update_bg_image_uniforms(None);
    }

    /// Create a ChannelTexture from a solid color for shader iChannel0.
    ///
    /// Creates a small texture with the specified color that can be used
    /// as a channel texture in custom shaders. The texture is fully opaque;
    /// window_opacity controls overall transparency.
    pub fn get_solid_color_as_channel_texture(&self, color: [u8; 3]) -> ChannelTexture {
        log::info!(
            "get_solid_color_as_channel_texture: RGB({},{},{})",
            color[0],
            color[1],
            color[2]
        );
        let size = 4u32;
        let mut pixels = Vec::with_capacity((size * size * 4) as usize);
        for _ in 0..(size * size) {
            pixels.push(color[0]);
            pixels.push(color[1]);
            pixels.push(color[2]);
            pixels.push(255); // Fully opaque
        }

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("solid color channel texture"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
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
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });

        ChannelTexture::from_view_and_texture(view, sampler, size, size, texture)
    }

    /// Set background based on mode (Default, Color, or Image).
    ///
    /// This unified method handles all background types and should be used
    /// instead of calling individual methods directly.
    pub fn set_background(
        &mut self,
        mode: par_term_config::BackgroundMode,
        color: [u8; 3],
        image_path: Option<&str>,
        image_mode: par_term_config::BackgroundImageMode,
        image_opacity: f32,
        image_enabled: bool,
    ) {
        log::info!(
            "[BACKGROUND] set_background: mode={:?}, color=RGB({}, {}, {}), image_path={:?}",
            mode,
            color[0],
            color[1],
            color[2],
            image_path
        );
        match mode {
            par_term_config::BackgroundMode::Default => {
                // Clear background texture - use theme default
                self.bg_state.bg_image_texture = None;
                self.pipelines.bg_image_bind_group = None;
                self.bg_state.bg_image_width = 0;
                self.bg_state.bg_image_height = 0;
                self.bg_state.bg_is_solid_color = false;
            }
            par_term_config::BackgroundMode::Color => {
                // create_solid_color_texture sets bg_is_solid_color = true
                self.create_solid_color_texture(color);
            }
            par_term_config::BackgroundMode::Image => {
                if image_enabled {
                    // set_background_image sets bg_is_solid_color = false
                    self.set_background_image(image_path, image_mode, image_opacity);
                } else {
                    // Image disabled - clear texture
                    self.bg_state.bg_image_texture = None;
                    self.pipelines.bg_image_bind_group = None;
                    self.bg_state.bg_image_width = 0;
                    self.bg_state.bg_image_height = 0;
                    self.bg_state.bg_is_solid_color = false;
                }
            }
        }
    }

    /// Load a per-pane background image into the texture cache.
    /// Returns Ok(true) if the image was newly loaded, Ok(false) if already cached.
    pub(crate) fn load_pane_background(&mut self, path: &str) -> Result<bool, RenderError> {
        if self.bg_state.pane_bg_cache.contains_key(path) {
            return Ok(false);
        }

        // Expand tilde in path (e.g., ~/images/bg.png -> /home/user/images/bg.png)
        let expanded = if let Some(rest) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(rest).to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };

        log::info!("Loading per-pane background image: {}", expanded);
        let img = image::open(&expanded)
            .map_err(|e| {
                log::error!("Failed to open pane background image '{}': {}", path, e);
                RenderError::ImageLoad {
                    path: expanded.clone(),
                    source: e,
                }
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

        self.bg_state.pane_bg_cache.insert(
            path.to_string(),
            super::background::PaneBackgroundEntry {
                texture,
                view,
                sampler,
                width,
                height,
            },
        );

        Ok(true)
    }

    /// Create a bind group and uniform buffer for a per-pane background render.
    /// The uniform buffer provides pane dimensions, position, and surface size so the
    /// background_image.wgsl shader computes texture coords and NDC positions relative to the pane.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_pane_bg_bind_group(
        &self,
        entry: &super::background::PaneBackgroundEntry,
        pane_x: f32,
        pane_y: f32,
        pane_width: f32,
        pane_height: f32,
        mode: par_term_config::BackgroundImageMode,
        opacity: f32,
        darken: f32,
    ) -> (wgpu::BindGroup, wgpu::Buffer) {
        // Shader uniform struct layout (48 bytes):
        //   image_size: vec2<f32>    @ offset 0  (8 bytes)
        //   window_size: vec2<f32>   @ offset 8  (8 bytes) - pane dimensions
        //   mode: u32                @ offset 16 (4 bytes)
        //   opacity: f32             @ offset 20 (4 bytes)
        //   pane_offset: vec2<f32>   @ offset 24 (8 bytes) - pane position in window
        //   surface_size: vec2<f32>  @ offset 32 (8 bytes) - window dimensions
        //   darken: f32              @ offset 40 (4 bytes)
        let mut data = [0u8; 48];
        // image_size (vec2<f32>)
        data[0..4].copy_from_slice(&(entry.width as f32).to_le_bytes());
        data[4..8].copy_from_slice(&(entry.height as f32).to_le_bytes());
        // window_size (pane dimensions for UV calculation)
        data[8..12].copy_from_slice(&pane_width.to_le_bytes());
        data[12..16].copy_from_slice(&pane_height.to_le_bytes());
        // mode (u32)
        data[16..20].copy_from_slice(&(mode as u32).to_le_bytes());
        // opacity (combine with window_opacity)
        let effective_opacity = opacity * self.window_opacity;
        data[20..24].copy_from_slice(&effective_opacity.to_le_bytes());
        // pane_offset (vec2<f32>) - pane position within the window
        data[24..28].copy_from_slice(&pane_x.to_le_bytes());
        data[28..32].copy_from_slice(&pane_y.to_le_bytes());
        // surface_size (vec2<f32>) - full window dimensions
        let surface_w = self.config.width as f32;
        let surface_h = self.config.height as f32;
        data[32..36].copy_from_slice(&surface_w.to_le_bytes());
        data[36..40].copy_from_slice(&surface_h.to_le_bytes());
        // darken (f32)
        data[40..44].copy_from_slice(&darken.to_le_bytes());

        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pane bg uniform buffer"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&uniform_buffer, 0, &data);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pane bg bind group"),
            layout: &self.pipelines.bg_image_bind_group_layout,
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
}
