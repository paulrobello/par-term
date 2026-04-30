//! Shader configuration types: per-shader settings, metadata, and resolved configs.

use serde::de::Error as DeError;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ============================================================================
// Per-Shader Configuration Types
// ============================================================================

/// Metadata embedded in shader files via YAML block comments.
///
/// Parsed from `/*! par-term shader metadata ... */` blocks at the top of shader files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShaderMetadata {
    /// Human-readable name for the shader (e.g., "CRT Effect")
    pub name: Option<String>,
    /// Author of the shader
    pub author: Option<String>,
    /// Description of what the shader does
    pub description: Option<String>,
    /// Version string (e.g., "1.0.0")
    pub version: Option<String>,
    /// Default configuration values for this shader
    #[serde(default)]
    pub defaults: ShaderConfig,
    /// Optional safety/readability badges shown in settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub safety_badges: Vec<ShaderSafetyBadge>,
}

/// Readability/performance badges displayed for background shaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderSafetyBadge {
    FullContent,
    DistortsText,
    UsesTextures,
    UsesCubemap,
    HighGpuCost,
    BatteryFriendly,
}

impl ShaderSafetyBadge {
    pub fn label(self) -> &'static str {
        match self {
            Self::FullContent => "full-content",
            Self::DistortsText => "distorts text",
            Self::UsesTextures => "uses textures",
            Self::UsesCubemap => "uses cubemap",
            Self::HighGpuCost => "high GPU cost",
            Self::BatteryFriendly => "works well on battery",
        }
    }
}

/// Blend mode hint for shaders using the app background as iChannel0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShaderBackgroundBlendMode {
    Replace,
    Multiply,
    Screen,
    Overlay,
    LuminanceMask,
}

impl Default for ShaderBackgroundBlendMode {
    fn default() -> Self {
        Self::Replace
    }
}

impl ShaderBackgroundBlendMode {
    pub const ALL: [Self; 5] = [
        Self::Replace,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::LuminanceMask,
    ];

    pub fn as_uniform_int(self) -> i32 {
        match self {
            Self::Replace => 0,
            Self::Multiply => 1,
            Self::Screen => 2,
            Self::Overlay => 3,
            Self::LuminanceMask => 4,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::LuminanceMask => "Luminance mask",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ShaderColorValue(pub [f32; 4]);

impl ShaderColorValue {
    fn from_hex(value: &str) -> Result<Self, String> {
        let hex = value
            .strip_prefix('#')
            .ok_or_else(|| "color hex value must start with `#`".to_string())?;

        if hex.len() != 6 && hex.len() != 8 {
            return Err("color hex value must be `#rrggbb` or `#rrggbbaa`".to_string());
        }

        let parse_channel = |range: std::ops::Range<usize>| -> Result<f32, String> {
            u8::from_str_radix(&hex[range], 16)
                .map(|value| f32::from(value) / 255.0)
                .map_err(|_| "color hex value contains non-hex digits".to_string())
        };

        Ok(Self([
            parse_channel(0..2)?,
            parse_channel(2..4)?,
            parse_channel(4..6)?,
            if hex.len() == 8 {
                parse_channel(6..8)?
            } else {
                1.0
            },
        ]))
    }

    fn from_components(components: &[serde_yaml_ng::Value]) -> Result<Self, String> {
        if components.len() != 3 && components.len() != 4 {
            return Err("color array must have 3 or 4 normalized float components".to_string());
        }

        let mut color = [1.0_f32; 4];
        for (index, component) in components.iter().enumerate() {
            let value = match component {
                serde_yaml_ng::Value::Number(number) => number
                    .as_f64()
                    .ok_or_else(|| "color array component must be numeric".to_string())?,
                _ => return Err("color array components must be numeric".to_string()),
            };

            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(
                    "color array components must be finite normalized values in 0.0..=1.0"
                        .to_string(),
                );
            }

            color[index] = value as f32;
        }

        Ok(Self(color))
    }

    fn to_hex_string(self) -> String {
        let channels = self
            .0
            .map(|value| ((value.clamp(0.0, 1.0) * 255.0).round()) as u8);
        if channels[3] == 255 {
            format!("#{:02x}{:02x}{:02x}", channels[0], channels[1], channels[2])
        } else {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                channels[0], channels[1], channels[2], channels[3]
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderUniformValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Color(ShaderColorValue),
    Vec2([f32; 2]),
}

impl ShaderUniformValue {
    fn numeric_component(component: &serde_yaml_ng::Value, context: &str) -> Result<f32, String> {
        let value = match component {
            serde_yaml_ng::Value::Number(number) => number
                .as_f64()
                .ok_or_else(|| format!("{} component must be numeric", context))?,
            _ => return Err(format!("{} components must be numeric", context)),
        };

        if !value.is_finite() {
            return Err(format!("{} components must be finite", context));
        }

        Ok(value as f32)
    }

    fn vec2_from_components(components: &[serde_yaml_ng::Value]) -> Result<[f32; 2], String> {
        if components.len() != 2 {
            return Err("vec2 array must have exactly 2 finite numeric components".to_string());
        }

        Ok([
            Self::numeric_component(&components[0], "vec2 array")?,
            Self::numeric_component(&components[1], "vec2 array")?,
        ])
    }

    fn from_yaml_value(value: &serde_yaml_ng::Value) -> Result<Self, String> {
        match value {
            serde_yaml_ng::Value::Bool(value) => Ok(Self::Bool(*value)),
            serde_yaml_ng::Value::Number(number) => {
                let value = number
                    .as_f64()
                    .ok_or_else(|| "numeric uniform value must be numeric".to_string())?;
                if !value.is_finite() {
                    return Err("numeric uniform value must be finite".to_string());
                }

                if value.fract() == 0.0 {
                    if value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX) {
                        Ok(Self::Int(value as i32))
                    } else {
                        Err("integer uniform value must fit i32".to_string())
                    }
                } else {
                    Ok(Self::Float(value as f32))
                }
            }
            serde_yaml_ng::Value::String(value) if value.starts_with('#') => {
                ShaderColorValue::from_hex(value).map(Self::Color)
            }
            serde_yaml_ng::Value::String(_) => {
                Err("string uniform values are only supported for color hex strings".to_string())
            }
            serde_yaml_ng::Value::Sequence(components) if components.len() == 2 => {
                Self::vec2_from_components(components).map(Self::Vec2)
            }
            serde_yaml_ng::Value::Sequence(components)
                if components.len() == 3 || components.len() == 4 =>
            {
                ShaderColorValue::from_components(components).map(Self::Color)
            }
            serde_yaml_ng::Value::Sequence(_) => {
                Err("shader uniform arrays must have length 2 (vec2) or 3/4 (color)".to_string())
            }
            _ => Err("unsupported shader uniform value type".to_string()),
        }
    }
}

impl Serialize for ShaderUniformValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Float(value) => serializer.serialize_f32(*value),
            Self::Int(value) => serializer.serialize_i32(*value),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Color(value) => serializer.serialize_str(&value.to_hex_string()),
            Self::Vec2(value) => {
                let mut sequence = serializer.serialize_seq(Some(2))?;
                sequence.serialize_element(&value[0])?;
                sequence.serialize_element(&value[1])?;
                sequence.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ShaderUniformValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_yaml_ng::Value::deserialize(deserializer)?;
        Self::from_yaml_value(&value).map_err(D::Error::custom)
    }
}

fn deserialize_uniforms<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, ShaderUniformValue>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = BTreeMap::<String, serde_yaml_ng::Value>::deserialize(deserializer)?;
    let mut uniforms = BTreeMap::new();

    for (name, value) in raw {
        match ShaderUniformValue::from_yaml_value(&value) {
            Ok(value) => {
                uniforms.insert(name, value);
            }
            Err(error) => {
                log::warn!(
                    "Skipping invalid shader uniform default `{}`: {}",
                    name,
                    error
                );
            }
        }
    }

    Ok(uniforms)
}

/// Per-shader configuration settings.
///
/// Used both for embedded defaults in shader files and for user overrides in config.yaml.
/// All fields are optional to allow partial overrides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShaderConfig {
    /// Animation speed multiplier (1.0 = normal speed)
    pub animation_speed: Option<f32>,
    /// Brightness multiplier (0.05-1.0)
    pub brightness: Option<f32>,
    /// Text opacity when using this shader (0.0-1.0)
    pub text_opacity: Option<f32>,
    /// When true, shader receives full terminal content for manipulation
    pub full_content: Option<bool>,
    /// Path to texture for iChannel0
    pub channel0: Option<String>,
    /// Path to texture for iChannel1
    pub channel1: Option<String>,
    /// Path to texture for iChannel2
    pub channel2: Option<String>,
    /// Path to texture for iChannel3
    pub channel3: Option<String>,
    /// Path prefix for cubemap faces
    pub cubemap: Option<String>,
    /// Whether cubemap sampling is enabled
    pub cubemap_enabled: Option<bool>,
    /// Use the app's background image as iChannel0 instead of a separate texture
    pub use_background_as_channel0: Option<bool>,
    /// Blend mode hint for shaders using the app background as iChannel0.
    pub background_channel0_blend_mode: Option<ShaderBackgroundBlendMode>,
    /// Auto-dim shader output beneath terminal text/content for readability.
    pub auto_dim_under_text: Option<bool>,
    /// Strength of auto-dimming under text (0.0 = no extra dimming, 1.0 = black).
    pub auto_dim_strength: Option<f32>,
    /// Custom shader uniform values for `// control ...` declarations.
    #[serde(
        default,
        deserialize_with = "deserialize_uniforms",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub uniforms: BTreeMap<String, ShaderUniformValue>,
}

/// Cursor shader specific configuration.
///
/// Extends base ShaderConfig with cursor-specific settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CursorShaderConfig {
    /// Base shader configuration
    #[serde(flatten)]
    pub base: ShaderConfig,
    /// Hide the default cursor when this shader is enabled
    pub hides_cursor: Option<bool>,
    /// Disable cursor shader while in alt screen (vim, less, htop)
    pub disable_in_alt_screen: Option<bool>,
    /// Cursor glow radius in pixels
    pub glow_radius: Option<f32>,
    /// Cursor glow intensity (0.0-1.0)
    pub glow_intensity: Option<f32>,
    /// Duration of cursor trail effect in seconds
    pub trail_duration: Option<f32>,
    /// Cursor color for shader effects [R, G, B] (0-255)
    pub cursor_color: Option<[u8; 3]>,
}

/// Metadata embedded in cursor shader files via YAML block comments.
///
/// Parsed from `/*! par-term shader metadata ... */` blocks at the top of cursor shader files.
/// Similar to `ShaderMetadata` but with cursor-specific defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CursorShaderMetadata {
    /// Human-readable name for the shader (e.g., "Cursor Glow Effect")
    pub name: Option<String>,
    /// Author of the shader
    pub author: Option<String>,
    /// Description of what the shader does
    pub description: Option<String>,
    /// Version string (e.g., "1.0.0")
    pub version: Option<String>,
    /// Default configuration values for this cursor shader
    #[serde(default)]
    pub defaults: CursorShaderConfig,
}

/// Fully resolved shader configuration with all values filled in.
///
/// Created by merging user overrides, shader metadata defaults, and global defaults.
#[derive(Debug, Clone)]
pub struct ResolvedShaderConfig {
    /// Animation speed multiplier
    pub animation_speed: f32,
    /// Brightness multiplier
    pub brightness: f32,
    /// Text opacity
    pub text_opacity: f32,
    /// Full content mode enabled
    pub full_content: bool,
    /// Resolved path to iChannel0 texture
    pub channel0: Option<PathBuf>,
    /// Resolved path to iChannel1 texture
    pub channel1: Option<PathBuf>,
    /// Resolved path to iChannel2 texture
    pub channel2: Option<PathBuf>,
    /// Resolved path to iChannel3 texture
    pub channel3: Option<PathBuf>,
    /// Resolved cubemap path prefix
    pub cubemap: Option<PathBuf>,
    /// Cubemap sampling enabled
    pub cubemap_enabled: bool,
    /// Use the app's background image as iChannel0
    pub use_background_as_channel0: bool,
    /// Blend mode hint for shaders using the app background as iChannel0.
    pub background_channel0_blend_mode: ShaderBackgroundBlendMode,
    /// Auto-dim shader output beneath terminal text/content for readability.
    pub auto_dim_under_text: bool,
    /// Strength of auto-dimming under text.
    pub auto_dim_strength: f32,
    /// Custom shader uniform values resolved from metadata defaults and user overrides.
    pub custom_uniforms: BTreeMap<String, ShaderUniformValue>,
}

impl Default for ResolvedShaderConfig {
    fn default() -> Self {
        Self {
            animation_speed: 1.0,
            brightness: 1.0,
            text_opacity: 1.0,
            full_content: false,
            channel0: None,
            channel1: None,
            channel2: None,
            channel3: None,
            cubemap: None,
            cubemap_enabled: true,
            use_background_as_channel0: false,
            background_channel0_blend_mode: ShaderBackgroundBlendMode::Replace,
            auto_dim_under_text: false,
            auto_dim_strength: 0.35,
            custom_uniforms: BTreeMap::new(),
        }
    }
}

/// Fully resolved cursor shader configuration with all values filled in.
#[derive(Debug, Clone)]
pub struct ResolvedCursorShaderConfig {
    /// Base resolved shader config
    pub base: ResolvedShaderConfig,
    /// Hide the default cursor when this shader is enabled
    pub hides_cursor: bool,
    /// Disable cursor shader while in alt screen (vim, less, htop)
    pub disable_in_alt_screen: bool,
    /// Cursor glow radius in pixels
    pub glow_radius: f32,
    /// Cursor glow intensity (0.0-1.0)
    pub glow_intensity: f32,
    /// Duration of cursor trail effect in seconds
    pub trail_duration: f32,
    /// Cursor color for shader effects [R, G, B] (0-255)
    pub cursor_color: [u8; 3],
}

impl Default for ResolvedCursorShaderConfig {
    fn default() -> Self {
        Self {
            base: ResolvedShaderConfig::default(),
            hides_cursor: false,
            disable_in_alt_screen: true,
            glow_radius: 80.0,
            glow_intensity: 0.3,
            trail_duration: 0.5,
            cursor_color: [255, 255, 255],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_uniform_values_parse_int_float_and_vec2_defaults() {
        let yaml = r#"
uniforms:
  iCount: 4
  iGlow: 0.75
  iOrigin: [0.25, 0.5]
  iTint: [1.0, 0.5, 0.0]
  iEnabled: true
  iBadVec2: [0.0, .inf]
  iBadString: not-a-color
"#;

        let config: ShaderConfig =
            serde_yaml_ng::from_str(yaml).expect("deserialize shader config");

        assert_eq!(
            config.uniforms.get("iCount"),
            Some(&ShaderUniformValue::Int(4))
        );
        assert_eq!(
            config.uniforms.get("iGlow"),
            Some(&ShaderUniformValue::Float(0.75))
        );
        assert_eq!(
            config.uniforms.get("iOrigin"),
            Some(&ShaderUniformValue::Vec2([0.25, 0.5]))
        );
        assert_eq!(
            config.uniforms.get("iTint"),
            Some(&ShaderUniformValue::Color(ShaderColorValue([
                1.0, 0.5, 0.0, 1.0,
            ])))
        );
        assert_eq!(
            config.uniforms.get("iEnabled"),
            Some(&ShaderUniformValue::Bool(true))
        );
        assert!(!config.uniforms.contains_key("iBadVec2"));
        assert!(!config.uniforms.contains_key("iBadString"));

        let serialized = serde_yaml_ng::to_string(&config).expect("serialize shader config");
        assert!(serialized.contains("iCount: 4"));
        assert!(serialized.contains("iOrigin:\n"));
        assert!(serialized.contains("- 0.25"));
        assert!(serialized.contains("- 0.5"));
    }
}
