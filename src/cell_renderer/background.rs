use super::CellRenderer;
use anyhow::Result;

impl CellRenderer {
    pub(crate) fn load_background_image(&mut self, path: &str) -> Result<()> {
        let img = image::open(path)?.to_rgba8();
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

        self.bg_image_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bg image bind group"),
                layout: &self.bg_image_bind_group_layout,
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
                        resource: self.bg_image_uniform_buffer.as_entire_binding(),
                    },
                ],
            }));
        self.bg_image_texture = Some(texture);
        self.update_bg_image_uniforms();
        Ok(())
    }

    pub(crate) fn update_bg_image_uniforms(&mut self) {
        let mut data = [0.0f32; 16];
        data[0] = self.bg_image_opacity;
        data[1] = self.window_opacity;
        data[2] = self.bg_image_mode as u32 as f32;
        data[3] = self.config.width as f32;
        data[4] = self.config.height as f32;
        self.queue.write_buffer(
            &self.bg_image_uniform_buffer,
            0,
            bytemuck::cast_slice(&data),
        );
    }

    #[allow(dead_code)]
    pub fn set_background_image(
        &mut self,
        path: Option<&str>,
        mode: crate::config::BackgroundImageMode,
        opacity: f32,
    ) {
        self.bg_image_mode = mode;
        self.bg_image_opacity = opacity;
        if let Some(p) = path {
            let _ = self.load_background_image(p);
        } else {
            self.bg_image_texture = None;
            self.bg_image_bind_group = None;
        }
        self.update_bg_image_uniforms();
    }

    #[allow(dead_code)]
    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.bg_image_opacity = opacity;
        self.update_bg_image_uniforms();
    }

    #[allow(dead_code)]
    pub fn update_background_image_opacity_only(&mut self, opacity: f32) {
        self.bg_image_opacity = opacity;
        self.update_bg_image_uniforms();
    }
}
