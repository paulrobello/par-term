//! Cubemap texture loading for custom shaders
//!
//! Provides loading and management of 6-face cubemap textures for environment mapping
//! and skybox effects in custom shaders. Exposed as `iCubemap` uniform in GLSL.

use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView};
use std::path::{Path, PathBuf};
use wgpu::*;

/// Cubemap face suffixes for wgpu (order: +X, -X, +Y, -Y, +Z, -Z)
/// Note: py/ny files are swapped to correct for wgpu's Y-axis convention
const FACE_SUFFIXES: [&str; 6] = ["px", "nx", "ny", "py", "pz", "nz"];
const SUPPORTED_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "hdr"];

/// A cubemap texture that can be bound to a custom shader
pub struct CubemapTexture {
    /// The GPU texture (kept alive to ensure view/sampler remain valid)
    #[allow(dead_code)]
    pub texture: Texture,
    /// View for binding to shaders (TextureViewDimension::Cube)
    pub view: TextureView,
    /// Sampler for texture filtering
    pub sampler: Sampler,
    /// Size of each face in pixels (faces are always square)
    pub face_size: u32,
    /// Whether this is an HDR texture (uses Rgba16Float)
    #[allow(dead_code)]
    pub is_hdr: bool,
}

impl CubemapTexture {
    /// Create a 1x1 placeholder cubemap
    ///
    /// This is used when no cubemap is configured, ensuring the shader
    /// can still sample from it without errors.
    pub fn placeholder(device: &Device, queue: &Queue) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Cubemap Placeholder"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Write transparent black to all 6 faces
        for layer in 0..6u32 {
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: TextureAspect::All,
                },
                &[0u8, 0, 0, 0],
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }

        let view = texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..Default::default()
        });

        let sampler = Self::create_sampler(device);

        Self {
            texture,
            view,
            sampler,
            face_size: 1,
            is_hdr: false,
        }
    }

    /// Load cubemap from path prefix (auto-detects format)
    ///
    /// Expects 6 face files named: {prefix}-px.{ext}, -nx.{ext}, -py.{ext}, -ny.{ext}, -pz.{ext}, -nz.{ext}
    /// where {ext} is one of: png, jpg, jpeg, hdr
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue
    /// * `prefix` - Path prefix (e.g., "textures/cubemaps/env-outside")
    ///
    /// # Returns
    /// The loaded cubemap texture, or an error if loading fails
    pub fn from_prefix(device: &Device, queue: &Queue, prefix: &Path) -> Result<Self> {
        // Find face files and determine format
        let face_paths = Self::find_face_files(prefix)?;
        let is_hdr = face_paths[0]
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("hdr"));

        // Load first face to get dimensions
        let first_img = image::open(&face_paths[0])
            .with_context(|| format!("Failed to load: {}", face_paths[0].display()))?;
        let (width, height) = first_img.dimensions();

        if width != height {
            anyhow::bail!("Cubemap faces must be square, got {}x{}", width, height);
        }
        let face_size = width;

        // Choose format based on image type
        let (format, bytes_per_pixel) = if is_hdr {
            (TextureFormat::Rgba16Float, 8u32) // 4 channels * 2 bytes
        } else {
            (TextureFormat::Rgba8UnormSrgb, 4u32)
        };

        // Create 6-layer texture
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Cubemap Texture"),
            size: Extent3d {
                width: face_size,
                height: face_size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload first face
        Self::upload_face(
            queue,
            &texture,
            &first_img,
            0,
            face_size,
            bytes_per_pixel,
            is_hdr,
        )?;

        // Load and upload remaining faces
        for (layer, path) in face_paths.iter().enumerate().skip(1) {
            let img =
                image::open(path).with_context(|| format!("Failed to load: {}", path.display()))?;
            let (w, h) = img.dimensions();
            if w != face_size || h != face_size {
                anyhow::bail!(
                    "Face size mismatch: expected {}x{}, got {}x{} for {}",
                    face_size,
                    face_size,
                    w,
                    h,
                    path.display()
                );
            }
            Self::upload_face(
                queue,
                &texture,
                &img,
                layer as u32,
                face_size,
                bytes_per_pixel,
                is_hdr,
            )?;
        }

        let view = texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..Default::default()
        });

        let sampler = Self::create_sampler(device);

        log::info!(
            "Loaded {} cubemap ({}x{} per face) from {}",
            if is_hdr { "HDR" } else { "LDR" },
            face_size,
            face_size,
            prefix.display()
        );

        Ok(Self {
            texture,
            view,
            sampler,
            face_size,
            is_hdr,
        })
    }

    /// Find face files for the given prefix
    ///
    /// Searches for files matching {prefix}-{suffix}.{ext} for all supported extensions
    fn find_face_files(prefix: &Path) -> Result<[PathBuf; 6]> {
        let parent = prefix.parent().unwrap_or(Path::new("."));
        let stem = prefix.file_name().and_then(|s| s.to_str()).unwrap_or("");

        let mut paths: [Option<PathBuf>; 6] = Default::default();

        for (i, suffix) in FACE_SUFFIXES.iter().enumerate() {
            for ext in &SUPPORTED_EXTENSIONS {
                let filename = format!("{}-{}.{}", stem, suffix, ext);
                let path = parent.join(&filename);
                if path.exists() {
                    paths[i] = Some(path);
                    break;
                }
            }
            if paths[i].is_none() {
                anyhow::bail!(
                    "Missing cubemap face: {}-{}.{{png,jpg,jpeg,hdr}}",
                    prefix.display(),
                    suffix
                );
            }
        }

        Ok(paths.map(|p| p.unwrap()))
    }

    /// Upload a single face to the GPU texture
    fn upload_face(
        queue: &Queue,
        texture: &Texture,
        img: &DynamicImage,
        layer: u32,
        face_size: u32,
        bytes_per_pixel: u32,
        is_hdr: bool,
    ) -> Result<()> {
        // Flip image vertically - cubemap textures expect Y=0 at bottom,
        // but image files store Y=0 at top
        let img = img.flipv();

        let data: Vec<u8> = if is_hdr {
            // Convert to Rgba16Float (half precision)
            let rgb32f = img.to_rgb32f();
            rgb32f
                .pixels()
                .flat_map(|p| {
                    let r = half::f16::from_f32(p.0[0]);
                    let g = half::f16::from_f32(p.0[1]);
                    let b = half::f16::from_f32(p.0[2]);
                    let a = half::f16::from_f32(1.0);
                    [
                        r.to_le_bytes(),
                        g.to_le_bytes(),
                        b.to_le_bytes(),
                        a.to_le_bytes(),
                    ]
                    .into_iter()
                    .flatten()
                })
                .collect()
        } else {
            img.to_rgba8().into_raw()
        };

        queue.write_texture(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: TextureAspect::All,
            },
            &data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_pixel * face_size),
                rows_per_image: Some(face_size),
            },
            Extent3d {
                width: face_size,
                height: face_size,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    /// Create a sampler for cubemap textures
    fn create_sampler(device: &Device) -> Sampler {
        device.create_sampler(&SamplerDescriptor {
            label: Some("Cubemap Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        })
    }

    /// Get the resolution as a vec4 [size, size, 1.0, 0.0]
    ///
    /// This format matches the Shadertoy iChannelResolution style.
    pub fn resolution(&self) -> [f32; 4] {
        [self.face_size as f32, self.face_size as f32, 1.0, 0.0]
    }
}
