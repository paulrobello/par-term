/// Uniform data passed to custom shaders
/// Layout must match GLSL std140 rules:
/// - vec2 aligned to 8 bytes
/// - vec4 aligned to 16 bytes
/// - float aligned to 4 bytes
/// - struct size rounded to 16 bytes (largest alignment)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CustomShaderUniforms {
    /// Viewport resolution (iResolution.xy) - offset 0, size 8
    pub resolution: [f32; 2],
    /// Time in seconds since shader started (iTime) - offset 8, size 4
    pub time: f32,
    /// Time since last frame in seconds (iTimeDelta) - offset 12, size 4
    pub time_delta: f32,
    /// Mouse state (iMouse) - offset 16, size 16
    /// xy = current position (if dragging) or last drag position
    /// zw = click position (positive when held, negative when released)
    pub mouse: [f32; 4],
    /// Date/time (iDate) - offset 32, size 16
    /// x = year, y = month (0-11), z = day (1-31), w = seconds since midnight
    pub date: [f32; 4],
    /// Window opacity for transparency support - offset 48, size 4
    pub opacity: f32,
    /// Text opacity (separate from window opacity) - offset 52, size 4
    pub text_opacity: f32,
    /// Full content mode: 1.0 = shader receives and outputs full content, 0.0 = background only
    pub full_content_mode: f32,
    /// Frame counter (iFrame) - offset 60, size 4
    pub frame: f32,
    /// Current frame rate in FPS (iFrameRate) - offset 64, size 4
    pub frame_rate: f32,
    /// Pixel aspect ratio (iResolution.z) - offset 68, size 4, usually 1.0
    pub resolution_z: f32,
    /// Brightness multiplier for shader output (0.05-1.0) - offset 72, size 4
    pub brightness: f32,
    /// Time when last key was pressed (same timebase as iTime) - offset 76, size 4
    pub key_press_time: f32,

    // ============ Cursor uniforms (Ghostty-compatible, v1.2.0+) ============
    // Offsets 80-159
    /// Current cursor position (xy) and size (zw) in pixels - offset 80, size 16
    pub current_cursor: [f32; 4],
    /// Previous cursor position (xy) and size (zw) in pixels - offset 96, size 16
    pub previous_cursor: [f32; 4],
    /// Current cursor RGBA color (with opacity baked into alpha) - offset 112, size 16
    pub current_cursor_color: [f32; 4],
    /// Previous cursor RGBA color - offset 128, size 16
    pub previous_cursor_color: [f32; 4],
    /// Time when cursor last moved (same timebase as iTime) - offset 144, size 4
    pub cursor_change_time: f32,

    // ============ Cursor shader configuration uniforms ============
    // Offsets 148-175
    /// Cursor trail duration in seconds - offset 148, size 4
    pub cursor_trail_duration: f32,
    /// Cursor glow radius in pixels - offset 152, size 4
    pub cursor_glow_radius: f32,
    /// Cursor glow intensity (0.0-1.0) - offset 156, size 4
    pub cursor_glow_intensity: f32,
    /// User-configured cursor color for shader effects [R, G, B, 1.0] - offset 160, size 16
    /// (placed last because vec4 must be aligned to 16 bytes in std140)
    pub cursor_shader_color: [f32; 4],

    // ============ Channel resolution uniforms (Shadertoy-compatible) ============
    // Offsets 176-255
    /// Channel 0 resolution (terminal texture) [width, height, 1.0, 0.0] - offset 176, size 16
    pub channel0_resolution: [f32; 4],
    /// Channel 1 resolution [width, height, 1.0, 0.0] - offset 192, size 16
    pub channel1_resolution: [f32; 4],
    /// Channel 2 resolution [width, height, 1.0, 0.0] - offset 208, size 16
    pub channel2_resolution: [f32; 4],
    /// Channel 3 resolution [width, height, 1.0, 0.0] - offset 224, size 16
    pub channel3_resolution: [f32; 4],
    /// Channel 4 resolution [width, height, 1.0, 0.0] - offset 240, size 16
    pub channel4_resolution: [f32; 4],
    /// Cubemap resolution [size, size, 1.0, 0.0] - offset 256, size 16
    pub cubemap_resolution: [f32; 4],

    // ============ Background color uniform ============
    /// Solid background color [R, G, B, A] - offset 272, size 16
    /// When A > 0, this color is used as the background instead of shader output.
    /// RGB values are NOT premultiplied. Alpha indicates solid color mode is active.
    pub background_color: [f32; 4],

    // ============ Progress bar uniform ============
    /// Progress bar state [state, percent, isActive, activeCount] - offset 288, size 16
    /// x = state of simple progress bar (0=hidden, 1=normal, 2=error, 3=indeterminate, 4=warning)
    /// y = percent as 0.0-1.0 (from simple bar's 0-100)
    /// z = 1.0 if any progress bar is active, 0.0 otherwise
    /// w = total count of active bars (simple + named)
    pub progress: [f32; 4],
}
// Total size: 304 bytes

pub(crate) const MAX_CUSTOM_FLOAT_UNIFORMS: usize = 16;
pub(crate) const MAX_CUSTOM_BOOL_UNIFORMS: usize = 16;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CustomShaderControlUniforms {
    /// 16 float slots stored as 4 vec4s for std140 array alignment.
    pub float_values: [[f32; 4]; 4],
    /// 16 bool slots stored as 4 uvec4s/ivec4s for std140 array alignment.
    pub bool_values: [[u32; 4]; 4],
}

impl CustomShaderControlUniforms {
    pub(crate) fn from_controls(
        controls: &[par_term_config::ShaderControl],
        values: &std::collections::BTreeMap<String, par_term_config::ShaderUniformValue>,
    ) -> Self {
        let mut uniforms = Self {
            float_values: [[0.0; 4]; 4],
            bool_values: [[0; 4]; 4],
        };
        let mut float_index = 0usize;
        let mut bool_index = 0usize;

        for control in controls {
            match control.kind {
                par_term_config::ShaderControlKind::Slider { min, max, .. } => {
                    if float_index >= MAX_CUSTOM_FLOAT_UNIFORMS {
                        continue;
                    }
                    let value = match values.get(&control.name) {
                        Some(par_term_config::ShaderUniformValue::Float(value)) => *value,
                        _ => min,
                    }
                    .clamp(min, max);
                    uniforms.float_values[float_index / 4][float_index % 4] = value;
                    float_index += 1;
                }
                par_term_config::ShaderControlKind::Checkbox => {
                    if bool_index >= MAX_CUSTOM_BOOL_UNIFORMS {
                        continue;
                    }
                    let value = matches!(
                        values.get(&control.name),
                        Some(par_term_config::ShaderUniformValue::Bool(true))
                    );
                    uniforms.bool_values[bool_index / 4][bool_index % 4] = u32::from(value);
                    bool_index += 1;
                }
            }
        }

        uniforms
    }
}

const _: () = assert!(
    std::mem::size_of::<CustomShaderControlUniforms>() == 128,
    "CustomShaderControlUniforms must be exactly 128 bytes"
);

// Compile-time assertion to ensure uniform struct size matches expectations
const _: () = assert!(
    std::mem::size_of::<CustomShaderUniforms>() == 304,
    "CustomShaderUniforms must be exactly 304 bytes for GPU compatibility"
);

#[cfg(test)]
mod custom_uniform_tests {
    use super::*;

    #[test]
    fn custom_shader_control_uniforms_are_vec4_aligned() {
        assert_eq!(std::mem::size_of::<CustomShaderControlUniforms>(), 128);
    }

    #[test]
    fn builds_control_uniforms_with_clamped_slider_and_bool_slots() {
        use par_term_config::{ShaderControl, ShaderControlKind, ShaderUniformValue};
        use std::collections::BTreeMap;

        let controls = vec![
            ShaderControl {
                name: "iGlow".to_string(),
                kind: ShaderControlKind::Slider {
                    min: 0.0,
                    max: 1.0,
                    step: 0.1,
                },
            },
            ShaderControl {
                name: "iEnabled".to_string(),
                kind: ShaderControlKind::Checkbox,
            },
        ];
        let values = BTreeMap::from([
            ("iGlow".to_string(), ShaderUniformValue::Float(2.0)),
            ("iEnabled".to_string(), ShaderUniformValue::Bool(true)),
        ]);

        let uniforms = CustomShaderControlUniforms::from_controls(&controls, &values);

        assert_eq!(uniforms.float_values[0][0], 1.0);
        assert_eq!(uniforms.bool_values[0][0], 1);
    }
}
