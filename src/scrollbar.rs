use std::sync::Arc;
use wgpu::BindGroupLayout;
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferUsages, ColorTargetState,
    ColorWrites, Device, FragmentState, MultisampleState, PipelineLayoutDescriptor, PrimitiveState,
    PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, TextureFormat, VertexState,
};

use crate::scrollback_metadata::ScrollbackMark;

/// Scrollbar renderer using wgpu
pub struct Scrollbar {
    device: Arc<Device>,
    pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    bind_group: BindGroup,
    track_bind_group: BindGroup,
    track_uniform_buffer: Buffer,
    mark_bind_group_layout: BindGroupLayout,
    width: f32,
    visible: bool,
    position_right: bool, // true = right side, false = left side
    thumb_color: [f32; 4],
    track_color: [f32; 4],

    // Cached state for hit testing and interaction
    scrollbar_x: f32,      // Pixel position X
    scrollbar_y: f32,      // Pixel position Y
    scrollbar_height: f32, // Pixel height (thumb)
    window_width: u32,
    window_height: u32,
    /// Top of the scrollbar track in pixels (accounts for tab bar, etc.)
    track_top: f32,
    /// Height of the scrollbar track in pixels (excludes insets)
    track_pixel_height: f32,

    // Scroll state
    scroll_offset: usize,
    visible_lines: usize,
    total_lines: usize,

    // Mark overlays (prompt/command indicators)
    marks: Vec<ScrollbarMarkInstance>,
    /// Mark hit-test data for tooltip display
    mark_hit_info: Vec<MarkHitInfo>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScrollbarUniforms {
    // Position and size (normalized device coordinates: -1 to 1)
    position: [f32; 2], // x, y
    size: [f32; 2],     // width, height
    // Color (RGBA)
    color: [f32; 4],
}

struct ScrollbarMarkInstance {
    bind_group: BindGroup,
    #[allow(dead_code)]
    buffer: Buffer,
}

/// Data for hit-testing marks on the scrollbar
#[derive(Clone)]
struct MarkHitInfo {
    /// Y position in pixels (from top)
    y_pixel: f32,
    /// Original mark data for tooltip display
    mark: ScrollbackMark,
}

impl Scrollbar {
    /// Create a new scrollbar renderer
    ///
    /// # Arguments
    /// * `device` - WGPU device
    /// * `format` - Texture format
    /// * `width` - Scrollbar width in pixels
    /// * `position` - Scrollbar position ("left" or "right")
    /// * `thumb_color` - RGBA color for thumb [r, g, b, a]
    /// * `track_color` - RGBA color for track [r, g, b, a]
    pub fn new(
        device: std::sync::Arc<Device>,
        format: TextureFormat,
        width: f32,
        position: &str,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) -> Self {
        // Create shader module
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Scrollbar Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/scrollbar.wgsl").into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Scrollbar Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Scrollbar Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Scrollbar Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    // Use premultiplied alpha blending since shader outputs premultiplied colors
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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

        // Create uniform buffers for thumb and track
        // Note: We don't need a vertex buffer because vertices are generated
        // procedurally in the shader using builtin(vertex_index)

        // Thumb uniform buffer
        let thumb_uniforms = ScrollbarUniforms {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            color: thumb_color,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scrollbar Thumb Uniform Buffer"),
            contents: bytemuck::cast_slice(&[thumb_uniforms]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Track uniform buffer
        let track_uniforms = ScrollbarUniforms {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            color: track_color,
        };

        let track_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scrollbar Track Uniform Buffer"),
            contents: bytemuck::cast_slice(&[track_uniforms]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create bind groups
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Scrollbar Thumb Bind Group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let track_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Scrollbar Track Bind Group"),
            layout: &bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: track_uniform_buffer.as_entire_binding(),
            }],
        });

        let mark_bind_group_layout = bind_group_layout.clone();

        let position_right = position.eq_ignore_ascii_case("right");

        Self {
            device,
            pipeline,
            uniform_buffer,
            bind_group,
            track_bind_group,
            track_uniform_buffer,
            mark_bind_group_layout,
            width,
            visible: false,
            position_right,
            thumb_color,
            track_color,
            scrollbar_x: 0.0,
            scrollbar_y: 0.0,
            scrollbar_height: 0.0,
            window_width: 0,
            window_height: 0,
            track_top: 0.0,
            track_pixel_height: 0.0,
            scroll_offset: 0,
            visible_lines: 0,
            total_lines: 0,
            marks: Vec::new(),
            mark_hit_info: Vec::new(),
        }
    }

    /// Update scrollbar position and visibility
    ///
    /// # Arguments
    /// * `scroll_offset` - Current scroll offset (0 = at bottom)
    /// * `visible_lines` - Number of lines visible on screen
    /// * `total_lines` - Total number of lines including scrollback
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    /// * `content_offset_y` - Top inset in pixels (e.g., tab bar at top)
    /// * `content_inset_bottom` - Bottom inset in pixels (e.g., status bar)
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        queue: &Queue,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
        window_width: u32,
        window_height: u32,
        content_offset_y: f32,
        content_inset_bottom: f32,
        marks: &[crate::scrollback_metadata::ScrollbackMark],
    ) {
        // Store parameters for hit testing
        self.scroll_offset = scroll_offset;
        self.visible_lines = visible_lines;
        self.total_lines = total_lines;
        self.window_width = window_width;
        self.window_height = window_height;

        // Show scrollbar when either scrollback exists or mark indicators are available
        self.visible = total_lines > visible_lines || !marks.is_empty();

        if !self.visible {
            return;
        }

        // The visible track area excludes top and bottom insets (tab bar, status bar, etc.)
        let track_pixel_height =
            (window_height as f32 - content_offset_y - content_inset_bottom).max(1.0);
        self.track_top = content_offset_y;
        self.track_pixel_height = track_pixel_height;

        // Calculate scrollbar dimensions (guard against zero)
        let total = total_lines.max(1);
        let viewport_ratio = visible_lines.min(total) as f32 / total as f32;
        let scrollbar_height = (viewport_ratio * track_pixel_height).max(20.0);

        // Calculate scrollbar position
        // When scroll_offset is 0, we're at the bottom
        // When scroll_offset is max, we're at the top
        let max_scroll = total.saturating_sub(visible_lines);

        // Clamp scroll_offset to valid range
        let clamped_offset = scroll_offset.min(max_scroll);

        let scroll_ratio = if max_scroll > 0 {
            (clamped_offset as f32 / max_scroll as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Position from bottom within the visible track area (offset by content_offset_y)
        let scrollbar_y = content_offset_y
            + ((1.0 - scroll_ratio) * (track_pixel_height - scrollbar_height))
                .clamp(0.0, track_pixel_height - scrollbar_height);

        // Store pixel coordinates for hit testing
        // Position on right or left based on config
        self.scrollbar_x = if self.position_right {
            window_width as f32 - self.width
        } else {
            0.0
        };
        self.scrollbar_y = scrollbar_y;
        self.scrollbar_height = scrollbar_height;

        // Convert to normalized device coordinates (-1 to 1)
        let wh = window_height as f32;
        let ndc_width = 2.0 * self.width / window_width as f32;
        let ndc_x = if self.position_right {
            1.0 - ndc_width // align right edge at +1
        } else {
            -1.0 // left edge at -1
        };

        // Track spans only the visible area (between top inset and bottom inset)
        let track_bottom_pixel = wh - content_offset_y - track_pixel_height;
        let track_ndc_y = -1.0 + (2.0 * track_bottom_pixel / wh);
        let track_ndc_height = 2.0 * track_pixel_height / wh;
        let track_uniforms = ScrollbarUniforms {
            position: [ndc_x, track_ndc_y],
            size: [ndc_width, track_ndc_height],
            color: self.track_color,
        };
        queue.write_buffer(
            &self.track_uniform_buffer,
            0,
            bytemuck::cast_slice(&[track_uniforms]),
        );

        // Update thumb uniforms (scrollable part)
        let thumb_bottom = wh - (scrollbar_y + scrollbar_height);
        let thumb_ndc_y = -1.0 + (2.0 * thumb_bottom / wh);
        let thumb_ndc_height = 2.0 * scrollbar_height / wh;
        let thumb_uniforms = ScrollbarUniforms {
            position: [ndc_x, thumb_ndc_y],
            size: [ndc_width, thumb_ndc_height],
            color: self.thumb_color,
        };
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[thumb_uniforms]),
        );

        // Prepare and upload mark uniforms (draw later)
        self.prepare_marks(marks, total_lines, window_height, content_offset_y, content_inset_bottom);
    }

    /// Render the scrollbar (track + thumb)
    pub fn render<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        if !self.visible {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);

        // Render track (background) first
        render_pass.set_bind_group(0, &self.track_bind_group, &[]);
        render_pass.draw(0..4, 0..1);

        // Render thumb on top
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..4, 0..1);

        // Render marks on top of track/thumb
        for mark in &self.marks {
            render_pass.set_bind_group(0, &mark.bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }
    }

    fn prepare_marks(
        &mut self,
        marks: &[crate::scrollback_metadata::ScrollbackMark],
        total_lines: usize,
        window_height: u32,
        content_offset_y: f32,
        content_inset_bottom: f32,
    ) {
        self.marks.clear();
        self.mark_hit_info.clear();

        if total_lines == 0 || marks.is_empty() {
            return;
        }

        let wh = window_height as f32;
        let track_pixel_height = (wh - content_offset_y - content_inset_bottom).max(1.0);
        let mark_height_ndc = (2.0 * 4.0) / wh; // 4px height
        let ndc_width = 2.0 * self.width / self.window_width as f32;
        let ndc_x = if self.position_right {
            1.0 - ndc_width
        } else {
            -1.0
        };

        for mark in marks {
            if mark.line >= total_lines {
                continue;
            }
            let ratio = mark.line as f32 / (total_lines as f32 - 1.0).max(1.0);
            // Position within the constrained track area
            let y_pixel = content_offset_y + ratio * track_pixel_height;
            let ndc_y = 1.0 - 2.0 * y_pixel / wh;

            // Store pixel position for hit testing (y from top)
            self.mark_hit_info.push(MarkHitInfo {
                y_pixel,
                mark: mark.clone(),
            });

            let color = if let Some((r, g, b)) = mark.color {
                [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
            } else {
                match mark.exit_code {
                    Some(0) => [0.2, 0.8, 0.4, 1.0],
                    Some(_) => [0.9, 0.25, 0.2, 1.0],
                    None => [0.6, 0.6, 0.6, 0.9],
                }
            };

            let mark_uniforms = ScrollbarUniforms {
                position: [ndc_x, ndc_y - mark_height_ndc / 2.0],
                size: [ndc_width, mark_height_ndc],
                color,
            };

            let buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Scrollbar Mark Buffer"),
                    contents: bytemuck::cast_slice(&[mark_uniforms]),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                });

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("Scrollbar Mark Bind Group"),
                layout: &self.mark_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            self.marks
                .push(ScrollbarMarkInstance { bind_group, buffer });
        }
    }

    /// Update scrollbar appearance (width and colors) in real-time
    pub fn update_appearance(&mut self, width: f32, thumb_color: [f32; 4], track_color: [f32; 4]) {
        self.width = width;
        self.thumb_color = thumb_color;
        self.track_color = track_color;
        // Note: Visual changes will be reflected on next frame when uniforms are updated
    }

    /// Update scrollbar position side (left/right)
    #[allow(dead_code)]
    pub fn update_position(&mut self, position: &str) {
        self.position_right = !position.eq_ignore_ascii_case("left");
    }

    #[allow(dead_code)]
    pub fn width(&self) -> f32 {
        self.width
    }

    #[allow(dead_code)]
    pub fn position_right(&self) -> bool {
        self.position_right
    }

    /// Check if a point (in pixel coordinates) is within the scrollbar bounds
    ///
    /// # Arguments
    /// * `x` - X coordinate in pixels (from left edge)
    /// * `y` - Y coordinate in pixels (from top edge)
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }

        x >= self.scrollbar_x
            && x <= self.scrollbar_x + self.width
            && y >= self.scrollbar_y
            && y <= self.scrollbar_y + self.scrollbar_height
    }

    /// Check if a point is within the scrollbar track (any Y position)
    pub fn track_contains_x(&self, x: f32) -> bool {
        if !self.visible {
            return false;
        }

        x >= self.scrollbar_x && x <= self.scrollbar_x + self.width
    }

    /// Get the current thumb bounds (top Y in pixels, height in pixels)
    pub fn thumb_bounds(&self) -> Option<(f32, f32)> {
        if !self.visible {
            return None;
        }

        Some((self.scrollbar_y, self.scrollbar_height))
    }

    /// Convert a mouse Y position to a scroll offset
    ///
    /// # Arguments
    /// * `mouse_y` - Desired thumb top Y coordinate in pixels (from top edge)
    ///
    /// # Returns
    /// The scroll offset corresponding to the mouse position, or None if scrollbar is not visible
    pub fn mouse_y_to_scroll_offset(&self, mouse_y: f32) -> Option<usize> {
        if !self.visible {
            return None;
        }

        let max_scroll = self.total_lines.saturating_sub(self.visible_lines);
        if max_scroll == 0 {
            return Some(0);
        }

        // Calculate the scrollable track area (space the thumb can move within the track)
        let track_height = (self.track_pixel_height - self.scrollbar_height).max(1.0);

        // Clamp mouse position relative to the track top
        let relative_y = mouse_y - self.track_top;
        let clamped_y = relative_y.clamp(0.0, track_height);

        // Calculate scroll ratio (inverted because 0 = bottom)
        let scroll_ratio = 1.0 - (clamped_y / track_height);

        // Convert to scroll offset
        let scroll_offset = (scroll_ratio * max_scroll as f32).round() as usize;

        Some(scroll_offset.min(max_scroll))
    }

    /// Whether the scrollbar is currently visible
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Find a mark at the given mouse position (in pixels from top-left).
    /// Returns the mark if the mouse is within `tolerance` pixels of a mark's Y position
    /// and within the scrollbar's X bounds.
    pub fn mark_at_position(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        tolerance: f32,
    ) -> Option<&ScrollbackMark> {
        if !self.visible || !self.track_contains_x(mouse_x) {
            return None;
        }

        // Find the closest mark within tolerance
        let mut closest: Option<(f32, &MarkHitInfo)> = None;
        for hit_info in &self.mark_hit_info {
            let distance = (hit_info.y_pixel - mouse_y).abs();
            if distance <= tolerance {
                match closest {
                    Some((best_dist, _)) if distance < best_dist => {
                        closest = Some((distance, hit_info));
                    }
                    None => {
                        closest = Some((distance, hit_info));
                    }
                    _ => {}
                }
            }
        }

        closest.map(|(_, hit_info)| &hit_info.mark)
    }
}
