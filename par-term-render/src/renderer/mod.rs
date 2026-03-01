use crate::cell_renderer::{Cell, CellRenderer, PaneViewport};
use crate::custom_shader_renderer::CustomShaderRenderer;
use crate::graphics_renderer::GraphicsRenderer;
use anyhow::Result;
use winit::dpi::PhysicalSize;

mod egui_render;
pub mod graphics;
pub mod params;
mod render_passes;
mod rendering;
pub mod shaders;
mod state;

// Re-export SeparatorMark from par-term-config
pub use par_term_config::SeparatorMark;
pub use params::RendererParams;

/// Compute which separator marks are visible in the current viewport.
///
/// Maps absolute scrollback line numbers to screen rows for the current view.
/// Deduplicates marks that are close together (e.g., multi-line prompts generate
/// both a PromptStart and CommandStart mark within a few lines). When marks are
/// within `MERGE_THRESHOLD` lines of each other, they are merged — keeping the
/// earliest screen row (from PromptStart) while inheriting exit code and color
/// from whichever mark carries them.
pub fn compute_visible_separator_marks(
    marks: &[par_term_config::ScrollbackMark],
    scrollback_len: usize,
    scroll_offset: usize,
    visible_lines: usize,
) -> Vec<SeparatorMark> {
    let viewport_start = scrollback_len.saturating_sub(scroll_offset);
    let viewport_end = viewport_start + visible_lines;

    marks
        .iter()
        .filter_map(|mark| {
            if mark.line >= viewport_start && mark.line < viewport_end {
                let screen_row = mark.line - viewport_start;
                Some((screen_row, mark.exit_code, mark.color))
            } else {
                None
            }
        })
        .collect()
}

/// Information needed to render a single pane
pub struct PaneRenderInfo<'a> {
    /// Viewport bounds and state for this pane
    pub viewport: PaneViewport,
    /// Cells to render (should match viewport grid size)
    pub cells: &'a [Cell],
    /// Grid dimensions (cols, rows)
    pub grid_size: (usize, usize),
    /// Cursor position within this pane (col, row), or None if no cursor visible
    pub cursor_pos: Option<(usize, usize)>,
    /// Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    pub cursor_opacity: f32,
    /// Whether this pane has a scrollbar visible
    pub show_scrollbar: bool,
    /// Scrollback marks for this pane
    pub marks: Vec<par_term_config::ScrollbackMark>,
    /// Scrollback length for this pane (needed for separator mark mapping)
    pub scrollback_len: usize,
    /// Current scroll offset for this pane (needed for separator mark mapping)
    pub scroll_offset: usize,
    /// Per-pane background image override (None = use global background)
    pub background: Option<par_term_config::PaneBackground>,
    /// Inline graphics (Sixel/iTerm2/Kitty) to render for this pane
    pub graphics: Vec<par_term_emu_core_rust::graphics::TerminalGraphic>,
}

/// Information needed to render a pane divider
#[derive(Clone, Copy, Debug)]
pub struct DividerRenderInfo {
    /// X position in pixels
    pub x: f32,
    /// Y position in pixels
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
    /// Whether this divider is currently being hovered
    pub hovered: bool,
}

impl DividerRenderInfo {
    /// Create from a DividerRect
    pub fn from_rect(rect: &par_term_config::DividerRect, hovered: bool) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            hovered,
        }
    }
}

/// Information needed to render a pane title bar
#[derive(Clone, Debug)]
pub struct PaneTitleInfo {
    /// X position of the title bar in pixels
    pub x: f32,
    /// Y position of the title bar in pixels
    pub y: f32,
    /// Width of the title bar in pixels
    pub width: f32,
    /// Height of the title bar in pixels
    pub height: f32,
    /// Title text to display
    pub title: String,
    /// Whether this pane is focused
    pub focused: bool,
    /// Text color [R, G, B] as floats (0.0-1.0)
    pub text_color: [f32; 3],
    /// Background color [R, G, B] as floats (0.0-1.0)
    pub bg_color: [f32; 3],
}

/// Settings for rendering pane dividers and focus indicators
#[derive(Clone, Copy, Debug)]
pub struct PaneDividerSettings {
    /// Color for dividers [R, G, B] as floats (0.0-1.0)
    pub divider_color: [f32; 3],
    /// Color when hovering over dividers [R, G, B] as floats (0.0-1.0)
    pub hover_color: [f32; 3],
    /// Whether to show focus indicator around focused pane
    pub show_focus_indicator: bool,
    /// Color for focus indicator [R, G, B] as floats (0.0-1.0)
    pub focus_color: [f32; 3],
    /// Width of focus indicator border in pixels
    pub focus_width: f32,
    /// Style of dividers (solid, double, dashed, shadow)
    pub divider_style: par_term_config::DividerStyle,
}

impl Default for PaneDividerSettings {
    fn default() -> Self {
        Self {
            divider_color: [0.3, 0.3, 0.3],
            hover_color: [0.5, 0.6, 0.8],
            show_focus_indicator: true,
            focus_color: [0.4, 0.6, 1.0],
            focus_width: 2.0,
            divider_style: par_term_config::DividerStyle::default(),
        }
    }
}

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

    // Cursor shader renderer for cursor-specific effects (separate from background shader)
    pub(crate) cursor_shader_renderer: Option<CustomShaderRenderer>,
    // Track current cursor shader path to detect changes
    pub(crate) cursor_shader_path: Option<String>,

    // Cached for convenience
    pub(crate) size: PhysicalSize<u32>,

    // Dirty flag for optimization - only render when content has changed
    pub(crate) dirty: bool,

    // Cached scrollbar state to avoid redundant GPU uploads
    pub(crate) last_scrollbar_state: (usize, usize, usize),

    // Skip cursor shader when alt screen is active (TUI apps like vim, htop)
    pub(crate) cursor_shader_disabled_for_alt_screen: bool,

    // Debug overlay text
    pub(crate) debug_text: Option<String>,
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
        let use_background_as_channel0 = params.use_background_as_channel0;
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

        // Preliminary font lookup to get metrics for accurate cell height
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
        let char_height = (natural_line_height * line_spacing).max(1.0);

        // Scale logical pixel values (config) to physical pixels (wgpu surface)
        let scale = scale_factor as f32;
        let window_padding = params.window_padding * scale;
        let scrollbar_width = params.scrollbar_width * scale;

        // Calculate available space after padding and scrollbar
        let available_width = (size.width as f32 - window_padding * 2.0 - scrollbar_width).max(0.0);
        let available_height = (size.height as f32 - window_padding * 2.0).max(0.0);

        // Calculate terminal dimensions based on font size in pixels and spacing
        let char_width = (char_advance * char_spacing).max(1.0); // Configurable character width
        let cols = (available_width / char_width).max(1.0) as usize;
        let rows = (available_height / char_height).max(1.0) as usize;

        // Create cell renderer with font fallback support (owns scrollbar)
        let cell_renderer = CellRenderer::new(
            window.clone(),
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
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
            {
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
                bg_path
            },
            background_image_mode,
            background_image_opacity,
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
        let (mut custom_shader_renderer, initial_shader_path) = shaders::init_custom_shader(
            &cell_renderer,
            size.width,
            size.height,
            window_padding,
            custom_shader_path,
            custom_shader_enabled,
            custom_shader_animation,
            custom_shader_animation_speed,
            window_opacity,
            custom_shader_full_content,
            custom_shader_brightness,
            custom_shader_channel_paths,
            custom_shader_cubemap_path,
            use_background_as_channel0,
        );

        // Create cursor shader renderer if configured (separate from background shader)
        let (mut cursor_shader_renderer, initial_cursor_shader_path) = shaders::init_cursor_shader(
            &cell_renderer,
            size.width,
            size.height,
            window_padding,
            cursor_shader_path,
            cursor_shader_enabled,
            cursor_shader_animation,
            cursor_shader_animation_speed,
            window_opacity,
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
            cursor_shader_renderer,
            cursor_shader_path: initial_cursor_shader_path,
            size,
            dirty: true, // Start dirty to ensure initial render
            last_scrollbar_state: (usize::MAX, 0, 0), // Force first update
            cursor_shader_disabled_for_alt_screen: false,
            debug_text: None,
        })
    }

    /// Resize the renderer and recalculate grid dimensions based on padding/font metrics
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) -> (usize, usize) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.dirty = true; // Mark dirty on resize
            let result = self.cell_renderer.resize(new_size.width, new_size.height);

            // Update graphics renderer cell dimensions
            self.graphics_renderer.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                self.cell_renderer.window_padding(),
            );

            // Update custom shader renderer dimensions
            if let Some(ref mut custom_shader) = self.custom_shader_renderer {
                custom_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                custom_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            // Update cursor shader renderer dimensions
            if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
                cursor_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                cursor_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            return result;
        }

        self.cell_renderer.grid_size()
    }

    /// Update scale factor and resize so the PTY grid matches the new DPI.
    pub fn handle_scale_factor_change(
        &mut self,
        scale_factor: f64,
        new_size: PhysicalSize<u32>,
    ) -> (usize, usize) {
        let old_scale = self.cell_renderer.scale_factor;
        self.cell_renderer.update_scale_factor(scale_factor);
        let new_scale = self.cell_renderer.scale_factor;

        // Rescale physical pixel values when DPI changes
        if old_scale > 0.0 && (old_scale - new_scale).abs() > f32::EPSILON {
            // Rescale content_offset_y
            let logical_offset_y = self.cell_renderer.content_offset_y() / old_scale;
            let new_physical_offset_y = logical_offset_y * new_scale;
            self.cell_renderer
                .set_content_offset_y(new_physical_offset_y);
            self.graphics_renderer
                .set_content_offset_y(new_physical_offset_y);
            if let Some(ref mut cs) = self.custom_shader_renderer {
                cs.set_content_offset_y(new_physical_offset_y);
            }
            if let Some(ref mut cs) = self.cursor_shader_renderer {
                cs.set_content_offset_y(new_physical_offset_y);
            }

            // Rescale content_offset_x
            let logical_offset_x = self.cell_renderer.content_offset_x() / old_scale;
            let new_physical_offset_x = logical_offset_x * new_scale;
            self.cell_renderer
                .set_content_offset_x(new_physical_offset_x);
            self.graphics_renderer
                .set_content_offset_x(new_physical_offset_x);
            if let Some(ref mut cs) = self.custom_shader_renderer {
                cs.set_content_offset_x(new_physical_offset_x);
            }
            if let Some(ref mut cs) = self.cursor_shader_renderer {
                cs.set_content_offset_x(new_physical_offset_x);
            }

            // Rescale content_inset_bottom
            let logical_inset_bottom = self.cell_renderer.content_inset_bottom() / old_scale;
            let new_physical_inset_bottom = logical_inset_bottom * new_scale;
            self.cell_renderer
                .set_content_inset_bottom(new_physical_inset_bottom);

            // Rescale egui_bottom_inset (status bar)
            if self.cell_renderer.grid.egui_bottom_inset > 0.0 {
                let logical_egui_bottom = self.cell_renderer.grid.egui_bottom_inset / old_scale;
                self.cell_renderer.grid.egui_bottom_inset = logical_egui_bottom * new_scale;
            }

            // Rescale content_inset_right (AI Inspector panel)
            if self.cell_renderer.grid.content_inset_right > 0.0 {
                let logical_inset_right = self.cell_renderer.grid.content_inset_right / old_scale;
                self.cell_renderer.grid.content_inset_right = logical_inset_right * new_scale;
            }

            // Rescale egui_right_inset
            if self.cell_renderer.grid.egui_right_inset > 0.0 {
                let logical_egui_right = self.cell_renderer.grid.egui_right_inset / old_scale;
                self.cell_renderer.grid.egui_right_inset = logical_egui_right * new_scale;
            }

            // Rescale window_padding
            let logical_padding = self.cell_renderer.window_padding() / old_scale;
            let new_physical_padding = logical_padding * new_scale;
            self.cell_renderer
                .update_window_padding(new_physical_padding);

            // Sync new scale factor to shader renderers for cursor sizing
            if let Some(ref mut cs) = self.custom_shader_renderer {
                cs.set_scale_factor(new_scale);
            }
            if let Some(ref mut cs) = self.cursor_shader_renderer {
                cs.set_scale_factor(new_scale);
            }
        }

        self.resize(new_size)
    }
}

// Layout and sizing accessors — simple getters/setters for grid geometry, padding,
// and content offsets. Co-located here with resize/handle_scale_factor_change since
// all of these deal with the spatial layout of the renderer.
impl Renderer {
    /// Get the current size
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    /// Get the current grid dimensions (columns, rows)
    pub fn grid_size(&self) -> (usize, usize) {
        self.cell_renderer.grid_size()
    }

    /// Get cell width in pixels
    pub fn cell_width(&self) -> f32 {
        self.cell_renderer.cell_width()
    }

    /// Get cell height in pixels
    pub fn cell_height(&self) -> f32 {
        self.cell_renderer.cell_height()
    }

    /// Get window padding in physical pixels (scaled by DPI)
    pub fn window_padding(&self) -> f32 {
        self.cell_renderer.window_padding()
    }

    /// Get the vertical content offset in physical pixels (e.g., tab bar height scaled by DPI)
    pub fn content_offset_y(&self) -> f32 {
        self.cell_renderer.content_offset_y()
    }

    /// Get the display scale factor (e.g., 2.0 on Retina displays)
    pub fn scale_factor(&self) -> f32 {
        self.cell_renderer.scale_factor
    }

    /// Set the vertical content offset (e.g., tab bar height) in logical pixels.
    /// The offset is scaled by the display scale factor to physical pixels internally,
    /// since the cell renderer works in physical pixel coordinates while egui (tab bar)
    /// uses logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_y(&mut self, logical_offset: f32) -> Option<(usize, usize)> {
        // Scale from logical pixels (egui/config) to physical pixels (wgpu surface)
        let physical_offset = logical_offset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_offset_y(physical_offset);
        // Always update graphics renderer offset, even if grid size didn't change
        self.graphics_renderer.set_content_offset_y(physical_offset);
        // Update custom shader renderer content offset
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_offset_y(physical_offset);
        }
        // Update cursor shader renderer content offset
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_offset_y(physical_offset);
        }
        if result.is_some() {
            self.dirty = true;
        }
        result
    }

    /// Get the horizontal content offset in physical pixels
    pub fn content_offset_x(&self) -> f32 {
        self.cell_renderer.content_offset_x()
    }

    /// Set the horizontal content offset (e.g., tab bar on left) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_x(&mut self, logical_offset: f32) -> Option<(usize, usize)> {
        let physical_offset = logical_offset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_offset_x(physical_offset);
        self.graphics_renderer.set_content_offset_x(physical_offset);
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_offset_x(physical_offset);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_offset_x(physical_offset);
        }
        if result.is_some() {
            self.dirty = true;
        }
        result
    }

    /// Get the bottom content inset in physical pixels
    pub fn content_inset_bottom(&self) -> f32 {
        self.cell_renderer.content_inset_bottom()
    }

    /// Get the right content inset in physical pixels
    pub fn content_inset_right(&self) -> f32 {
        self.cell_renderer.content_inset_right()
    }

    /// Set the bottom content inset (e.g., tab bar at bottom) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_bottom(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_inset_bottom(physical_inset);
        if result.is_some() {
            self.dirty = true;
            // Invalidate the scrollbar cache — the track height depends on
            // the bottom inset, so the scrollbar must be repositioned.
            self.last_scrollbar_state = (usize::MAX, 0, 0);
        }
        result
    }

    /// Set the right content inset (e.g., AI Inspector panel) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_right(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_inset_right(physical_inset);

        // Also update custom shader renderer to exclude panel area from effects
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_inset_right(physical_inset);
        }
        // Also update cursor shader renderer
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_inset_right(physical_inset);
        }

        if result.is_some() {
            self.dirty = true;
            // Invalidate the scrollbar cache so the next update_scrollbar()
            // repositions the scrollbar at the new right inset. Without this,
            // the cache guard sees the same (scroll_offset, visible_lines,
            // total_lines) tuple and skips the GPU upload, leaving the
            // scrollbar stuck at the old position.
            self.last_scrollbar_state = (usize::MAX, 0, 0);
        }
        result
    }

    /// Set the additional bottom inset from egui panels (status bar, tmux bar).
    ///
    /// This inset reduces the terminal grid height so content does not render
    /// behind the status bar. Also affects scrollbar bounds.
    /// Returns `Some((cols, rows))` if the grid was resized.
    pub fn set_egui_bottom_inset(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        if (self.cell_renderer.grid.egui_bottom_inset - physical_inset).abs() > f32::EPSILON {
            self.cell_renderer.grid.egui_bottom_inset = physical_inset;
            let (w, h) = (
                self.cell_renderer.config.width,
                self.cell_renderer.config.height,
            );
            return Some(self.cell_renderer.resize(w, h));
        }
        None
    }

    /// Set the additional right inset from egui panels (AI Inspector).
    ///
    /// This inset is added to `content_inset_right` for scrollbar bounds only.
    /// egui panels already claim space before wgpu rendering, so this doesn't
    /// affect the terminal grid sizing.
    pub fn set_egui_right_inset(&mut self, logical_inset: f32) {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        self.cell_renderer.grid.egui_right_inset = physical_inset;
    }
}
