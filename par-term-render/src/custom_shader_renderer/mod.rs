//! Custom shader renderer for post-processing effects
//!
//! Supports Ghostty/Shadertoy-style GLSL shaders with the following uniforms:
//! - `iTime`: Time in seconds (animated or fixed at 0.0)
//! - `iResolution`: Viewport resolution (width, height, 1.0)
//! - `iChannel0-3`: User texture channels (Shadertoy compatible)
//! - `iChannel4`: Terminal content texture
//! - `iTimeKeyPress`: Time when last key was pressed (same timebase as iTime)
//!
//! Ghostty-compatible cursor uniforms (v1.2.0+):
//! - `iCurrentCursor`: Current cursor position (xy) and size (zw) in pixels
//! - `iPreviousCursor`: Previous cursor position and size
//! - `iCurrentCursorColor`: Current cursor RGBA color (with opacity baked in)
//! - `iPreviousCursorColor`: Previous cursor RGBA color
//! - `iTimeCursorChange`: Time when cursor last moved (same timebase as iTime)

use anyhow::{Context, Result};
use par_term_emu_core_rust::cursor::CursorStyle;
use std::path::Path;
use std::time::Instant;
use wgpu::*;

mod cubemap;
mod cursor;
pub mod pipeline;
pub mod textures;
pub mod transpiler;
pub mod types;

use cubemap::CubemapTexture;
use pipeline::{create_bind_group, create_bind_group_layout, create_render_pipeline};
use textures::{ChannelTexture, load_channel_textures};
use transpiler::{transpile_glsl_to_wgsl, transpile_glsl_to_wgsl_source};
use types::CustomShaderUniforms;

/// Custom shader renderer that applies post-processing effects
pub struct CustomShaderRenderer {
    /// The render pipeline for the custom shader
    pub(crate) pipeline: RenderPipeline,
    /// Bind group for shader uniforms and textures
    pub(crate) bind_group: BindGroup,
    /// Uniform buffer for shader parameters
    pub(crate) uniform_buffer: Buffer,
    /// Intermediate texture to render terminal content into
    pub(crate) intermediate_texture: Texture,
    /// View of the intermediate texture
    pub(crate) intermediate_texture_view: TextureView,
    /// Start time for animation
    pub(crate) start_time: Instant,
    /// Whether animation is enabled
    pub(crate) animation_enabled: bool,
    /// Animation speed multiplier
    pub(crate) animation_speed: f32,
    /// Current texture dimensions
    pub(crate) texture_width: u32,
    pub(crate) texture_height: u32,
    /// Surface format for compatibility
    pub(crate) surface_format: TextureFormat,
    /// Bind group layout for recreating bind groups on resize
    pub(crate) bind_group_layout: BindGroupLayout,
    /// Sampler for the intermediate texture
    pub(crate) sampler: Sampler,
    /// Display scale factor for DPI scaling (e.g., 2.0 on Retina)
    pub(crate) scale_factor: f32,
    /// Window opacity for transparency
    pub(crate) window_opacity: f32,
    /// When true, text is always rendered at full opacity (overrides text_opacity)
    pub(crate) keep_text_opaque: bool,
    /// Full content mode - shader receives and can manipulate full terminal content
    pub(crate) full_content_mode: bool,
    /// Brightness multiplier for shader output (0.05-1.0)
    pub(crate) brightness: f32,
    /// Frame counter for iFrame uniform
    pub(crate) frame_count: u32,
    /// Last frame time for calculating time delta
    pub(crate) last_frame_time: Instant,
    /// Current mouse position in pixels (xy)
    pub(crate) mouse_position: [f32; 2],
    /// Last click position in pixels (zw)
    pub(crate) mouse_click_position: [f32; 2],
    /// Whether mouse button is currently pressed (affects sign of zw)
    pub(crate) mouse_button_down: bool,
    /// Frame rate tracking: time accumulator for averaging
    pub(crate) frame_time_accumulator: f32,
    /// Frame rate tracking: frames in current second
    pub(crate) frames_in_second: u32,
    /// Current smoothed frame rate
    pub(crate) current_frame_rate: f32,

    // ============ Cursor tracking (Ghostty-compatible) ============
    /// Current cursor position in cell coordinates (col, row)
    pub(crate) current_cursor_pos: (usize, usize),
    /// Previous cursor position in cell coordinates
    pub(crate) previous_cursor_pos: (usize, usize),
    /// Current cursor RGBA color
    pub(crate) current_cursor_color: [f32; 4],
    /// Previous cursor RGBA color
    pub(crate) previous_cursor_color: [f32; 4],
    /// Current cursor opacity (0.0 = invisible, 1.0 = fully visible)
    pub(crate) current_cursor_opacity: f32,
    /// Previous cursor opacity
    pub(crate) previous_cursor_opacity: f32,
    /// Time when cursor position last changed (same timebase as iTime)
    pub(crate) cursor_change_time: f32,
    /// Current cursor style (for size calculation)
    pub(crate) current_cursor_style: CursorStyle,
    /// Previous cursor style
    pub(crate) previous_cursor_style: CursorStyle,
    /// Cell width in pixels (for cursor position calculation)
    pub(crate) cursor_cell_width: f32,
    /// Cell height in pixels (for cursor position calculation)
    pub(crate) cursor_cell_height: f32,
    /// Window padding in pixels (for cursor position calculation)
    pub(crate) cursor_window_padding: f32,
    /// Vertical content offset in pixels (e.g., tab bar height)
    pub(crate) cursor_content_offset_y: f32,
    /// Horizontal content offset in pixels (e.g., tab bar on left)
    pub(crate) cursor_content_offset_x: f32,

    // ============ Cursor shader configuration ============
    /// User-configured cursor color for shader effects [R, G, B, A]
    pub(crate) cursor_shader_color: [f32; 4],
    /// Cursor trail duration in seconds
    pub(crate) cursor_trail_duration: f32,
    /// Cursor glow radius in pixels
    pub(crate) cursor_glow_radius: f32,
    /// Cursor glow intensity (0.0-1.0)
    pub(crate) cursor_glow_intensity: f32,

    // ============ Key press tracking ============
    /// Time when a key was last pressed (same timebase as iTime)
    pub(crate) key_press_time: f32,

    // ============ Channel textures (iChannel0-3) ============
    /// Texture channels 0-3 (placeholders or loaded textures, Shadertoy compatible)
    pub(crate) channel_textures: [ChannelTexture; 4],

    // ============ Cubemap texture (iCubemap) ============
    /// Cubemap texture for environment mapping (placeholder or loaded)
    pub(crate) cubemap: CubemapTexture,

    // ============ Background image as iChannel0 ============
    /// When true, use the background image texture as iChannel0 instead of the configured texture
    pub(crate) use_background_as_channel0: bool,
    /// Background texture to use as iChannel0 when use_background_as_channel0 is true
    /// This is a reference texture (view + sampler + dimensions) from the cell renderer
    pub(crate) background_channel_texture: Option<ChannelTexture>,

    // ============ Solid background color ============
    /// Solid background color [R, G, B, A] for shader compositing.
    /// When A > 0, the shader uses this color as background instead of shader output.
    /// RGB values are NOT premultiplied.
    pub(crate) background_color: [f32; 4],

    // ============ Progress bar state ============
    /// Progress bar data [state, percent, isActive, activeCount]
    pub(crate) progress_data: [f32; 4],

    // ============ Content inset for panels ============
    /// Right content inset in pixels (e.g., AI Inspector panel).
    /// The shader renders to a viewport offset by this amount from the left.
    pub(crate) content_inset_right: f32,
}

impl CustomShaderRenderer {
    /// Create a new custom shader renderer from a GLSL shader file
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        shader_path: &Path,
        width: u32,
        height: u32,
        animation_enabled: bool,
        animation_speed: f32,
        window_opacity: f32,
        full_content_mode: bool,
        channel_paths: &[Option<std::path::PathBuf>; 4],
        cubemap_path: Option<&Path>,
    ) -> Result<Self> {
        // Load the GLSL shader
        let glsl_source = std::fs::read_to_string(shader_path)
            .with_context(|| format!("Failed to read shader file: {}", shader_path.display()))?;

        // Transpile GLSL to WGSL
        let wgsl_source = transpile_glsl_to_wgsl(&glsl_source, shader_path)?;

        log::info!(
            "Loaded custom shader from {} ({} bytes GLSL -> {} bytes WGSL)",
            shader_path.display(),
            glsl_source.len(),
            wgsl_source.len()
        );
        log::debug!("Generated WGSL:\n{}", wgsl_source);

        // DEBUG: Write generated WGSL to file for inspection
        let shader_name = shader_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let debug_filename = format!("/tmp/par_term_{}_shader.wgsl", shader_name);
        if let Err(e) = std::fs::write(&debug_filename, &wgsl_source) {
            log::warn!("Failed to write debug shader: {}", e);
        } else {
            log::info!("Wrote debug shader to {}", debug_filename);
        }

        // Pre-validate WGSL
        let module = naga::front::wgsl::parse_str(&wgsl_source)
            .context("Custom shader WGSL parse failed")?;
        let _info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .context("Custom shader WGSL validation failed")?;

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Custom Shader Module"),
            source: ShaderSource::Wgsl(wgsl_source.clone().into()),
        });

        // Create intermediate texture for terminal content
        let (intermediate_texture, intermediate_texture_view) =
            Self::create_intermediate_texture(device, surface_format, width, height);

        // Create sampler for the intermediate texture (terminal content)
        // Use Nearest filtering to keep text crisp and pixel-perfect
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Custom Shader Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // Load channel textures (iChannel0-3)
        let channel_textures = load_channel_textures(device, queue, channel_paths);

        // Load cubemap texture (iCubemap)
        let cubemap = match cubemap_path {
            Some(path) => match CubemapTexture::from_prefix(device, queue, path) {
                Ok(cm) => cm,
                Err(e) => {
                    log::error!("Failed to load cubemap '{}': {}", path.display(), e);
                    CubemapTexture::placeholder(device, queue)
                }
            },
            None => CubemapTexture::placeholder(device, queue),
        };

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Custom Shader Uniforms"),
            size: std::mem::size_of::<CustomShaderUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout and bind group
        let bind_group_layout = create_bind_group_layout(device);
        let bind_group = create_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &intermediate_texture_view,
            &sampler,
            &channel_textures,
            &cubemap,
        );

        // Create render pipeline
        let pipeline = create_render_pipeline(
            device,
            &shader_module,
            &bind_group_layout,
            surface_format,
            Some("Custom Shader Pipeline"),
        );

        let now = Instant::now();
        Ok(Self {
            pipeline,
            bind_group,
            uniform_buffer,
            intermediate_texture,
            intermediate_texture_view,
            start_time: now,
            animation_enabled,
            animation_speed,
            texture_width: width,
            texture_height: height,
            surface_format,
            bind_group_layout,
            sampler,
            window_opacity,
            keep_text_opaque: false,
            scale_factor: 1.0,
            full_content_mode,
            brightness: 1.0,
            frame_count: 0,
            last_frame_time: now,
            mouse_position: [0.0, 0.0],
            mouse_click_position: [0.0, 0.0],
            mouse_button_down: false,
            frame_time_accumulator: 0.0,
            frames_in_second: 0,
            current_frame_rate: 60.0,
            current_cursor_pos: (0, 0),
            previous_cursor_pos: (0, 0),
            current_cursor_color: [1.0, 1.0, 1.0, 1.0],
            previous_cursor_color: [1.0, 1.0, 1.0, 1.0],
            current_cursor_opacity: 1.0,
            previous_cursor_opacity: 1.0,
            cursor_change_time: 0.0,
            current_cursor_style: CursorStyle::SteadyBlock,
            previous_cursor_style: CursorStyle::SteadyBlock,
            cursor_cell_width: 10.0,
            cursor_cell_height: 20.0,
            cursor_window_padding: 0.0,
            cursor_content_offset_y: 0.0,
            cursor_content_offset_x: 0.0,
            cursor_shader_color: [1.0, 1.0, 1.0, 1.0],
            cursor_trail_duration: 0.5,
            cursor_glow_radius: 80.0,
            cursor_glow_intensity: 0.3,
            key_press_time: 0.0,
            channel_textures,
            cubemap,
            use_background_as_channel0: false,
            background_channel_texture: None,
            background_color: [0.0, 0.0, 0.0, 0.0], // No solid background by default
            progress_data: [0.0, 0.0, 0.0, 0.0],
            content_inset_right: 0.0,
        })
    }

    /// Create the intermediate texture for rendering terminal content
    fn create_intermediate_texture(
        device: &Device,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Custom Shader Intermediate Texture"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// Get a view of the intermediate texture for rendering terminal content into
    pub fn intermediate_texture_view(&self) -> &TextureView {
        &self.intermediate_texture_view
    }

    /// Clear the intermediate texture (e.g., when switching to split pane mode)
    ///
    /// This prevents old single-pane content from showing through the shader.
    pub fn clear_intermediate_texture(&self, device: &Device, queue: &Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Clear Intermediate Texture Encoder"),
        });

        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Intermediate Texture Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.intermediate_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Resize the intermediate texture when window size changes
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == self.texture_width && height == self.texture_height {
            return;
        }

        self.texture_width = width;
        self.texture_height = height;

        // Recreate intermediate texture
        let (texture, view) =
            Self::create_intermediate_texture(device, self.surface_format, width, height);
        self.intermediate_texture = texture;
        self.intermediate_texture_view = view;

        // Recreate bind group with new texture view (handles background as channel0 if enabled)
        self.recreate_bind_group(device);
    }

    /// Render the custom shader effect to the output texture
    ///
    /// # Arguments
    /// * `device` - The GPU device
    /// * `queue` - The command queue
    /// * `output_view` - The texture view to render to
    /// * `apply_opacity` - Whether to apply window opacity. Set to `false` when rendering
    ///   to an intermediate texture that will be processed by another shader (to avoid
    ///   double-applying opacity).
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        output_view: &TextureView,
        apply_opacity: bool,
    ) -> Result<()> {
        self.render_with_clear_color(
            device,
            queue,
            output_view,
            apply_opacity,
            Color::TRANSPARENT,
        )
    }

    /// Render the custom shader with a specified clear color.
    /// Use this for solid background colors where the clear color provides the background.
    pub fn render_with_clear_color(
        &mut self,
        device: &Device,
        queue: &Queue,
        output_view: &TextureView,
        apply_opacity: bool,
        clear_color: Color,
    ) -> Result<()> {
        let now = Instant::now();

        // Calculate time value
        let time = if self.animation_enabled {
            self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0)
        } else {
            0.0
        };

        // Calculate time delta
        let time_delta = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Update frame rate calculation
        self.frame_time_accumulator += time_delta;
        self.frames_in_second += 1;
        if self.frame_time_accumulator >= 1.0 {
            self.current_frame_rate = self.frames_in_second as f32 / self.frame_time_accumulator;
            self.frame_time_accumulator = 0.0;
            self.frames_in_second = 0;
        }

        self.frame_count = self.frame_count.wrapping_add(1);

        // Calculate uniforms
        let uniforms = self.build_uniforms(time, time_delta, apply_opacity);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Create command encoder and render
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Custom Shader Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Custom Shader Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(clear_color),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Note: We intentionally do NOT set a viewport here to exclude the panel area.
            // The viewport approach doesn't work because fragCoord in WGSL is relative to
            // the render target, not the viewport, causing UV coordinate mismatches.
            // The opaque panel (PANEL_BG with alpha 255) covers any shader output under it.

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Build the uniform buffer data
    fn build_uniforms(
        &self,
        time: f32,
        time_delta: f32,
        apply_opacity: bool,
    ) -> CustomShaderUniforms {
        // Calculate iMouse uniform
        let height = self.texture_height as f32;
        let mouse_y_flipped = height - self.mouse_position[1];
        let click_y_flipped = height - self.mouse_click_position[1];

        let mouse = if self.mouse_button_down {
            [
                self.mouse_position[0],
                mouse_y_flipped,
                self.mouse_click_position[0],
                click_y_flipped,
            ]
        } else {
            [
                self.mouse_position[0],
                mouse_y_flipped,
                -self.mouse_click_position[0].abs(),
                -click_y_flipped.abs(),
            ]
        };

        // Calculate iDate uniform
        let date = Self::calculate_date();

        // Calculate cursor pixel positions
        let (curr_x, curr_y) =
            self.cursor_to_pixels(self.current_cursor_pos.0, self.current_cursor_pos.1);
        let (prev_x, prev_y) =
            self.cursor_to_pixels(self.previous_cursor_pos.0, self.previous_cursor_pos.1);

        // When rendering to intermediate texture (for further shader processing),
        // use 0.0 to signal "chain mode" to the shader. This tells the shader to:
        // - Use full background color for RGB (not premultiplied by opacity)
        // - Output terminal-only alpha (so next shader can detect transparent areas)
        // The final shader in the chain will apply actual window opacity.
        let effective_opacity = if apply_opacity {
            self.window_opacity
        } else {
            0.0 // Chain mode: shader detects this and preserves transparency info
        };

        // Resolution stays at full texture size for correct UV sampling
        // The viewport (set in render) limits where output appears
        CustomShaderUniforms {
            resolution: [self.texture_width as f32, self.texture_height as f32],
            time,
            time_delta,
            mouse,
            date,
            opacity: effective_opacity,
            // When keep_text_opaque is true, text stays at full opacity (1.0)
            // When false, text uses the same opacity as the window background
            text_opacity: if self.keep_text_opaque || !apply_opacity {
                1.0
            } else {
                self.window_opacity
            },
            full_content_mode: if self.full_content_mode { 1.0 } else { 0.0 },
            frame: self.frame_count as f32,
            frame_rate: self.current_frame_rate,
            resolution_z: 1.0,
            brightness: self.brightness,
            key_press_time: self.key_press_time,
            current_cursor: [
                curr_x,
                curr_y,
                self.cursor_width_for_style(self.current_cursor_style, self.scale_factor),
                self.cursor_height_for_style(self.current_cursor_style, self.scale_factor),
            ],
            previous_cursor: [
                prev_x,
                prev_y,
                self.cursor_width_for_style(self.previous_cursor_style, self.scale_factor),
                self.cursor_height_for_style(self.previous_cursor_style, self.scale_factor),
            ],
            current_cursor_color: [
                self.current_cursor_color[0],
                self.current_cursor_color[1],
                self.current_cursor_color[2],
                self.current_cursor_color[3] * self.current_cursor_opacity,
            ],
            previous_cursor_color: [
                self.previous_cursor_color[0],
                self.previous_cursor_color[1],
                self.previous_cursor_color[2],
                self.previous_cursor_color[3] * self.previous_cursor_opacity,
            ],
            cursor_change_time: self.cursor_change_time,
            cursor_trail_duration: self.cursor_trail_duration,
            cursor_glow_radius: self.cursor_glow_radius,
            cursor_glow_intensity: self.cursor_glow_intensity,
            cursor_shader_color: self.cursor_shader_color,
            channel0_resolution: self.effective_channel0_resolution(),
            channel1_resolution: self.channel_textures[1].resolution(),
            channel2_resolution: self.channel_textures[2].resolution(),
            channel3_resolution: self.channel_textures[3].resolution(),
            channel4_resolution: [
                self.texture_width as f32,
                self.texture_height as f32,
                1.0,
                0.0,
            ],
            cubemap_resolution: self.cubemap.resolution(),
            background_color: self.background_color,
            progress: self.progress_data,
        }
    }

    /// Calculate the iDate uniform value
    fn calculate_date() -> [f32; 4] {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_sys = SystemTime::now();
        let since_epoch = now_sys.duration_since(UNIX_EPOCH).unwrap_or_default();
        let secs = since_epoch.as_secs();

        let days_since_epoch = secs / 86400;
        let secs_today = (secs % 86400) as f32;

        let mut year = 1970i32;
        let mut remaining_days = days_since_epoch as i32;

        loop {
            let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                366
            } else {
                365
            };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days_in_months: [i32; 12] = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 0i32;
        for (i, &days) in days_in_months.iter().enumerate() {
            if remaining_days < days {
                month = i as i32;
                break;
            }
            remaining_days -= days;
        }

        let day = remaining_days + 1;
        [year as f32, month as f32, day as f32, secs_today]
    }

    /// Check if animation is enabled
    pub fn animation_enabled(&self) -> bool {
        self.animation_enabled
    }

    /// Set animation enabled state
    pub fn set_animation_enabled(&mut self, enabled: bool) {
        self.animation_enabled = enabled;
        if enabled {
            self.start_time = Instant::now();
        }
    }

    /// Update animation speed multiplier
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.animation_speed = speed.max(0.0);
    }

    /// Update window opacity
    pub fn set_opacity(&mut self, opacity: f32) {
        self.window_opacity = opacity.clamp(0.0, 1.0);
    }

    /// Update shader brightness multiplier
    pub fn set_brightness(&mut self, brightness: f32) {
        self.brightness = brightness.clamp(0.05, 1.0);
    }

    /// Update full content mode
    pub fn set_full_content_mode(&mut self, enabled: bool) {
        self.full_content_mode = enabled;
    }

    /// Check if full content mode is enabled
    pub fn full_content_mode(&self) -> bool {
        self.full_content_mode
    }

    /// Set whether text should always be rendered at full opacity
    /// When true, overrides text_opacity to 1.0
    pub fn set_keep_text_opaque(&mut self, keep_opaque: bool) {
        self.keep_text_opaque = keep_opaque;
    }

    /// Update mouse position in pixel coordinates
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_position = [x, y];
    }

    /// Update mouse button state and click position
    pub fn set_mouse_button(&mut self, pressed: bool, x: f32, y: f32) {
        self.mouse_button_down = pressed;
        if pressed {
            self.mouse_click_position = [x, y];
        }
    }

    /// Update key press time for shader effects
    ///
    /// Call this when a key is pressed to enable key-press-based shader effects
    /// like screen pulses or typing animations.
    pub fn update_key_press(&mut self) {
        self.key_press_time = if self.animation_enabled {
            self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0)
        } else {
            0.0
        };
        log::trace!("Key pressed at shader time={:.3}", self.key_press_time);
    }

    /// Update a channel texture at runtime
    pub fn update_channel_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        channel: u8,
        path: Option<&std::path::Path>,
    ) -> Result<()> {
        if !(1..=4).contains(&channel) {
            anyhow::bail!("Invalid channel index: {} (must be 1-4)", channel);
        }

        let index = (channel - 1) as usize;

        let new_texture = match path {
            Some(p) => ChannelTexture::from_file(device, queue, p)?,
            None => ChannelTexture::placeholder(device, queue),
        };

        self.channel_textures[index] = new_texture;

        // Use recreate_bind_group to properly handle use_background_as_channel0 logic
        self.recreate_bind_group(device);

        log::info!(
            "Updated iChannel{} texture: {}",
            channel,
            path.map(|p| p.display().to_string())
                .unwrap_or_else(|| "placeholder".to_string())
        );

        Ok(())
    }

    /// Update the cubemap texture at runtime
    pub fn update_cubemap(
        &mut self,
        device: &Device,
        queue: &Queue,
        path: Option<&std::path::Path>,
    ) -> Result<()> {
        let new_cubemap = match path {
            Some(p) => CubemapTexture::from_prefix(device, queue, p)?,
            None => CubemapTexture::placeholder(device, queue),
        };

        self.cubemap = new_cubemap;

        // Use recreate_bind_group to properly handle use_background_as_channel0 logic
        self.recreate_bind_group(device);

        log::info!(
            "Updated cubemap texture: {}",
            path.map(|p| p.display().to_string())
                .unwrap_or_else(|| "placeholder".to_string())
        );

        Ok(())
    }

    /// Set whether to use the background image as iChannel0.
    ///
    /// When enabled and a background texture is set, the background image will be
    /// used as iChannel0 instead of the configured channel0 texture file.
    ///
    /// Note: This only updates the flag. Use `update_use_background_as_channel0`
    /// if you also need to recreate the bind group.
    pub fn set_use_background_as_channel0(&mut self, use_background: bool) {
        if self.use_background_as_channel0 != use_background {
            self.use_background_as_channel0 = use_background;
            log::info!("use_background_as_channel0 set to {}", use_background);
        }
    }

    /// Check if using background image as iChannel0.
    pub fn use_background_as_channel0(&self) -> bool {
        self.use_background_as_channel0
    }

    /// Set the background texture to use as iChannel0 when enabled.
    ///
    /// Call this whenever the background image changes to update the shader's
    /// channel0 binding. The device parameter is needed to recreate the bind group.
    ///
    /// When use_background_as_channel0 is enabled, the background texture takes
    /// priority over any configured channel0 texture.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `texture` - The background texture (view, sampler, dimensions), or None to clear
    pub fn set_background_texture(&mut self, device: &Device, texture: Option<ChannelTexture>) {
        self.background_channel_texture = texture;

        // Recreate bind group if we're using background as channel0
        // The background texture takes priority over configured channel0 when enabled
        if self.use_background_as_channel0 {
            self.recreate_bind_group(device);
        }
    }

    /// Set the solid background color for shader compositing.
    ///
    /// When set (alpha > 0), the shader uses this color as background instead of shader output.
    /// This allows solid background colors to show through properly with window transparency.
    ///
    /// # Arguments
    /// * `color` - RGB color values [R, G, B] (0.0-1.0, NOT premultiplied)
    /// * `active` - Whether solid color mode is active (sets alpha to 1.0 or 0.0)
    pub fn set_background_color(&mut self, color: [f32; 3], active: bool) {
        self.background_color = [color[0], color[1], color[2], if active { 1.0 } else { 0.0 }];
    }

    /// Update progress bar state for shader effects.
    ///
    /// # Arguments
    /// * `state` - Progress state (0=hidden, 1=normal, 2=error, 3=indeterminate, 4=warning)
    /// * `percent` - Progress percentage as 0.0-1.0
    /// * `is_active` - 1.0 if any progress bar is active, 0.0 otherwise
    /// * `active_count` - Total count of active bars (simple + named)
    pub fn update_progress(&mut self, state: f32, percent: f32, is_active: f32, active_count: f32) {
        self.progress_data = [state, percent, is_active, active_count];
    }

    /// Check if channel0 has a real configured texture (not just a 1x1 placeholder).
    fn channel0_has_real_texture(&self) -> bool {
        let ch0 = &self.channel_textures[0];
        // Placeholder textures are 1x1
        ch0.width > 1 || ch0.height > 1
    }

    /// Get the effective channel0 resolution for the iChannelResolution uniform.
    ///
    /// This follows the same priority as texture selection:
    /// 1. If use_background_as_channel0 is enabled and background exists, use its resolution
    /// 2. Otherwise use channel0 texture resolution (whether configured or placeholder)
    fn effective_channel0_resolution(&self) -> [f32; 4] {
        if self.use_background_as_channel0 {
            self.background_channel_texture
                .as_ref()
                .map(|t| t.resolution())
                .unwrap_or_else(|| self.channel_textures[0].resolution())
        } else {
            self.channel_textures[0].resolution()
        }
    }

    /// Recreate the bind group, using background texture for channel0 if enabled.
    ///
    /// Priority for iChannel0:
    /// 1. If use_background_as_channel0 is enabled and background exists, use background
    /// 2. If channel0 has a configured texture (not placeholder), use it
    /// 3. Otherwise use the placeholder
    ///
    /// This is called when:
    /// - The background texture changes (and use_background_as_channel0 is true)
    /// - use_background_as_channel0 flag changes
    /// - The window resizes (intermediate texture changes)
    fn recreate_bind_group(&mut self, device: &Device) {
        // Priority: use_background_as_channel0 (explicit override) > configured channel0 > placeholder
        let channel0_texture = if self.use_background_as_channel0 {
            // User explicitly wants background image as channel0
            self.background_channel_texture
                .as_ref()
                .unwrap_or(&self.channel_textures[0])
        } else if self.channel0_has_real_texture() {
            // Channel0 has a real texture configured
            &self.channel_textures[0]
        } else {
            // Use the placeholder
            &self.channel_textures[0]
        };

        // Create a temporary array with the potentially swapped channel0
        let effective_channels = [
            channel0_texture,
            &self.channel_textures[1],
            &self.channel_textures[2],
            &self.channel_textures[3],
        ];

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Custom Shader Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                // iChannel0 (background or configured texture)
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[0].view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[0].sampler),
                },
                // iChannel1
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[1].view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[1].sampler),
                },
                // iChannel2
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[2].view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[2].sampler),
                },
                // iChannel3
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[3].view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[3].sampler),
                },
                // iChannel4 (terminal content)
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&self.intermediate_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                // iCubemap
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&self.cubemap.view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::Sampler(&self.cubemap.sampler),
                },
            ],
        });
    }

    /// Update the use_background_as_channel0 setting and recreate bind group if needed.
    ///
    /// Call this when the setting changes in the UI or config.
    pub fn update_use_background_as_channel0(&mut self, device: &Device, use_background: bool) {
        if self.use_background_as_channel0 != use_background {
            self.use_background_as_channel0 = use_background;
            self.recreate_bind_group(device);
            log::info!("use_background_as_channel0 toggled to {}", use_background);
        }
    }

    /// Reload the shader from a source string
    pub fn reload_from_source(&mut self, device: &Device, source: &str, name: &str) -> Result<()> {
        let wgsl_source = transpile_glsl_to_wgsl_source(source, name)?;

        log::info!(
            "Reloading custom shader from source ({} bytes GLSL -> {} bytes WGSL)",
            source.len(),
            wgsl_source.len()
        );
        log::debug!("Generated WGSL:\n{}", wgsl_source);

        // Pre-validate WGSL
        let module = naga::front::wgsl::parse_str(&wgsl_source)
            .context("Custom shader WGSL parse failed")?;
        let _info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .context("Custom shader WGSL validation failed")?;

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Custom Shader Module (reloaded)"),
            source: ShaderSource::Wgsl(wgsl_source.into()),
        });

        self.pipeline = create_render_pipeline(
            device,
            &shader_module,
            &self.bind_group_layout,
            self.surface_format,
            Some("Custom Shader Pipeline (reloaded)"),
        );

        self.start_time = Instant::now();

        log::info!("Custom shader reloaded successfully from source");
        Ok(())
    }

    /// Set the right content inset (e.g., AI Inspector panel).
    ///
    /// When non-zero, the shader will render to a viewport that excludes
    /// the right inset area, ensuring effects don't appear under the panel.
    pub fn set_content_inset_right(&mut self, inset: f32) {
        self.content_inset_right = inset;
    }
}
