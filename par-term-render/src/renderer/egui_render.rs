use anyhow::Result;

use super::Renderer;

impl Renderer {
    /// Render egui overlay on top of the terminal
    pub(crate) fn render_egui(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
        egui_output: egui::FullOutput,
        egui_ctx: &egui::Context,
        force_opaque: bool,
    ) -> Result<()> {
        use wgpu::TextureViewDescriptor;

        // Create view of the surface texture
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        // Create command encoder for egui
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui encoder"),
                });

        // Convert egui output to screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: egui_output.pixels_per_point,
        };

        // Update egui textures
        for (id, image_delta) in &egui_output.textures_delta.set {
            self.egui_renderer.update_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                *id,
                image_delta,
            );
        }

        // Tessellate egui shapes into paint jobs
        let mut paint_jobs = egui_ctx.tessellate(egui_output.shapes, egui_output.pixels_per_point);

        // If requested, force all egui vertices to full opacity so UI stays solid
        if force_opaque {
            for job in paint_jobs.iter_mut() {
                match &mut job.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => {
                        for v in mesh.vertices.iter_mut() {
                            v.color[3] = 255;
                        }
                    }
                    egui::epaint::Primitive::Callback(_) => {}
                }
            }
        }

        // Update egui buffers
        self.egui_renderer.update_buffers(
            self.cell_renderer.device(),
            self.cell_renderer.queue(),
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render egui on top of the terminal content
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top of terminal
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Convert to 'static lifetime as required by egui_renderer.render()
            let mut render_pass = render_pass.forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        } // render_pass dropped here

        // Submit egui commands
        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Free egui textures
        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        Ok(())
    }
}
