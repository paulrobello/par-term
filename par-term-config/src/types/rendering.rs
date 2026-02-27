//! Rendering-related configuration types: GPU, display modes, pane/divider layout.

use serde::{Deserialize, Serialize};

// ============================================================================
// GPU / VSync / Power Types
// ============================================================================

/// VSync mode (presentation mode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VsyncMode {
    /// No VSync - render as fast as possible (lowest latency, highest GPU usage)
    Immediate,
    /// Mailbox VSync - cap at monitor refresh rate with triple buffering (balanced)
    Mailbox,
    /// FIFO VSync - strict vsync with double buffering (lowest GPU usage, most compatible)
    #[default]
    Fifo,
}

impl VsyncMode {
    /// Convert to wgpu::PresentMode
    #[cfg(feature = "wgpu-types")]
    pub fn to_present_mode(self) -> wgpu::PresentMode {
        match self {
            VsyncMode::Immediate => wgpu::PresentMode::Immediate,
            VsyncMode::Mailbox => wgpu::PresentMode::Mailbox,
            VsyncMode::Fifo => wgpu::PresentMode::Fifo,
        }
    }
}

/// GPU power preference for adapter selection
///
/// Controls which GPU adapter is preferred when multiple GPUs are available
/// (e.g., integrated Intel GPU vs discrete NVIDIA/AMD GPU).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PowerPreference {
    /// No preference - let the system decide (default)
    #[default]
    None,
    /// Prefer integrated GPU (Intel/AMD iGPU) - saves battery
    LowPower,
    /// Prefer discrete GPU (NVIDIA/AMD) - maximum performance
    HighPerformance,
}

impl PowerPreference {
    /// Convert to wgpu::PowerPreference
    #[cfg(feature = "wgpu-types")]
    pub fn to_wgpu(self) -> wgpu::PowerPreference {
        match self {
            PowerPreference::None => wgpu::PowerPreference::None,
            PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
        }
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            PowerPreference::None => "None (System Default)",
            PowerPreference::LowPower => "Low Power (Integrated GPU)",
            PowerPreference::HighPerformance => "High Performance (Discrete GPU)",
        }
    }

    /// All available power preferences for UI iteration
    pub fn all() -> &'static [PowerPreference] {
        &[
            PowerPreference::None,
            PowerPreference::LowPower,
            PowerPreference::HighPerformance,
        ]
    }
}

// ============================================================================
// Image / Background Types
// ============================================================================

/// Image scaling quality for inline graphics (Sixel, iTerm2, Kitty)
///
/// Controls the GPU texture sampling filter used when scaling inline images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageScalingMode {
    /// Nearest-neighbor filtering - sharp pixels, good for pixel art
    Nearest,
    /// Bilinear filtering - smooth scaling (default)
    #[default]
    Linear,
}

impl ImageScalingMode {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ImageScalingMode::Nearest => "Nearest (Sharp)",
            ImageScalingMode::Linear => "Linear (Smooth)",
        }
    }

    /// All available modes for UI iteration
    pub fn all() -> &'static [ImageScalingMode] {
        &[ImageScalingMode::Nearest, ImageScalingMode::Linear]
    }

    /// Convert to wgpu FilterMode
    #[cfg(feature = "wgpu-types")]
    pub fn to_filter_mode(self) -> wgpu::FilterMode {
        match self {
            ImageScalingMode::Nearest => wgpu::FilterMode::Nearest,
            ImageScalingMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

/// Background image display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundImageMode {
    /// Scale to fit window while maintaining aspect ratio (may have letterboxing)
    Fit,
    /// Scale to fill window while maintaining aspect ratio (may crop edges)
    Fill,
    /// Stretch to fill window exactly (ignores aspect ratio)
    #[default]
    Stretch,
    /// Repeat image in a tiled pattern at original size
    Tile,
    /// Center image at original size (no scaling)
    Center,
}

/// Background source selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundMode {
    /// Use theme's default background color
    #[default]
    Default,
    /// Use a custom solid color
    Color,
    /// Use a background image
    Image,
}

/// Per-pane background image configuration (for config persistence)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneBackgroundConfig {
    /// Pane index (0-based)
    pub index: usize,
    /// Image path
    pub image: String,
    /// Display mode
    #[serde(default)]
    pub mode: BackgroundImageMode,
    /// Opacity
    #[serde(default = "crate::defaults::background_image_opacity")]
    pub opacity: f32,
    /// Darken amount (0.0 = no darkening, 1.0 = fully black)
    #[serde(default = "crate::defaults::pane_background_darken")]
    pub darken: f32,
}

/// Per-pane background image configuration (runtime state)
#[derive(Debug, Clone, Default)]
pub struct PaneBackground {
    /// Path to the background image (None = use global background)
    pub image_path: Option<String>,
    /// Display mode (fit/fill/stretch/tile/center)
    pub mode: BackgroundImageMode,
    /// Opacity (0.0-1.0)
    pub opacity: f32,
    /// Darken amount (0.0 = no darkening, 1.0 = fully black)
    pub darken: f32,
}

impl PaneBackground {
    /// Create a new PaneBackground with default settings
    pub fn new() -> Self {
        Self {
            image_path: None,
            mode: BackgroundImageMode::default(),
            opacity: 1.0,
            darken: 0.0,
        }
    }

    /// Returns true if this pane has a custom background image set
    pub fn has_image(&self) -> bool {
        self.image_path.is_some()
    }
}

// ============================================================================
// Pane / Divider Layout Types
// ============================================================================

/// Position of pane title bars
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaneTitlePosition {
    /// Title bar at the top of the pane (default)
    #[default]
    Top,
    /// Title bar at the bottom of the pane
    Bottom,
}

impl PaneTitlePosition {
    /// All available positions for UI dropdowns
    pub const ALL: &'static [PaneTitlePosition] =
        &[PaneTitlePosition::Top, PaneTitlePosition::Bottom];

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            PaneTitlePosition::Top => "Top",
            PaneTitlePosition::Bottom => "Bottom",
        }
    }
}

/// Style of dividers between panes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DividerStyle {
    /// Solid line (default)
    #[default]
    Solid,
    /// Double line effect (two thin lines with gap)
    Double,
    /// Dashed line effect
    Dashed,
    /// Shadow effect (gradient fade)
    Shadow,
}

impl DividerStyle {
    /// All available styles for UI dropdowns
    pub const ALL: &'static [DividerStyle] = &[
        DividerStyle::Solid,
        DividerStyle::Double,
        DividerStyle::Dashed,
        DividerStyle::Shadow,
    ];

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            DividerStyle::Solid => "Solid",
            DividerStyle::Double => "Double",
            DividerStyle::Dashed => "Dashed",
            DividerStyle::Shadow => "Shadow",
        }
    }
}

/// A divider rectangle between panes
#[derive(Debug, Clone, Copy)]
pub struct DividerRect {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
    /// Whether this is a horizontal divider (vertical line)
    pub is_horizontal: bool,
}

impl DividerRect {
    /// Create a new divider rect
    pub fn new(x: f32, y: f32, width: f32, height: f32, is_horizontal: bool) -> Self {
        Self {
            x,
            y,
            width,
            height,
            is_horizontal,
        }
    }

    /// Check if a point is inside the divider (with optional padding for easier grabbing)
    pub fn contains(&self, px: f32, py: f32, padding: f32) -> bool {
        px >= self.x - padding
            && px < self.x + self.width + padding
            && py >= self.y - padding
            && py < self.y + self.height + padding
    }
}

// ============================================================================
// Shared ID and Mark Types
// ============================================================================

/// Visible command separator mark: (row, col_offset, optional_color)
pub type SeparatorMark = (usize, Option<i32>, Option<(u8, u8, u8)>);

/// Unique identifier for a pane
pub type PaneId = u64;

/// Unique identifier for a tab
pub type TabId = u64;
