//! Custom shader renderer for post-processing effects
//!
//! Supports Ghostty/Shadertoy-style GLSL shaders with the following uniforms:
//! - `iTime`: Time in seconds (animated or fixed at 0.0)
//! - `iResolution`: Viewport resolution (width, height, 1.0)
//! - `iChannel0`: Terminal content texture

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Instant;
use wgpu::*;

/// Uniform data passed to custom shaders
/// Layout must match GLSL std140 rules:
/// - vec2 aligned to 8 bytes
/// - vec4 aligned to 16 bytes
/// - float aligned to 4 bytes
/// - struct size rounded to 16 bytes (largest alignment)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CustomShaderUniforms {
    /// Viewport resolution (iResolution.xy) - offset 0, size 8
    resolution: [f32; 2],
    /// Time in seconds since shader started (iTime) - offset 8, size 4
    time: f32,
    /// Time since last frame in seconds (iTimeDelta) - offset 12, size 4
    time_delta: f32,
    /// Mouse state (iMouse) - offset 16, size 16
    /// xy = current position (if dragging) or last drag position
    /// zw = click position (positive when held, negative when released)
    mouse: [f32; 4],
    /// Date/time (iDate) - offset 32, size 16
    /// x = year, y = month (0-11), z = day (1-31), w = seconds since midnight
    date: [f32; 4],
    /// Window opacity for transparency support - offset 48, size 4
    opacity: f32,
    /// Text opacity (separate from window opacity) - offset 52, size 4
    text_opacity: f32,
    /// Full content mode: 1.0 = shader receives and outputs full content, 0.0 = background only
    full_content_mode: f32,
    /// Frame counter (iFrame) - offset 60, size 4
    frame: f32,
    /// Current frame rate in FPS (iFrameRate) - offset 64, size 4
    frame_rate: f32,
    /// Pixel aspect ratio (iResolution.z) - offset 68, size 4, usually 1.0
    resolution_z: f32,
    /// Padding to reach 80 bytes (multiple of 16) - offset 72, size 8
    _padding: [f32; 2],
}
// Total size: 80 bytes

/// Custom shader renderer that applies post-processing effects
pub struct CustomShaderRenderer {
    /// The render pipeline for the custom shader
    pipeline: RenderPipeline,
    /// Bind group for shader uniforms and textures
    bind_group: BindGroup,
    /// Uniform buffer for shader parameters
    uniform_buffer: Buffer,
    /// Intermediate texture to render terminal content into
    intermediate_texture: Texture,
    /// View of the intermediate texture
    intermediate_texture_view: TextureView,
    /// Start time for animation
    start_time: Instant,
    /// Whether animation is enabled
    animation_enabled: bool,
    /// Animation speed multiplier
    animation_speed: f32,
    /// Current texture dimensions
    texture_width: u32,
    texture_height: u32,
    /// Surface format for compatibility
    surface_format: TextureFormat,
    /// Bind group layout for recreating bind groups on resize
    bind_group_layout: BindGroupLayout,
    /// Sampler for the intermediate texture
    sampler: Sampler,
    /// Window opacity for transparency
    window_opacity: f32,
    /// Text opacity (separate from window opacity)
    text_opacity: f32,
    /// Full content mode - shader receives and can manipulate full terminal content
    full_content_mode: bool,
    /// Frame counter for iFrame uniform
    frame_count: u32,
    /// Last frame time for calculating time delta
    last_frame_time: Instant,
    /// Current mouse position in pixels (xy)
    mouse_position: [f32; 2],
    /// Last click position in pixels (zw)
    mouse_click_position: [f32; 2],
    /// Whether mouse button is currently pressed (affects sign of zw)
    mouse_button_down: bool,
    /// Frame rate tracking: time accumulator for averaging
    frame_time_accumulator: f32,
    /// Frame rate tracking: frames in current second
    frames_in_second: u32,
    /// Current smoothed frame rate
    current_frame_rate: f32,
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
            _padding: [0.0, 0.0],
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

/// Transpile a Ghostty/Shadertoy-style GLSL shader to WGSL
///
/// The input shader should have a `mainImage(out vec4 fragColor, in vec2 fragCoord)` function
/// and can use the following uniforms:
/// - `iTime`: Time in seconds
/// - `iTimeDelta`: Time since last frame in seconds
/// - `iFrame`: Frame counter
/// - `iFrameRate`: Current frame rate in FPS
/// - `iResolution`: Viewport resolution (vec2, z available as iResolutionZ)
/// - `iMouse`: Mouse state (xy=current pos, zw=click pos)
/// - `iDate`: Date/time (year, month, day, seconds since midnight)
/// - `iChannel0`: Terminal content texture (sampler2D)
/// - `iOpacity`: Window opacity (par-term specific)
/// - `iTextOpacity`: Text opacity (par-term specific)
/// - `iFullContent`: Full content mode flag (par-term specific)
fn transpile_glsl_to_wgsl(glsl_source: &str, shader_path: &Path) -> Result<String> {
    // Wrap the Shadertoy-style shader in a proper GLSL fragment shader
    // We need to:
    // 1. Add version and precision qualifiers
    // 2. Declare uniforms and samplers
    // 3. Add input/output declarations
    // 4. Add a main() that calls mainImage()

    let wrapped_glsl = format!(
        r#"#version 450

// Uniforms - must match Rust struct layout (std140)
// Total size: 80 bytes
layout(set = 0, binding = 0) uniform Uniforms {{
    vec2 iResolution;      // offset 0, size 8 - Viewport resolution
    float iTime;           // offset 8, size 4 - Time in seconds
    float iTimeDelta;      // offset 12, size 4 - Time since last frame
    vec4 iMouse;           // offset 16, size 16 - Mouse state (xy=current, zw=click)
    vec4 iDate;            // offset 32, size 16 - Date (year, month, day, seconds)
    float iOpacity;        // offset 48, size 4 - Window opacity
    float iTextOpacity;    // offset 52, size 4 - Text opacity
    float iFullContent;    // offset 56, size 4 - Full content mode (1.0 = enabled)
    float iFrame;          // offset 60, size 4 - Frame counter
    float iFrameRate;      // offset 64, size 4 - Current FPS
    float iResolutionZ;    // offset 68, size 4 - Pixel aspect ratio (usually 1.0)
    vec2 _pad;             // offset 72, size 8 - Padding
}};                        // total: 80 bytes

// Terminal content texture (iChannel0)
layout(set = 0, binding = 1) uniform texture2D _iChannel0Tex;
layout(set = 0, binding = 2) uniform sampler _iChannel0Sampler;

// Combined sampler for texture() calls
#define iChannel0 sampler2D(_iChannel0Tex, _iChannel0Sampler)

// Input from vertex shader
layout(location = 0) in vec2 v_uv;

// Output color
layout(location = 0) out vec4 outColor;

// ============ User shader code begins ============

{glsl_source}

// ============ User shader code ends ============

void main() {{
    vec2 fragCoord = v_uv * iResolution;
    vec4 shaderColor;
    mainImage(shaderColor, fragCoord);

    if (iFullContent > 0.5) {{
        // Full content mode: shader output is used directly
        // The shader has full control over the terminal content via iChannel0
        // Apply window opacity to the shader's alpha output
        outColor = vec4(shaderColor.rgb * iOpacity, shaderColor.a * iOpacity);
    }} else {{
        // Background-only mode: text is composited cleanly on top
        // Sample terminal to detect text pixels
        vec4 terminalColor = texture(iChannel0, v_uv);
        float hasText = step(0.01, terminalColor.a);

        // Text pixels: use terminal color with text opacity
        // Background pixels: use shader output with window opacity
        vec3 textCol = terminalColor.rgb;
        vec3 bgCol = shaderColor.rgb;

        // Composite: text over shader background
        float textA = hasText * iTextOpacity;
        float bgA = (1.0 - hasText) * iOpacity;

        vec3 finalRgb = textCol * textA + bgCol * bgA;
        float finalA = textA + bgA;

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    );

    // Parse GLSL using naga
    let mut parser = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);

    let module = parser.parse(&options, &wrapped_glsl).map_err(|errors| {
        let error_messages: Vec<String> = errors
            .errors
            .iter()
            .map(|e| format!("  {:?}", e.kind))
            .collect();
        anyhow::anyhow!(
            "GLSL parse error in '{}'. Errors:\n{}",
            shader_path.display(),
            error_messages.join("\n")
        )
    })?;

    // Validate the module
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| {
        anyhow::anyhow!(
            "Shader validation failed for '{}': {:?}",
            shader_path.display(),
            e
        )
    })?;

    // Generate WGSL output for fragment shader
    let mut fragment_wgsl = String::new();
    let mut writer =
        naga::back::wgsl::Writer::new(&mut fragment_wgsl, naga::back::wgsl::WriterFlags::empty());

    writer.write(&module, &info).map_err(|e| {
        anyhow::anyhow!(
            "WGSL generation failed for '{}': {:?}",
            shader_path.display(),
            e
        )
    })?;

    // The generated WGSL will have a main() function but we need to rename it to fs_main
    // and add a vertex shader
    let fragment_wgsl = fragment_wgsl.replace("fn main(", "fn fs_main(");

    // Build the complete shader with vertex shader
    let full_wgsl = format!(
        r#"// Auto-generated WGSL from GLSL shader: {}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {{
    var out: VertexOutput;

    // Generate full-screen quad vertices (triangle strip)
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Full screen in NDC
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);

    return out;
}}

// ============ Fragment shader (transpiled from GLSL) ============

{fragment_wgsl}
"#,
        shader_path.display()
    );

    Ok(full_wgsl)
}

/// Transpile a Ghostty/Shadertoy-style GLSL shader to WGSL from source string
///
/// Same as `transpile_glsl_to_wgsl` but takes a source string and name instead of a file path.
fn transpile_glsl_to_wgsl_source(glsl_source: &str, name: &str) -> Result<String> {
    // Wrap the Shadertoy-style shader in a proper GLSL fragment shader
    let wrapped_glsl = format!(
        r#"#version 450

// Uniforms - must match Rust struct layout (std140)
// Total size: 80 bytes
layout(set = 0, binding = 0) uniform Uniforms {{
    vec2 iResolution;      // offset 0, size 8 - Viewport resolution
    float iTime;           // offset 8, size 4 - Time in seconds
    float iTimeDelta;      // offset 12, size 4 - Time since last frame
    vec4 iMouse;           // offset 16, size 16 - Mouse state (xy=current, zw=click)
    vec4 iDate;            // offset 32, size 16 - Date (year, month, day, seconds)
    float iOpacity;        // offset 48, size 4 - Window opacity
    float iTextOpacity;    // offset 52, size 4 - Text opacity
    float iFullContent;    // offset 56, size 4 - Full content mode (1.0 = enabled)
    float iFrame;          // offset 60, size 4 - Frame counter
    float iFrameRate;      // offset 64, size 4 - Current FPS
    float iResolutionZ;    // offset 68, size 4 - Pixel aspect ratio (usually 1.0)
    vec2 _pad;             // offset 72, size 8 - Padding
}};                        // total: 80 bytes

// Terminal content texture (iChannel0)
layout(set = 0, binding = 1) uniform texture2D _iChannel0Tex;
layout(set = 0, binding = 2) uniform sampler _iChannel0Sampler;

// Combined sampler for texture() calls
#define iChannel0 sampler2D(_iChannel0Tex, _iChannel0Sampler)

// Input from vertex shader
layout(location = 0) in vec2 v_uv;

// Output color
layout(location = 0) out vec4 outColor;

// ============ User shader code begins ============

{glsl_source}

// ============ User shader code ends ============

void main() {{
    vec2 fragCoord = v_uv * iResolution;
    vec4 shaderColor;
    mainImage(shaderColor, fragCoord);

    if (iFullContent > 0.5) {{
        // Full content mode: shader output is used directly
        // The shader has full control over the terminal content via iChannel0
        // Apply window opacity to the shader's alpha output
        outColor = vec4(shaderColor.rgb * iOpacity, shaderColor.a * iOpacity);
    }} else {{
        // Background-only mode: text is composited cleanly on top
        // Sample terminal to detect text pixels
        vec4 terminalColor = texture(iChannel0, v_uv);
        float hasText = step(0.01, terminalColor.a);

        // Text pixels: use terminal color with text opacity
        // Background pixels: use shader output with window opacity
        vec3 textCol = terminalColor.rgb;
        vec3 bgCol = shaderColor.rgb;

        // Composite: text over shader background
        float textA = hasText * iTextOpacity;
        float bgA = (1.0 - hasText) * iOpacity;

        vec3 finalRgb = textCol * textA + bgCol * bgA;
        float finalA = textA + bgA;

        outColor = vec4(finalRgb, finalA);
    }}
}}
"#
    );

    // Parse GLSL using naga
    let mut parser = naga::front::glsl::Frontend::default();
    let options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);

    let module = parser.parse(&options, &wrapped_glsl).map_err(|errors| {
        let error_messages: Vec<String> = errors
            .errors
            .iter()
            .map(|e| format!("  {:?}", e.kind))
            .collect();
        anyhow::anyhow!(
            "GLSL parse error in '{}'. Errors:\n{}",
            name,
            error_messages.join("\n")
        )
    })?;

    // Validate the module
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| anyhow::anyhow!("Shader validation failed for '{}': {:?}", name, e))?;

    // Generate WGSL output for fragment shader
    let mut fragment_wgsl = String::new();
    let mut writer =
        naga::back::wgsl::Writer::new(&mut fragment_wgsl, naga::back::wgsl::WriterFlags::empty());

    writer
        .write(&module, &info)
        .map_err(|e| anyhow::anyhow!("WGSL generation failed for '{}': {:?}", name, e))?;

    // The generated WGSL will have a main() function but we need to rename it to fs_main
    // and add a vertex shader
    let fragment_wgsl = fragment_wgsl.replace("fn main(", "fn fs_main(");

    // Build the complete shader with vertex shader
    let full_wgsl = format!(
        r#"// Auto-generated WGSL from GLSL shader: {}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {{
    var out: VertexOutput;

    // Generate full-screen quad vertices (triangle strip)
    let x = f32(vertex_index & 1u);
    let y = f32((vertex_index >> 1u) & 1u);

    // Full screen in NDC
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);

    return out;
}}

// ============ Fragment shader (transpiled from GLSL) ============

{fragment_wgsl}
"#,
        name
    );

    Ok(full_wgsl)
}
