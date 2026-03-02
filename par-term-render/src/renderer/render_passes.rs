use anyhow::Result;

use super::{DividerRenderInfo, PaneDividerSettings, PaneTitleInfo, Renderer};
use crate::cell_renderer::PaneViewport;

impl Renderer {
    /// Render pane dividers on top of pane content
    ///
    /// This should be called after rendering pane content but before egui.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `dividers` - List of dividers to render with hover state
    /// * `settings` - Divider appearance settings
    pub fn render_dividers(
        &mut self,
        surface_view: &wgpu::TextureView,
        dividers: &[DividerRenderInfo],
        settings: &PaneDividerSettings,
    ) -> Result<()> {
        if dividers.is_empty() {
            return Ok(());
        }

        // Build divider instances using the cell renderer's background pipeline
        // We reuse the bg_instances buffer for dividers
        let mut instances = Vec::with_capacity(dividers.len() * 3); // Extra capacity for multi-rect styles

        let w = self.size.width as f32;
        let h = self.size.height as f32;

        for divider in dividers {
            let color = if divider.hovered {
                settings.hover_color
            } else {
                settings.divider_color
            };

            use par_term_config::DividerStyle;
            match settings.divider_style {
                DividerStyle::Solid => {
                    let x_ndc = divider.x / w * 2.0 - 1.0;
                    let y_ndc = 1.0 - (divider.y / h * 2.0);
                    let w_ndc = divider.width / w * 2.0;
                    let h_ndc = divider.height / h * 2.0;

                    instances.push(crate::cell_renderer::types::BackgroundInstance {
                        position: [x_ndc, y_ndc],
                        size: [w_ndc, h_ndc],
                        color: [color[0], color[1], color[2], 1.0],
                    });
                }
                DividerStyle::Double => {
                    // Two parallel lines with a visible gap between them
                    let is_horizontal = divider.width > divider.height;
                    let thickness = if is_horizontal {
                        divider.height
                    } else {
                        divider.width
                    };

                    if thickness >= 4.0 {
                        // Enough space for two 1px lines with visible gap
                        if is_horizontal {
                            // Top line
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            // Bottom line (gap in between shows background)
                            let bottom_y = divider.y + divider.height - 1.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (bottom_y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        } else {
                            // Left line
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            // Right line
                            let right_x = divider.x + divider.width - 1.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [right_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        }
                    } else {
                        // Divider too thin for double lines — render centered 1px line
                        // (visibly thinner than Solid to differentiate)
                        if is_horizontal {
                            let center_y = divider.y + (divider.height - 1.0) / 2.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (center_y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        } else {
                            let center_x = divider.x + (divider.width - 1.0) / 2.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [center_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        }
                    }
                }
                DividerStyle::Dashed => {
                    // Dashed line effect using segments
                    let is_horizontal = divider.width > divider.height;
                    let dash_len: f32 = 6.0;
                    let gap_len: f32 = 4.0;

                    if is_horizontal {
                        let mut x = divider.x;
                        while x < divider.x + divider.width {
                            let seg_w = dash_len.min(divider.x + divider.width - x);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [seg_w / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            x += dash_len + gap_len;
                        }
                    } else {
                        let mut y = divider.y;
                        while y < divider.y + divider.height {
                            let seg_h = dash_len.min(divider.y + divider.height - y);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (y / h * 2.0)],
                                size: [divider.width / w * 2.0, seg_h / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            y += dash_len + gap_len;
                        }
                    }
                }
                DividerStyle::Shadow => {
                    // Beveled/embossed effect — all rendering stays within divider bounds
                    // Highlight on top/left edge, shadow on bottom/right edge
                    let is_horizontal = divider.width > divider.height;
                    let thickness = if is_horizontal {
                        divider.height
                    } else {
                        divider.width
                    };

                    // Brighter highlight color
                    let highlight = [
                        (color[0] + 0.3).min(1.0),
                        (color[1] + 0.3).min(1.0),
                        (color[2] + 0.3).min(1.0),
                        1.0,
                    ];
                    // Darker shadow color
                    let shadow = [(color[0] * 0.3), (color[1] * 0.3), (color[2] * 0.3), 1.0];

                    if thickness >= 3.0 {
                        // 3+ px: highlight line / main body / shadow line
                        let edge = 1.0_f32;
                        if is_horizontal {
                            // Top highlight
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, edge / h * 2.0],
                                color: highlight,
                            });
                            // Main body (middle portion)
                            let body_y = divider.y + edge;
                            let body_h = divider.height - edge * 2.0;
                            if body_h > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [divider.x / w * 2.0 - 1.0, 1.0 - (body_y / h * 2.0)],
                                    size: [divider.width / w * 2.0, body_h / h * 2.0],
                                    color: [color[0], color[1], color[2], 1.0],
                                });
                            }
                            // Bottom shadow
                            let shadow_y = divider.y + divider.height - edge;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (shadow_y / h * 2.0)],
                                size: [divider.width / w * 2.0, edge / h * 2.0],
                                color: shadow,
                            });
                        } else {
                            // Left highlight
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [edge / w * 2.0, divider.height / h * 2.0],
                                color: highlight,
                            });
                            // Main body
                            let body_x = divider.x + edge;
                            let body_w = divider.width - edge * 2.0;
                            if body_w > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [body_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                    size: [body_w / w * 2.0, divider.height / h * 2.0],
                                    color: [color[0], color[1], color[2], 1.0],
                                });
                            }
                            // Right shadow
                            let shadow_x = divider.x + divider.width - edge;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [shadow_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [edge / w * 2.0, divider.height / h * 2.0],
                                color: shadow,
                            });
                        }
                    } else {
                        // 2px or less: top/left half highlight, bottom/right half shadow
                        if is_horizontal {
                            let half = (divider.height / 2.0).max(1.0);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, half / h * 2.0],
                                color: highlight,
                            });
                            let bottom_y = divider.y + half;
                            let bottom_h = divider.height - half;
                            if bottom_h > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [
                                        divider.x / w * 2.0 - 1.0,
                                        1.0 - (bottom_y / h * 2.0),
                                    ],
                                    size: [divider.width / w * 2.0, bottom_h / h * 2.0],
                                    color: shadow,
                                });
                            }
                        } else {
                            let half = (divider.width / 2.0).max(1.0);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [half / w * 2.0, divider.height / h * 2.0],
                                color: highlight,
                            });
                            let right_x = divider.x + half;
                            let right_w = divider.width - half;
                            if right_w > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [
                                        right_x / w * 2.0 - 1.0,
                                        1.0 - (divider.y / h * 2.0),
                                    ],
                                    size: [right_w / w * 2.0, divider.height / h * 2.0],
                                    color: shadow,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Write instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

        // Render dividers
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("divider render encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("divider render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render focus indicator around a pane
    ///
    /// This draws a colored border around the focused pane to highlight it.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `viewport` - The focused pane's viewport
    /// * `settings` - Divider/focus settings
    pub fn render_focus_indicator(
        &mut self,
        surface_view: &wgpu::TextureView,
        viewport: &PaneViewport,
        settings: &PaneDividerSettings,
    ) -> Result<()> {
        if !settings.show_focus_indicator {
            return Ok(());
        }

        let border_w = settings.focus_width;
        let color = [
            settings.focus_color[0],
            settings.focus_color[1],
            settings.focus_color[2],
            1.0,
        ];

        // Create 4 border rectangles (top, bottom, left, right)
        let instances = vec![
            // Top border
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - (viewport.y / self.size.height as f32 * 2.0),
                ],
                size: [
                    viewport.width / self.size.width as f32 * 2.0,
                    border_w / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Bottom border
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + viewport.height - border_w) / self.size.height as f32
                        * 2.0),
                ],
                size: [
                    viewport.width / self.size.width as f32 * 2.0,
                    border_w / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Left border (between top and bottom)
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + border_w) / self.size.height as f32 * 2.0),
                ],
                size: [
                    border_w / self.size.width as f32 * 2.0,
                    (viewport.height - border_w * 2.0) / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Right border (between top and bottom)
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    (viewport.x + viewport.width - border_w) / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + border_w) / self.size.height as f32 * 2.0),
                ],
                size: [
                    border_w / self.size.width as f32 * 2.0,
                    (viewport.height - border_w * 2.0) / self.size.height as f32 * 2.0,
                ],
                color,
            },
        ];

        // Write instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

        // Render focus indicator
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("focus indicator encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("focus indicator pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render pane title bars (background rectangles + text)
    ///
    /// Title bars are rendered on top of pane content and dividers.
    /// Each title bar consists of a colored background rectangle and centered text.
    pub fn render_pane_titles(
        &mut self,
        surface_view: &wgpu::TextureView,
        titles: &[PaneTitleInfo],
    ) -> Result<()> {
        if titles.is_empty() {
            return Ok(());
        }

        let width = self.size.width as f32;
        let height = self.size.height as f32;

        // Phase 1: Render title bar backgrounds
        let mut bg_instances = Vec::with_capacity(titles.len());
        for title in titles {
            let x_ndc = title.x / width * 2.0 - 1.0;
            let y_ndc = 1.0 - (title.y / height * 2.0);
            let w_ndc = title.width / width * 2.0;
            let h_ndc = title.height / height * 2.0;

            // Title bar must be fully opaque (alpha=1.0) to cover the background.
            // Differentiate focused/unfocused by lightening/darkening the color.
            let brightness = if title.focused { 1.0 } else { 0.7 };

            bg_instances.push(crate::cell_renderer::types::BackgroundInstance {
                position: [x_ndc, y_ndc],
                size: [w_ndc, h_ndc],
                color: [
                    title.bg_color[0] * brightness,
                    title.bg_color[1] * brightness,
                    title.bg_color[2] * brightness,
                    1.0, // Always fully opaque
                ],
            });
        }

        // Write background instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&bg_instances),
        );

        // Render title backgrounds
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane title bg encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane title bg pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..bg_instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Phase 2: Render title text using glyph atlas
        let mut text_instances = Vec::new();
        let baseline_y = self.cell_renderer.font.font_ascent;

        for title in titles {
            let title_text = &title.title;
            if title_text.is_empty() {
                continue;
            }

            // Calculate starting X position (centered in title bar with left padding)
            let padding_x = 8.0;
            let mut x_pos = title.x + padding_x;
            let y_base = title.y + (title.height - self.cell_renderer.grid.cell_height) / 2.0;

            let text_color = [
                title.text_color[0],
                title.text_color[1],
                title.text_color[2],
                if title.focused { 1.0 } else { 0.8 },
            ];

            // Truncate title if it would overflow the title bar
            let max_chars =
                ((title.width - padding_x * 2.0) / self.cell_renderer.grid.cell_width) as usize;
            let display_text: String = if title_text.len() > max_chars && max_chars > 3 {
                let truncated: String = title_text.chars().take(max_chars - 1).collect();
                format!("{}\u{2026}", truncated) // ellipsis
            } else {
                title_text.clone()
            };

            for ch in display_text.chars() {
                if x_pos >= title.x + title.width - padding_x {
                    break;
                }

                if let Some((font_idx, glyph_id)) =
                    self.cell_renderer.font_manager.find_glyph(ch, false, false)
                {
                    let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                    // Check if this character should be rendered as a monochrome symbol
                    let force_monochrome = crate::cell_renderer::atlas::should_render_as_symbol(ch);
                    let info = if self
                        .cell_renderer
                        .atlas
                        .glyph_cache
                        .contains_key(&cache_key)
                    {
                        self.cell_renderer.lru_remove(cache_key);
                        self.cell_renderer.lru_push_front(cache_key);
                        self.cell_renderer
                            .atlas
                            .glyph_cache
                            .get(&cache_key)
                            .expect("Glyph cache entry must exist after contains_key check")
                            .clone()
                    } else if let Some(raster) =
                        self.cell_renderer
                            .rasterize_glyph(font_idx, glyph_id, force_monochrome)
                    {
                        let info = self.cell_renderer.upload_glyph(cache_key, &raster);
                        self.cell_renderer
                            .atlas
                            .glyph_cache
                            .insert(cache_key, info.clone());
                        self.cell_renderer.lru_push_front(cache_key);
                        info
                    } else {
                        x_pos += self.cell_renderer.grid.cell_width;
                        continue;
                    };

                    let glyph_left = x_pos + info.bearing_x;
                    let glyph_top = y_base + (baseline_y - info.bearing_y);

                    text_instances.push(crate::cell_renderer::types::TextInstance {
                        position: [
                            glyph_left / width * 2.0 - 1.0,
                            1.0 - (glyph_top / height * 2.0),
                        ],
                        size: [
                            info.width as f32 / width * 2.0,
                            info.height as f32 / height * 2.0,
                        ],
                        tex_offset: [info.x as f32 / 2048.0, info.y as f32 / 2048.0],
                        tex_size: [info.width as f32 / 2048.0, info.height as f32 / 2048.0],
                        color: text_color,
                        is_colored: if info.is_colored { 1 } else { 0 },
                    });
                }

                x_pos += self.cell_renderer.grid.cell_width;
            }
        }

        if text_instances.is_empty() {
            return Ok(());
        }

        // Write text instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.text_instance_buffer,
            0,
            bytemuck::cast_slice(&text_instances),
        );

        // Render title text
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane title text encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane title text pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.cell_renderer.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..text_instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        Ok(())
    }
}
