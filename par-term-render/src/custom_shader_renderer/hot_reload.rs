//! Hot-reload watcher callbacks for custom shader renderer.
//!
//! Provides the ability to reload a shader from a new GLSL source string at
//! runtime without recreating the full renderer.  This is called by the
//! file-watcher when the shader file on disk changes.

use anyhow::{Context, Result};
use wgpu::*;

use super::pipeline::create_render_pipeline;
use super::transpiler::transpile_glsl_to_wgsl_source;
use super::{CustomShaderRenderer, write_debug_shader_wgsl};

impl CustomShaderRenderer {
    /// Reload the shader from a GLSL source string.
    ///
    /// Transpiles the provided GLSL source to WGSL, validates it, and
    /// recreates the render pipeline.  The uniform buffer and all textures
    /// remain intact; only the pipeline is replaced.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `source` - GLSL shader source code
    /// * `name`   - Shader name used for diagnostic messages and WGSL debug output
    pub fn reload_from_source(&mut self, device: &Device, source: &str, name: &str) -> Result<()> {
        let control_parse = par_term_config::parse_shader_controls(source);
        for warning in &control_parse.warnings {
            log::warn!(
                "Shader control warning line {}: {}",
                warning.line,
                warning.message
            );
        }
        let custom_controls = control_parse.controls;
        let wgsl_source = transpile_glsl_to_wgsl_source(source, name)?;

        log::info!(
            "Reloading custom shader from source ({} bytes GLSL -> {} bytes WGSL)",
            source.len(),
            wgsl_source.len()
        );
        log::debug!("Generated WGSL:\n{}", wgsl_source);
        write_debug_shader_wgsl(name, &wgsl_source);

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
            label: Some("Custom Shader Module (reloaded)"),
            source: ShaderSource::Wgsl(wgsl_source.into()),
        });

        self.pipeline = create_render_pipeline(
            device,
            &shader_module,
            &self.bind_group_layout,
            self.surface_format,
            Some("Custom Shader Pipeline (reloaded)"),
        );
        self.custom_controls = custom_controls;

        log::info!("Custom shader reloaded successfully from source");
        Ok(())
    }
}
