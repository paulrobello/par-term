//! Typed error types for par-term-render.
//!
//! This module provides structured error types so callers at the crate boundary
//! can match on specific error variants instead of relying on opaque `anyhow`
//! strings.

use thiserror::Error;

/// Top-level error type for the GPU rendering engine.
///
/// Covers the main failure categories that callers may want to distinguish:
/// - GPU initialisation (adapter, device, surface)
/// - Shader compilation and reload
/// - Image / texture loading
/// - GPU surface / presentation
/// - Screenshot capture
#[derive(Debug, Error)]
pub enum RenderError {
    // -----------------------------------------------------------------------
    // GPU initialisation
    // -----------------------------------------------------------------------
    /// A suitable wgpu GPU adapter could not be found for the given surface.
    #[error("GPU adapter not found: no compatible GPU adapter available for this surface")]
    AdapterNotFound,

    /// The wgpu device could not be created or the device was lost.
    #[error("GPU device error: {0}")]
    DeviceError(String),

    /// The wgpu surface could not be created for the window.
    #[error("GPU surface creation failed: {0}")]
    SurfaceCreation(String),

    // -----------------------------------------------------------------------
    // Shader errors
    // -----------------------------------------------------------------------
    /// The shader source file could not be read from disk.
    #[error("Shader file read failed for '{path}': {source}")]
    ShaderFileRead {
        /// Path to the shader file that could not be read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The GLSL source could not be parsed (transpilation step 1).
    #[error("GLSL parse error in '{name}':\n{details}")]
    GlslParse {
        /// Shader name or path.
        name: String,
        /// Human-readable parse error messages.
        details: String,
    },

    /// The intermediate WGSL source could not be parsed.
    #[error("WGSL parse error for '{name}': {details}")]
    WgslParse {
        /// Shader name or path.
        name: String,
        /// Human-readable parse error details.
        details: String,
    },

    /// The shader module failed naga validation.
    #[error("Shader validation failed for '{name}': {details}")]
    ShaderValidation {
        /// Shader name or path.
        name: String,
        /// Human-readable validation error details.
        details: String,
    },

    /// WGSL generation (from the naga IR) failed.
    #[error("WGSL code generation failed for '{name}': {details}")]
    WgslGeneration {
        /// Shader name or path.
        name: String,
        /// Human-readable generation error details.
        details: String,
    },

    /// A shader reload was requested but no shader is currently active,
    /// or a shader compilation error occurred during reload.
    #[error("Shader error: {0}")]
    NoActiveShader(String),

    // -----------------------------------------------------------------------
    // Image / texture loading
    // -----------------------------------------------------------------------
    /// An image file could not be opened or decoded.
    #[error("Image load failed for '{path}': {source}")]
    ImageLoad {
        /// Path to the image that failed to load.
        path: String,
        /// Underlying image error.
        #[source]
        source: image::ImageError,
    },

    /// The supplied raw RGBA byte slice has an unexpected length.
    #[error("Invalid RGBA data size: expected {expected} bytes, got {actual} bytes")]
    InvalidTextureData {
        /// Expected byte count (`width * height * 4`).
        expected: usize,
        /// Actual byte count received.
        actual: usize,
    },

    /// A cubemap face image is not square or all faces are not the same size.
    #[error("Cubemap geometry error: {0}")]
    CubemapGeometry(String),

    /// A required cubemap face file could not be found on disk.
    #[error("Cubemap face file not found: {0}")]
    CubemapFaceNotFound(String),

    // -----------------------------------------------------------------------
    // Surface / presentation
    // -----------------------------------------------------------------------
    /// `Surface::get_current_texture()` failed (timeout, outdated, lost, ...).
    #[error("GPU surface error: {0}")]
    Surface(#[from] wgpu::SurfaceError),

    // -----------------------------------------------------------------------
    // Screenshot
    // -----------------------------------------------------------------------
    /// The GPU buffer could not be mapped back to CPU memory, or a render
    /// step during screenshot capture failed.
    #[error("Screenshot capture failed: {0}")]
    ScreenshotMap(String),

    /// The pixel data could not be assembled into a final `RgbaImage`.
    #[error("Screenshot image assembly failed")]
    ScreenshotImageAssembly,
}

// ---------------------------------------------------------------------------
// Convenience conversions from common upstream error types
// ---------------------------------------------------------------------------

impl From<wgpu::CreateSurfaceError> for RenderError {
    fn from(e: wgpu::CreateSurfaceError) -> Self {
        RenderError::SurfaceCreation(e.to_string())
    }
}

impl From<wgpu::RequestDeviceError> for RenderError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        RenderError::DeviceError(e.to_string())
    }
}
