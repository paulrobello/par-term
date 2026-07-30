// ARC-009: `Renderer` is split across sibling modules under `renderer/`. This
// file holds the struct definition, its re-exports, and `Renderer::new`.
//
//   types.rs      — Pane/divider/title render-input types and separator-mark mapping
//   resize.rs     — Surface resize and DPI scale-factor changes
//   layout.rs     — Grid geometry, padding, and content offset/inset accessors
//   screenshot.rs — Offscreen capture: `take_screenshot` and its shader chain
//
// The remaining behaviour lives in rendering.rs, render_passes.rs, egui_render.rs,
// graphics.rs, shaders/, params.rs, and state.rs.
//
// Tracking: Issue ARC-009 in AUDIT.md.

use crate::cell_renderer::{CellRenderer, CellRendererConfig};
use crate::custom_shader_renderer::CustomShaderRenderer;
use crate::graphics_renderer::GraphicsRenderer;
use anyhow::Result;
use winit::dpi::PhysicalSize;

mod egui_render;
pub mod graphics;
mod layout;
pub mod params;

mod render_passes;
mod rendering;
mod resize;
mod screenshot;
pub mod shaders;
mod state;
mod types;

// Re-export SeparatorMark from par-term-config
pub use par_term_config::SeparatorMark;
pub use params::RendererParams;
pub use rendering::SplitPanesRenderParams;
pub use types::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo,
    compute_visible_separator_marks,
};
// Intentionally narrower than the `pub use` above: this helper is a crate-internal
// hot-path variant and must not become part of the published API surface.
pub(crate) use types::fill_visible_separator_marks;

/// Renderer for the terminal using custom wgpu cell renderer
pub struct Renderer {
    // Cell renderer (owns the scrollbar)
    pub(crate) cell_renderer: CellRenderer,

    // Graphics renderer for sixel images
    pub(crate) graphics_renderer: GraphicsRenderer,

    // Current sixel graphics to render.
    // Note: screen_row is isize to allow negative values for graphics scrolled off top
    pub(crate) sixel_graphics: Vec<crate::graphics_renderer::GraphicRenderInfo>,

    // egui renderer for settings UI
    pub(crate) egui_renderer: egui_wgpu::Renderer,

    // Custom shader renderer for post-processing effects (background shader)
    pub(crate) custom_shader_renderer: Option<CustomShaderRenderer>,
    // Track current shader path to detect changes
    pub(crate) custom_shader_path: Option<String>,
    // Background shader failure from construction, awaiting collection by the
    // frontend's shader-error sink. Read once via `take_startup_shader_errors`.
    pub(crate) startup_shader_error: Option<String>,

    // Cursor shader renderer for cursor-specific effects (separate from background shader)
    pub(crate) cursor_shader_renderer: Option<CustomShaderRenderer>,
    // Track current cursor shader path to detect changes
    pub(crate) cursor_shader_path: Option<String>,
    // Cursor shader failure from construction; see `startup_shader_error`.
    pub(crate) startup_cursor_shader_error: Option<String>,

    // Cached for convenience
    pub(crate) size: PhysicalSize<u32>,

    // Dirty flag for optimization - only render when content has changed
    pub(crate) dirty: bool,

    // Cached scrollbar state to avoid redundant GPU uploads.
    // Includes scroll position, line counts, marks, window size, AND pane viewport
    // bounds so that pane splits/resizes correctly trigger a scrollbar geometry update.
    pub(crate) last_scrollbar_state: (usize, usize, usize, usize, u32, u32, u32, u32, u32, u32),

    // Skip cursor shader when alt screen is active (TUI apps like vim, htop)
    pub(crate) cursor_shader_disabled_for_alt_screen: bool,

    // Debug overlay text
    pub(crate) debug_text: Option<String>,

    // Scratch buffer for divider instances, reused each frame to avoid
    // per-call heap allocations in `render_dividers`.
    pub(crate) scratch_divider_instances: Vec<crate::cell_renderer::BackgroundInstance>,
}

impl Renderer {
    /// Create a new renderer
    pub async fn new(params: RendererParams<'_>) -> Result<Self> {
        let window = params.window;
        let font_family = params.font_family;
        let font_family_bold = params.font_family_bold;
        let font_family_italic = params.font_family_italic;
        let font_family_bold_italic = params.font_family_bold_italic;
        let font_ranges = params.font_ranges;
        let font_size = params.font_size;
        let line_spacing = params.line_spacing;
        let char_spacing = params.char_spacing;
        let scrollbar_position = params.scrollbar_position;
        let scrollbar_thumb_color = params.scrollbar_thumb_color;
        let scrollbar_track_color = params.scrollbar_track_color;
        let enable_text_shaping = params.enable_text_shaping;
        let enable_ligatures = params.enable_ligatures;
        let enable_kerning = params.enable_kerning;
        let font_antialias = params.font_antialias;
        let font_hinting = params.font_hinting;
        let font_thin_strokes = params.font_thin_strokes;
        let minimum_contrast = params.minimum_contrast;
        let vsync_mode = params.vsync_mode;
        let power_preference = params.power_preference;
        let window_opacity = params.window_opacity;
        let background_color = params.background_color;
        let background_image_path = params.background_image_path;
        let background_image_enabled = params.background_image_enabled;
        let background_image_mode = params.background_image_mode;
        let background_image_opacity = params.background_image_opacity;
        let custom_shader_path = params.custom_shader_path;
        let custom_shader_enabled = params.custom_shader_enabled;
        let custom_shader_animation = params.custom_shader_animation;
        let custom_shader_animation_speed = params.custom_shader_animation_speed;
        let custom_shader_full_content = params.custom_shader_full_content;
        let custom_shader_brightness = params.custom_shader_brightness;
        let custom_shader_channel_paths = params.custom_shader_channel_paths;
        let custom_shader_cubemap_path = params.custom_shader_cubemap_path;
        let custom_shader_custom_uniforms = params.custom_shader_custom_uniforms;
        let use_background_as_channel0 = params.use_background_as_channel0;
        let background_channel0_blend_mode = params.background_channel0_blend_mode;
        let custom_shader_auto_dim_under_text = params.custom_shader_auto_dim_under_text;
        let custom_shader_auto_dim_strength = params.custom_shader_auto_dim_strength;
        let image_scaling_mode = params.image_scaling_mode;
        let image_preserve_aspect_ratio = params.image_preserve_aspect_ratio;
        let cursor_shader_path = params.cursor_shader_path;
        let cursor_shader_enabled = params.cursor_shader_enabled;
        let cursor_shader_animation = params.cursor_shader_animation;
        let cursor_shader_animation_speed = params.cursor_shader_animation_speed;

        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        // Standard DPI for the platform
        // macOS typically uses 72 DPI for points, Windows and most Linux use 96 DPI
        let platform_dpi = if cfg!(target_os = "macos") {
            72.0
        } else {
            96.0
        };

        // Convert font size from points to pixels for cell size calculation, honoring DPI and scale
        let base_font_pixels = font_size * platform_dpi / 72.0;
        let font_size_pixels = (base_font_pixels * scale_factor as f32).max(1.0);

        // Font lookup for the metrics that determine `cols`/`rows` below.  The
        // manager is then handed to `CellRenderer::new`; building a second one
        // there would re-enumerate every system font on each renderer rebuild.
        let font_manager = par_term_fonts::font_manager::FontManager::new(
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
        )?;

        let (font_ascent, font_descent, font_leading, char_advance) = {
            let primary_font = font_manager
                .get_font(0)
                .expect("Primary font at index 0 must exist after FontManager initialization");
            let metrics = primary_font.metrics(&[]);
            let scale = font_size_pixels / metrics.units_per_em as f32;

            // Get advance width of a standard character ('m' is common for monospace width)
            let glyph_id = primary_font.charmap().map('m');
            let advance = primary_font.glyph_metrics(&[]).advance_width(glyph_id) * scale;

            (
                metrics.ascent * scale,
                metrics.descent * scale,
                metrics.leading * scale,
                advance,
            )
        };

        // Use font metrics for cell height if line_spacing is 1.0
        // Natural line height = ascent + descent + leading
        let natural_line_height = font_ascent + font_descent + font_leading;
        let char_height = (natural_line_height * line_spacing).max(1.0).round();

        // Scale logical pixel values (config) to physical pixels (wgpu surface)
        let scale = scale_factor as f32;
        let window_padding = params.window_padding * scale;
        let scrollbar_width = params.scrollbar_width * scale;

        // Calculate available space after padding and scrollbar
        let available_width = (size.width as f32 - window_padding * 2.0 - scrollbar_width).max(0.0);
        let available_height = (size.height as f32 - window_padding * 2.0).max(0.0);

        // Calculate terminal dimensions based on font size in pixels and spacing
        let char_width = (char_advance * char_spacing).max(1.0).round(); // Configurable character width (rounded to integer pixels)
        let cols = (available_width / char_width).max(1.0) as usize;
        let rows = (available_height / char_height).max(1.0) as usize;

        // Create cell renderer with font fallback support (owns scrollbar)
        let bg_path = if background_image_enabled {
            background_image_path
        } else {
            None
        };
        log::info!(
            "Renderer::new: background_image_enabled={}, path={:?}",
            background_image_enabled,
            bg_path
        );
        let cell_renderer = CellRenderer::new(
            window.clone(),
            CellRendererConfig {
                font_manager,
                font_size,
                cols,
                rows,
                window_padding,
                line_spacing,
                char_spacing,
                scrollbar_position,
                scrollbar_width,
                scrollbar_thumb_color,
                scrollbar_track_color,
                enable_text_shaping,
                enable_ligatures,
                enable_kerning,
                font_antialias,
                font_hinting,
                font_thin_strokes,
                minimum_contrast,
                vsync_mode,
                power_preference,
                window_opacity,
                background_color,
                background_image_path: bg_path,
                background_image_mode,
                background_image_opacity,
            },
        )
        .await?;

        // Create egui renderer for settings UI
        let egui_renderer = egui_wgpu::Renderer::new(
            cell_renderer.device(),
            cell_renderer.surface_format(),
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );

        // Create graphics renderer for sixel images
        let graphics_renderer = GraphicsRenderer::new(
            cell_renderer.device(),
            cell_renderer.surface_format(),
            cell_renderer.cell_width(),
            cell_renderer.cell_height(),
            cell_renderer.window_padding(),
            image_scaling_mode,
            image_preserve_aspect_ratio,
        )?;

        // Create custom shader renderer if configured
        let (mut custom_shader_renderer, initial_shader_path, startup_shader_error) =
            shaders::init_custom_shader(
                &cell_renderer,
                shaders::CustomShaderInitParams {
                    size_width: size.width,
                    size_height: size.height,
                    window_padding,
                    path: custom_shader_path,
                    enabled: custom_shader_enabled,
                    animation: custom_shader_animation,
                    animation_speed: custom_shader_animation_speed,
                    window_opacity,
                    full_content: custom_shader_full_content,
                    brightness: custom_shader_brightness,
                    channel_paths: custom_shader_channel_paths,
                    cubemap_path: custom_shader_cubemap_path,
                    custom_uniforms: custom_shader_custom_uniforms,
                    use_background_as_channel0,
                    background_channel0_blend_mode,
                    auto_dim_under_text: custom_shader_auto_dim_under_text,
                    auto_dim_strength: custom_shader_auto_dim_strength,
                },
            );

        // Create cursor shader renderer if configured (separate from background shader)
        let (mut cursor_shader_renderer, initial_cursor_shader_path, startup_cursor_shader_error) =
            shaders::init_cursor_shader(
                &cell_renderer,
                shaders::CursorShaderInitParams {
                    size_width: size.width,
                    size_height: size.height,
                    window_padding,
                    path: cursor_shader_path,
                    enabled: cursor_shader_enabled,
                    animation: cursor_shader_animation,
                    animation_speed: cursor_shader_animation_speed,
                    window_opacity,
                },
            );

        // Sync DPI scale factor to shader renderers for cursor sizing
        if let Some(ref mut cs) = custom_shader_renderer {
            cs.set_scale_factor(scale);
        }
        if let Some(ref mut cs) = cursor_shader_renderer {
            cs.set_scale_factor(scale);
        }

        log::info!(
            "[renderer] Renderer created: custom_shader_loaded={}, cursor_shader_loaded={}",
            initial_shader_path.is_some(),
            initial_cursor_shader_path.is_some()
        );

        Ok(Self {
            cell_renderer,
            graphics_renderer,
            sixel_graphics: Vec::new(),
            egui_renderer,
            custom_shader_renderer,
            custom_shader_path: initial_shader_path,
            startup_shader_error,
            cursor_shader_renderer,
            cursor_shader_path: initial_cursor_shader_path,
            startup_cursor_shader_error,
            size,
            dirty: true, // Start dirty to ensure initial render
            last_scrollbar_state: (usize::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0), // Force first update
            cursor_shader_disabled_for_alt_screen: false,
            debug_text: None,
            scratch_divider_instances: Vec::new(),
        })
    }
}
