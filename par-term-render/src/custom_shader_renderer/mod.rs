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
mod hot_reload;
pub mod pipeline;
pub mod textures;
pub mod transpiler;
pub mod types;
mod uniforms;

use cubemap::CubemapTexture;
use pipeline::{create_bind_group, create_bind_group_layout, create_render_pipeline};
use textures::{ChannelTexture, load_channel_textures};
use transpiler::transpile_glsl_to_wgsl;

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

/// Parameters for creating a new [`CustomShaderRenderer`].
pub struct CustomShaderRendererConfig<'a> {
    pub surface_format: TextureFormat,
    pub shader_path: &'a Path,
    pub width: u32,
    pub height: u32,
    pub animation_enabled: bool,
    pub animation_speed: f32,
    pub window_opacity: f32,
    pub full_content_mode: bool,
    pub channel_paths: &'a [Option<std::path::PathBuf>; 4],
    pub cubemap_path: Option<&'a Path>,
}

impl CustomShaderRenderer {
    /// Create a new custom shader renderer from a GLSL shader file.
    pub fn new(
        device: &Device,
        queue: &Queue,
        config: CustomShaderRendererConfig<'_>,
    ) -> Result<Self> {
        let CustomShaderRendererConfig {
            surface_format,
            shader_path,
            width,
            height,
            animation_enabled,
            animation_speed,
            window_opacity,
            full_content_mode,
            channel_paths,
            cubemap_path,
        } = config;
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
        let uniform_buffer = Self::create_uniform_buffer(device);

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

    /// Get a view of the intermediate texture for rendering terminal content into
    pub fn intermediate_texture_view(&self) -> &TextureView {
        &self.intermediate_texture_view
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

    /// Set the right content inset (e.g., AI Inspector panel).
    ///
    /// When non-zero, the shader will render to a viewport that excludes
    /// the right inset area, ensuring effects don't appear under the panel.
    pub fn set_content_inset_right(&mut self, inset: f32) {
        self.content_inset_right = inset;
    }
}
