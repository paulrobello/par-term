//! Custom shader renderer for post-processing effects
//!
//! Supports Ghostty/Shadertoy-style GLSL shaders with the following uniforms:
//! - `iTime`: Time in seconds (animated or fixed at 0.0)
//! - `iResolution`: Viewport resolution (width, height, 1.0)
//! - `iChannel0`: Terminal content texture
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

pub mod transpiler;
pub mod types;

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
    /// Window opacity for transparency
    pub(crate) window_opacity: f32,
    /// Text opacity (separate from window opacity)
    pub(crate) text_opacity: f32,
    /// Full content mode - shader receives and can manipulate full terminal content
    pub(crate) full_content_mode: bool,
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

    // ============ Cursor shader configuration ============
    /// User-configured cursor color for shader effects [R, G, B, A]
    pub(crate) cursor_shader_color: [f32; 4],
    /// Cursor trail duration in seconds
    pub(crate) cursor_trail_duration: f32,
    /// Cursor glow radius in pixels
    pub(crate) cursor_glow_radius: f32,
    /// Cursor glow intensity (0.0-1.0)
    pub(crate) cursor_glow_intensity: f32,
}

impl CustomShaderRenderer {
    /// Create a new custom shader renderer from a GLSL shader file
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue
    /// * `surface_format` - The surface texture format
    /// * `shader_path` - Path to the GLSL shader file
    /// * `width` - Initial viewport width
    /// * `height` - Initial viewport height
    /// * `animation_enabled` - Whether to animate iTime
    /// * `animation_speed` - Animation speed multiplier
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &Device,
        _queue: &Queue,
        surface_format: TextureFormat,
        shader_path: &Path,
        width: u32,
        height: u32,
        animation_enabled: bool,
        animation_speed: f32,
        window_opacity: f32,
        text_opacity: f32,
        full_content_mode: bool,
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

        // Create the shader module
        // Pre-validate WGSL to surface errors gracefully
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

        // Create sampler for the intermediate texture
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Custom Shader Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Custom Shader Uniforms"),
            size: std::mem::size_of::<CustomShaderUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Custom Shader Bind Group Layout"),
            entries: &[
                // Uniform buffer (binding 0)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // iChannel0 texture (binding 1)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler (binding 2)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Custom Shader Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&intermediate_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Custom Shader Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Custom Shader Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_format,
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
        });

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
            text_opacity,
            full_content_mode,
            frame_count: 0,
            last_frame_time: now,
            mouse_position: [0.0, 0.0],
            mouse_click_position: [0.0, 0.0],
            mouse_button_down: false,
            frame_time_accumulator: 0.0,
            frames_in_second: 0,
            current_frame_rate: 60.0, // Start with reasonable default
            // Cursor tracking (Ghostty-compatible)
            current_cursor_pos: (0, 0),
            previous_cursor_pos: (0, 0),
            current_cursor_color: [1.0, 1.0, 1.0, 1.0], // White default
            previous_cursor_color: [1.0, 1.0, 1.0, 1.0],
            current_cursor_opacity: 1.0,
            previous_cursor_opacity: 1.0,
            cursor_change_time: 0.0,
            current_cursor_style: CursorStyle::SteadyBlock,
            previous_cursor_style: CursorStyle::SteadyBlock,
            cursor_cell_width: 10.0, // Will be updated from renderer
            cursor_cell_height: 20.0,
            cursor_window_padding: 0.0,
            // Cursor shader configuration (defaults match config.rs)
            cursor_shader_color: [1.0, 1.0, 1.0, 1.0], // White
            cursor_trail_duration: 0.5,
            cursor_glow_radius: 80.0,
            cursor_glow_intensity: 0.3,
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

        // Recreate bind group with new texture view
        self.bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Custom Shader Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.intermediate_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });
    }

    /// Render the custom shader effect to the output texture
    ///
    /// This should be called after the terminal content has been rendered to the
    /// intermediate texture obtained via `intermediate_texture_view()`.
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        output_view: &TextureView,
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

        // Update frame rate calculation (smoothed over ~1 second)
        self.frame_time_accumulator += time_delta;
        self.frames_in_second += 1;
        if self.frame_time_accumulator >= 1.0 {
            self.current_frame_rate = self.frames_in_second as f32 / self.frame_time_accumulator;
            self.frame_time_accumulator = 0.0;
            self.frames_in_second = 0;
        }

        // Increment frame counter
        self.frame_count = self.frame_count.wrapping_add(1);

        // Calculate iMouse uniform
        // xy = current position (Shadertoy uses bottom-left origin, so flip Y)
        // zw = click position (positive when button down, negative when up)
        let height = self.texture_height as f32;
        let mouse_y_flipped = height - self.mouse_position[1];
        let click_y_flipped = height - self.mouse_click_position[1];

        let mouse = if self.mouse_button_down {
            // When dragging, xy = current position, zw = positive click position
            [
                self.mouse_position[0],
                mouse_y_flipped,
                self.mouse_click_position[0],
                click_y_flipped,
            ]
        } else {
            // When not dragging, xy = last drag position (or 0), zw = negative click position
            [
                self.mouse_position[0],
                mouse_y_flipped,
                -self.mouse_click_position[0].abs(),
                -click_y_flipped.abs(),
            ]
        };

        // Calculate iDate uniform
        // x = year, y = month (0-11), z = day (1-31), w = seconds since midnight
        let date = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let now_sys = SystemTime::now();
            let since_epoch = now_sys.duration_since(UNIX_EPOCH).unwrap_or_default();
            let secs = since_epoch.as_secs();

            // Calculate date components (simplified UTC calculation)
            // This is a basic implementation - for more accuracy, consider using chrono
            let days_since_epoch = secs / 86400;
            let secs_today = (secs % 86400) as f32;

            // Approximate year/month/day calculation
            // Starting from 1970-01-01
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

            let day = remaining_days + 1; // Days are 1-indexed

            [year as f32, month as f32, day as f32, secs_today]
        };

        // Calculate cursor pixel positions
        let (curr_x, curr_y) =
            self.cursor_to_pixels(self.current_cursor_pos.0, self.current_cursor_pos.1);
        let (prev_x, prev_y) =
            self.cursor_to_pixels(self.previous_cursor_pos.0, self.previous_cursor_pos.1);

        // Debug: Log cursor position info periodically (every 60 frames)
        if self.frame_count.is_multiple_of(60) {
            log::debug!(
                "CURSOR_SHADER: pos=({},{}) -> pixels=({:.1},{:.1}), cell=({:.1}x{:.1}), padding={:.1}, resolution={}x{}",
                self.current_cursor_pos.0,
                self.current_cursor_pos.1,
                curr_x,
                curr_y,
                self.cursor_cell_width,
                self.cursor_cell_height,
                self.cursor_window_padding,
                self.texture_width,
                self.texture_height
            );
        }

        // Update uniforms
        let uniforms = CustomShaderUniforms {
            resolution: [self.texture_width as f32, self.texture_height as f32],
            time,
            time_delta,
            mouse,
            date,
            opacity: self.window_opacity,
            text_opacity: self.text_opacity,
            full_content_mode: if self.full_content_mode { 1.0 } else { 0.0 },
            frame: self.frame_count as f32,
            frame_rate: self.current_frame_rate,
            resolution_z: 1.0, // Pixel aspect ratio, usually 1.0
            _pad1: [0.0, 0.0],
            // Cursor uniforms (Ghostty-compatible)
            // Cursor dimensions vary by style:
            // - Block: full cell width x height
            // - Beam/Bar: thin width (2px) x full height
            // - Underline: full width x thin height (2px)
            current_cursor: [
                curr_x,
                curr_y,
                self.cursor_width_for_style(self.current_cursor_style),
                self.cursor_height_for_style(self.current_cursor_style),
            ],
            previous_cursor: [
                prev_x,
                prev_y,
                self.cursor_width_for_style(self.previous_cursor_style),
                self.cursor_height_for_style(self.previous_cursor_style),
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
            // Cursor shader configuration (floats first, then vec4 at aligned offset)
            cursor_trail_duration: self.cursor_trail_duration,
            cursor_glow_radius: self.cursor_glow_radius,
            cursor_glow_intensity: self.cursor_glow_intensity,
            cursor_shader_color: self.cursor_shader_color,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Create command encoder
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Custom Shader Encoder"),
        });

        // Render pass
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Custom Shader Render Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        // Clear to transparent to support window transparency
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Check if animation is enabled
    #[allow(dead_code)]
    pub fn animation_enabled(&self) -> bool {
        self.animation_enabled
    }

    /// Set animation enabled state
    #[allow(dead_code)]
    pub fn set_animation_enabled(&mut self, enabled: bool) {
        self.animation_enabled = enabled;
        if enabled {
            // Reset start time when enabling animation
            self.start_time = Instant::now();
        }
    }

    /// Update animation speed multiplier
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.animation_speed = speed.max(0.0);
    }

    /// Update window opacity (content alpha passed to shader)
    pub fn set_opacity(&mut self, opacity: f32) {
        self.window_opacity = opacity.clamp(0.0, 1.0);
    }

    /// Update full content mode
    pub fn set_full_content_mode(&mut self, enabled: bool) {
        self.full_content_mode = enabled;
    }

    /// Check if full content mode is enabled
    #[allow(dead_code)]
    pub fn full_content_mode(&self) -> bool {
        self.full_content_mode
    }

    /// Update mouse position in pixel coordinates
    ///
    /// # Arguments
    /// * `x` - Mouse X position in pixels (0 = left edge)
    /// * `y` - Mouse Y position in pixels (0 = top edge, will be flipped for shader)
    pub fn set_mouse_position(&mut self, x: f32, y: f32) {
        self.mouse_position = [x, y];
    }

    /// Update mouse button state and click position
    ///
    /// Call this when the left mouse button is pressed or released.
    ///
    /// # Arguments
    /// * `pressed` - True if mouse button is now pressed, false if released
    /// * `x` - Mouse X position in pixels at time of click/release
    /// * `y` - Mouse Y position in pixels at time of click/release (will be flipped for shader)
    pub fn set_mouse_button(&mut self, pressed: bool, x: f32, y: f32) {
        self.mouse_button_down = pressed;
        if pressed {
            // Record click position when button is pressed
            self.mouse_click_position = [x, y];
        }
    }

    // ============ Cursor tracking methods (Ghostty-compatible) ============

    /// Update cursor position and appearance for shader effects
    ///
    /// This method tracks cursor movement and records the time of change,
    /// enabling Ghostty-compatible cursor trail effects and animations.
    ///
    /// # Arguments
    /// * `col` - Cursor column position (0-based)
    /// * `row` - Cursor row position (0-based)
    /// * `opacity` - Cursor opacity (0.0 = invisible, 1.0 = fully visible)
    /// * `cursor_color` - Cursor RGBA color
    /// * `style` - Cursor style (Block, Beam, Underline)
    pub fn update_cursor(
        &mut self,
        col: usize,
        row: usize,
        opacity: f32,
        cursor_color: [f32; 4],
        style: CursorStyle,
    ) {
        let new_pos = (col, row);
        let style_changed = style != self.current_cursor_style;
        let pos_changed = new_pos != self.current_cursor_pos;

        if pos_changed || style_changed {
            // Store previous state before updating
            self.previous_cursor_pos = self.current_cursor_pos;
            self.previous_cursor_opacity = self.current_cursor_opacity;
            self.previous_cursor_color = self.current_cursor_color;
            self.previous_cursor_style = self.current_cursor_style;
            self.current_cursor_pos = new_pos;
            self.current_cursor_style = style;

            // Record time of change (same timebase as iTime)
            self.cursor_change_time = if self.animation_enabled {
                self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0)
            } else {
                0.0
            };

            if pos_changed {
                log::trace!(
                    "Cursor moved: ({}, {}) -> ({}, {}), change_time={:.3}",
                    self.previous_cursor_pos.0,
                    self.previous_cursor_pos.1,
                    col,
                    row,
                    self.cursor_change_time
                );
            }
        }
        self.current_cursor_opacity = opacity;
        self.current_cursor_color = cursor_color;
    }

    /// Update cell dimensions for cursor pixel position calculation
    ///
    /// # Arguments
    /// * `cell_width` - Cell width in pixels
    /// * `cell_height` - Cell height in pixels
    /// * `padding` - Window padding in pixels
    pub fn update_cell_dimensions(&mut self, cell_width: f32, cell_height: f32, padding: f32) {
        self.cursor_cell_width = cell_width;
        self.cursor_cell_height = cell_height;
        self.cursor_window_padding = padding;
    }

    /// Convert cursor cell coordinates to pixel coordinates
    ///
    /// Returns (x, y) in pixels from top-left corner of the window.
    fn cursor_to_pixels(&self, col: usize, row: usize) -> (f32, f32) {
        let x = self.cursor_window_padding + (col as f32 * self.cursor_cell_width);
        let y = self.cursor_window_padding + (row as f32 * self.cursor_cell_height);
        (x, y)
    }

    /// Get cursor width in pixels based on cursor style
    fn cursor_width_for_style(&self, style: CursorStyle) -> f32 {
        match style {
            // Block cursor: full cell width
            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => self.cursor_cell_width,
            // Beam/Bar cursor: thin vertical line (2 pixels)
            CursorStyle::SteadyBar | CursorStyle::BlinkingBar => 2.0,
            // Underline cursor: full cell width
            CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => self.cursor_cell_width,
        }
    }

    /// Get cursor height in pixels based on cursor style
    fn cursor_height_for_style(&self, style: CursorStyle) -> f32 {
        match style {
            // Block cursor: full cell height
            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => self.cursor_cell_height,
            // Beam/Bar cursor: full cell height
            CursorStyle::SteadyBar | CursorStyle::BlinkingBar => self.cursor_cell_height,
            // Underline cursor: thin horizontal line (2 pixels)
            CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => 2.0,
        }
    }

    /// Check if cursor animation might need continuous rendering
    ///
    /// Returns true if a cursor trail animation is likely still in progress
    /// (within 1 second of the last cursor movement).
    pub fn cursor_needs_animation(&self) -> bool {
        if self.animation_enabled {
            let current_time =
                self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0);
            // Allow 1 second for cursor trail animations to complete
            (current_time - self.cursor_change_time) < 1.0
        } else {
            false
        }
    }

    /// Update cursor shader configuration from config values
    ///
    /// # Arguments
    /// * `color` - Cursor color for shader effects [R, G, B] (0-255)
    /// * `trail_duration` - Duration of cursor trail effect in seconds
    /// * `glow_radius` - Radius of cursor glow effect in pixels
    /// * `glow_intensity` - Intensity of cursor glow effect (0.0-1.0)
    pub fn update_cursor_shader_config(
        &mut self,
        color: [u8; 3],
        trail_duration: f32,
        glow_radius: f32,
        glow_intensity: f32,
    ) {
        self.cursor_shader_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            1.0,
        ];
        self.cursor_trail_duration = trail_duration.max(0.0);
        self.cursor_glow_radius = glow_radius.max(0.0);
        self.cursor_glow_intensity = glow_intensity.clamp(0.0, 1.0);
    }

    /// Reload the shader from a source string
    ///
    /// This method compiles the new shader source and replaces the current pipeline.
    /// If compilation fails, returns an error and the old shader remains active.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `source` - The GLSL shader source code
    /// * `name` - A name for error messages (e.g., "editor")
    ///
    /// # Returns
    /// Ok(()) if successful, Err with error message if compilation fails
    pub fn reload_from_source(&mut self, device: &Device, source: &str, name: &str) -> Result<()> {
        // Transpile GLSL to WGSL
        let wgsl_source = transpile_glsl_to_wgsl_source(source, name)?;

        log::info!(
            "Reloading custom shader from source ({} bytes GLSL -> {} bytes WGSL)",
            source.len(),
            wgsl_source.len()
        );
        log::debug!("Generated WGSL:\n{}", wgsl_source);

        // Pre-validate WGSL to surface errors gracefully
        let module = naga::front::wgsl::parse_str(&wgsl_source)
            .context("Custom shader WGSL parse failed")?;
        let _info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .context("Custom shader WGSL validation failed")?;

        // Create the shader module
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Custom Shader Module (reloaded)"),
            source: ShaderSource::Wgsl(wgsl_source.into()),
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Custom Shader Pipeline Layout (reloaded)"),
            bind_group_layouts: &[&self.bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Custom Shader Pipeline (reloaded)"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: self.surface_format,
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
        });

        // Success! Replace the old pipeline
        self.pipeline = pipeline;

        // Reset animation timer
        self.start_time = Instant::now();

        log::info!("Custom shader reloaded successfully from source");
        Ok(())
    }
}
