use super::CellRenderer;
use anyhow::Result;

impl CellRenderer {
    pub(crate) fn load_background_image(&mut self, path: &str) -> Result<()> {
        log::info!("Loading background image from: {}", path);
        let img = image::open(path).map_err(|e| {
            log::error!("Failed to open background image '{}': {}", path, e);
            e
        })?.to_rgba8();
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
        self.bg_image_width = width;
        self.bg_image_height = height;
        self.update_bg_image_uniforms();
        Ok(())
    }

    pub(crate) fn update_bg_image_uniforms(&mut self) {
        // Shader uniform struct layout (32 bytes):
        //   image_size: vec2<f32>   @ offset 0  (8 bytes)
        //   window_size: vec2<f32>  @ offset 8  (8 bytes)
        //   mode: u32               @ offset 16 (4 bytes)
        //   opacity: f32            @ offset 20 (4 bytes)
        //   _padding: vec2<f32>     @ offset 24 (8 bytes)
        let mut data = [0u8; 32];

        // image_size (vec2<f32>)
        data[0..4].copy_from_slice(&(self.bg_image_width as f32).to_le_bytes());
        data[4..8].copy_from_slice(&(self.bg_image_height as f32).to_le_bytes());

        // window_size (vec2<f32>)
        data[8..12].copy_from_slice(&(self.config.width as f32).to_le_bytes());
        data[12..16].copy_from_slice(&(self.config.height as f32).to_le_bytes());

        // mode (u32)
        data[16..20].copy_from_slice(&(self.bg_image_mode as u32).to_le_bytes());

        // opacity (f32)
        data[20..24].copy_from_slice(&self.bg_image_opacity.to_le_bytes());

        // padding is already zeros

        self.queue.write_buffer(&self.bg_image_uniform_buffer, 0, &data);
    }

    pub fn set_background_image(
        &mut self,
        path: Option<&str>,
        mode: crate::config::BackgroundImageMode,
        opacity: f32,
    ) {
        self.bg_image_mode = mode;
        self.bg_image_opacity = opacity;
        if let Some(p) = path {
            log::info!("Loading background image: {}", p);
            if let Err(e) = self.load_background_image(p) {
                log::error!("Failed to load background image '{}': {}", p, e);
            }
        } else {
            self.bg_image_texture = None;
            self.bg_image_bind_group = None;
            self.bg_image_width = 0;
            self.bg_image_height = 0;
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
