use crate::cell_renderer::{Cell, CellRenderer, PaneViewport};
use crate::custom_shader_renderer::CustomShaderRenderer;
use crate::graphics_renderer::GraphicsRenderer;
use anyhow::Result;
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub mod graphics;
pub mod shaders;

// Re-export SeparatorMark from par-term-config
pub use par_term_config::SeparatorMark;

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

    // Current sixel graphics to render: (id, row, col, width_cells, height_cells, alpha, scroll_offset_rows)
    // Note: row is isize to allow negative values for graphics scrolled off top
    pub(crate) sixel_graphics: Vec<(u64, isize, usize, usize, usize, f32, usize)>,

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
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub(crate) debug_text: Option<String>,
}

impl Renderer {
    /// Create a new renderer
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        window: Arc<Window>,
        font_family: Option<&str>,
        font_family_bold: Option<&str>,
        font_family_italic: Option<&str>,
        font_family_bold_italic: Option<&str>,
        font_ranges: &[par_term_config::FontRange],
        font_size: f32,
        window_padding: f32,
        line_spacing: f32,
        char_spacing: f32,
        scrollbar_position: &str,
        scrollbar_width: f32,
        scrollbar_thumb_color: [f32; 4],
        scrollbar_track_color: [f32; 4],
        enable_text_shaping: bool,
        enable_ligatures: bool,
        enable_kerning: bool,
        font_antialias: bool,
        font_hinting: bool,
        font_thin_strokes: par_term_config::ThinStrokesMode,
        minimum_contrast: f32,
        vsync_mode: par_term_config::VsyncMode,
        power_preference: par_term_config::PowerPreference,
        window_opacity: f32,
        background_color: [u8; 3],
        background_image_path: Option<&str>,
        background_image_enabled: bool,
        background_image_mode: par_term_config::BackgroundImageMode,
        background_image_opacity: f32,
        custom_shader_path: Option<&str>,
        custom_shader_enabled: bool,
        custom_shader_animation: bool,
        custom_shader_animation_speed: f32,
        custom_shader_full_content: bool,
        custom_shader_brightness: f32,
        // Custom shader channel textures (iChannel0-3)
        custom_shader_channel_paths: &[Option<std::path::PathBuf>; 4],
        // Cubemap texture path prefix for environment mapping (iCubemap)
        custom_shader_cubemap_path: Option<&std::path::Path>,
        // Use background image as iChannel0 for custom shaders
        use_background_as_channel0: bool,
        // Inline image scaling mode (nearest vs linear)
        image_scaling_mode: par_term_config::ImageScalingMode,
        // Preserve aspect ratio when scaling inline images
        image_preserve_aspect_ratio: bool,
        // Cursor shader settings (separate from background shader)
        cursor_shader_path: Option<&str>,
        cursor_shader_enabled: bool,
        cursor_shader_animation: bool,
        cursor_shader_animation_speed: f32,
    ) -> Result<Self> {
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
            let primary_font = font_manager.get_font(0).unwrap();
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
        let window_padding = window_padding * scale;
        let scrollbar_width = scrollbar_width * scale;

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
            if self.cell_renderer.egui_bottom_inset > 0.0 {
                let logical_egui_bottom = self.cell_renderer.egui_bottom_inset / old_scale;
                self.cell_renderer.egui_bottom_inset = logical_egui_bottom * new_scale;
            }

            // Rescale content_inset_right (AI Inspector panel)
            if self.cell_renderer.content_inset_right > 0.0 {
                let logical_inset_right = self.cell_renderer.content_inset_right / old_scale;
                self.cell_renderer.content_inset_right = logical_inset_right * new_scale;
            }

            // Rescale egui_right_inset
            if self.cell_renderer.egui_right_inset > 0.0 {
                let logical_egui_right = self.cell_renderer.egui_right_inset / old_scale;
                self.cell_renderer.egui_right_inset = logical_egui_right * new_scale;
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

    /// Update the terminal cells
    pub fn update_cells(&mut self, cells: &[Cell]) {
        if self.cell_renderer.update_cells(cells) {
            self.dirty = true;
        }
    }

    /// Clear all cells in the renderer.
    /// Call this when switching tabs to ensure a clean slate.
    pub fn clear_all_cells(&mut self) {
        self.cell_renderer.clear_all_cells();
        self.dirty = true;
    }

    /// Update cursor position and style for geometric rendering
    pub fn update_cursor(
        &mut self,
        position: (usize, usize),
        opacity: f32,
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        if self.cell_renderer.update_cursor(position, opacity, style) {
            self.dirty = true;
        }
    }

    /// Clear cursor (hide it)
    pub fn clear_cursor(&mut self) {
        if self.cell_renderer.clear_cursor() {
            self.dirty = true;
        }
    }

    /// Update scrollbar state
    ///
    /// # Arguments
    /// * `scroll_offset` - Current scroll offset (0 = at bottom)
    /// * `visible_lines` - Number of lines visible on screen
    /// * `total_lines` - Total number of lines including scrollback
    /// * `marks` - Scrollback marks for visualization on the scrollbar
    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
        marks: &[par_term_config::ScrollbackMark],
    ) {
        let new_state = (scroll_offset, visible_lines, total_lines);
        if new_state == self.last_scrollbar_state {
            return;
        }
        self.last_scrollbar_state = new_state;
        self.cell_renderer
            .update_scrollbar(scroll_offset, visible_lines, total_lines, marks);
        self.dirty = true;
    }

    /// Set the visual bell flash intensity
    ///
    /// # Arguments
    /// * `intensity` - Flash intensity from 0.0 (no flash) to 1.0 (full white flash)
    pub fn set_visual_bell_intensity(&mut self, intensity: f32) {
        self.cell_renderer.set_visual_bell_intensity(intensity);
        if intensity > 0.0 {
            self.dirty = true; // Mark dirty when flash is active
        }
    }

    /// Update window opacity in real-time
    pub fn update_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_opacity(opacity);

        // Propagate to custom shader renderer if present
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_opacity(opacity);
        }

        // Propagate to cursor shader renderer if present
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_opacity(opacity);
        }

        self.dirty = true;
    }

    /// Update cursor color for cell rendering
    pub fn update_cursor_color(&mut self, color: [u8; 3]) {
        self.cell_renderer.update_cursor_color(color);
        self.dirty = true;
    }

    /// Update cursor text color (color of text under block cursor)
    pub fn update_cursor_text_color(&mut self, color: Option<[u8; 3]>) {
        self.cell_renderer.update_cursor_text_color(color);
        self.dirty = true;
    }

    /// Set whether cursor should be hidden when cursor shader is active
    pub fn set_cursor_hidden_for_shader(&mut self, hidden: bool) {
        if self.cell_renderer.set_cursor_hidden_for_shader(hidden) {
            self.dirty = true;
        }
    }

    /// Set window focus state (affects unfocused cursor rendering)
    pub fn set_focused(&mut self, focused: bool) {
        if self.cell_renderer.set_focused(focused) {
            self.dirty = true;
        }
    }

    /// Update cursor guide settings
    pub fn update_cursor_guide(&mut self, enabled: bool, color: [u8; 4]) {
        self.cell_renderer.update_cursor_guide(enabled, color);
        self.dirty = true;
    }

    /// Update cursor shadow settings.
    /// Offset and blur are in logical pixels and will be scaled to physical pixels internally.
    pub fn update_cursor_shadow(
        &mut self,
        enabled: bool,
        color: [u8; 4],
        offset: [f32; 2],
        blur: f32,
    ) {
        let scale = self.cell_renderer.scale_factor;
        let physical_offset = [offset[0] * scale, offset[1] * scale];
        let physical_blur = blur * scale;
        self.cell_renderer
            .update_cursor_shadow(enabled, color, physical_offset, physical_blur);
        self.dirty = true;
    }

    /// Update cursor boost settings
    pub fn update_cursor_boost(&mut self, intensity: f32, color: [u8; 3]) {
        self.cell_renderer.update_cursor_boost(intensity, color);
        self.dirty = true;
    }

    /// Update unfocused cursor style
    pub fn update_unfocused_cursor_style(&mut self, style: par_term_config::UnfocusedCursorStyle) {
        self.cell_renderer.update_unfocused_cursor_style(style);
        self.dirty = true;
    }

    /// Update command separator settings from config.
    /// Thickness is in logical pixels and will be scaled to physical pixels internally.
    pub fn update_command_separator(
        &mut self,
        enabled: bool,
        logical_thickness: f32,
        opacity: f32,
        exit_color: bool,
        color: [u8; 3],
    ) {
        let physical_thickness = logical_thickness * self.cell_renderer.scale_factor;
        self.cell_renderer.update_command_separator(
            enabled,
            physical_thickness,
            opacity,
            exit_color,
            color,
        );
        self.dirty = true;
    }

    /// Set the visible separator marks for the current frame (single-pane path)
    pub fn set_separator_marks(&mut self, marks: Vec<SeparatorMark>) {
        if self.cell_renderer.set_separator_marks(marks) {
            self.dirty = true;
        }
    }

    /// Set gutter indicator data for the current frame (single-pane path).
    pub fn set_gutter_indicators(&mut self, indicators: Vec<(usize, [f32; 4])>) {
        self.cell_renderer.set_gutter_indicators(indicators);
        self.dirty = true;
    }

    /// Set whether transparency affects only default background cells.
    /// When true, non-default (colored) backgrounds remain opaque for readability.
    pub fn set_transparency_affects_only_default_background(&mut self, value: bool) {
        self.cell_renderer
            .set_transparency_affects_only_default_background(value);
        self.dirty = true;
    }

    /// Set whether text should always be rendered at full opacity.
    /// When true, text remains opaque regardless of window transparency settings.
    pub fn set_keep_text_opaque(&mut self, value: bool) {
        self.cell_renderer.set_keep_text_opaque(value);

        // Also propagate to custom shader renderer if present
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_keep_text_opaque(value);
        }

        // And to cursor shader renderer if present
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_keep_text_opaque(value);
        }

        self.dirty = true;
    }

    pub fn set_link_underline_style(&mut self, style: par_term_config::LinkUnderlineStyle) {
        self.cell_renderer.set_link_underline_style(style);
        self.dirty = true;
    }

    /// Set whether cursor shader should be disabled due to alt screen being active
    ///
    /// When alt screen is active (e.g., vim, htop, less), cursor shader effects
    /// are disabled since TUI applications typically have their own cursor handling.
    pub fn set_cursor_shader_disabled_for_alt_screen(&mut self, disabled: bool) {
        if self.cursor_shader_disabled_for_alt_screen != disabled {
            log::debug!("[cursor-shader] Alt-screen disable set to {}", disabled);
            self.cursor_shader_disabled_for_alt_screen = disabled;
        } else {
            self.cursor_shader_disabled_for_alt_screen = disabled;
        }
    }

    /// Update window padding in real-time without full renderer rebuild.
    /// Accepts logical pixels (from config); scales to physical pixels internally.
    /// Returns Some((cols, rows)) if grid size changed and terminal needs resize.
    #[allow(dead_code)]
    pub fn update_window_padding(&mut self, logical_padding: f32) -> Option<(usize, usize)> {
        let physical_padding = logical_padding * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.update_window_padding(physical_padding);
        // Update graphics renderer padding
        self.graphics_renderer.update_cell_dimensions(
            self.cell_renderer.cell_width(),
            self.cell_renderer.cell_height(),
            physical_padding,
        );
        // Update custom shader renderer padding
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                physical_padding,
            );
        }
        // Update cursor shader renderer padding
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                physical_padding,
            );
        }
        self.dirty = true;
        result
    }

    /// Enable/disable background image and reload if needed
    #[allow(dead_code)]
    pub fn set_background_image_enabled(
        &mut self,
        enabled: bool,
        path: Option<&str>,
        mode: par_term_config::BackgroundImageMode,
        opacity: f32,
    ) {
        let path = if enabled { path } else { None };
        self.cell_renderer.set_background_image(path, mode, opacity);

        // Sync background texture to custom shader if it's using background as channel0
        self.sync_background_texture_to_shader();

        self.dirty = true;
    }

    /// Set background based on mode (Default, Color, or Image).
    ///
    /// This unified method handles all background types and syncs with shaders.
    pub fn set_background(
        &mut self,
        mode: par_term_config::BackgroundMode,
        color: [u8; 3],
        image_path: Option<&str>,
        image_mode: par_term_config::BackgroundImageMode,
        image_opacity: f32,
        image_enabled: bool,
    ) {
        self.cell_renderer.set_background(
            mode,
            color,
            image_path,
            image_mode,
            image_opacity,
            image_enabled,
        );

        // Sync background texture to custom shader if it's using background as channel0
        self.sync_background_texture_to_shader();

        // Sync background to shaders for proper compositing
        let is_solid_color = matches!(mode, par_term_config::BackgroundMode::Color);
        let is_image_mode = matches!(mode, par_term_config::BackgroundMode::Image);
        let normalized_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];

        // Sync to cursor shader
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            // When background shader is enabled and chained into cursor shader,
            // don't give cursor shader its own background - background shader handles it
            let has_background_shader = self.custom_shader_renderer.is_some();

            if has_background_shader {
                // Background shader handles the background, cursor shader just passes through
                cursor_shader.set_background_color([0.0, 0.0, 0.0], false);
                cursor_shader.set_background_texture(self.cell_renderer.device(), None);
                cursor_shader.update_use_background_as_channel0(self.cell_renderer.device(), false);
            } else {
                cursor_shader.set_background_color(normalized_color, is_solid_color);

                // For image mode, pass background image as iChannel0
                if is_image_mode && image_enabled {
                    let bg_texture = self.cell_renderer.get_background_as_channel_texture();
                    cursor_shader.set_background_texture(self.cell_renderer.device(), bg_texture);
                    cursor_shader
                        .update_use_background_as_channel0(self.cell_renderer.device(), true);
                } else {
                    // Clear background texture when not in image mode
                    cursor_shader.set_background_texture(self.cell_renderer.device(), None);
                    cursor_shader
                        .update_use_background_as_channel0(self.cell_renderer.device(), false);
                }
            }
        }

        // Sync to custom shader
        // Note: We don't pass is_solid_color=true to custom shaders because
        // that would replace the shader output with a solid color, making the
        // shader invisible. Custom shaders handle their own background.
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_background_color(normalized_color, false);
        }

        self.dirty = true;
    }

    /// Update scrollbar appearance in real-time.
    /// Width is in logical pixels and will be scaled to physical pixels internally.
    pub fn update_scrollbar_appearance(
        &mut self,
        logical_width: f32,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) {
        let physical_width = logical_width * self.cell_renderer.scale_factor;
        self.cell_renderer
            .update_scrollbar_appearance(physical_width, thumb_color, track_color);
        self.dirty = true;
    }

    /// Update scrollbar position (left/right) in real-time
    #[allow(dead_code)]
    pub fn update_scrollbar_position(&mut self, position: &str) {
        self.cell_renderer.update_scrollbar_position(position);
        self.dirty = true;
    }

    /// Update background image opacity in real-time
    #[allow(dead_code)]
    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_background_image_opacity(opacity);
        self.dirty = true;
    }

    /// Load a per-pane background image into the texture cache.
    /// Delegates to CellRenderer::load_pane_background.
    pub fn load_pane_background(&mut self, path: &str) -> anyhow::Result<bool> {
        self.cell_renderer.load_pane_background(path)
    }

    /// Update inline image scaling mode (nearest vs linear filtering).
    ///
    /// Recreates the GPU sampler and clears the texture cache so images
    /// are re-rendered with the new filter mode.
    pub fn update_image_scaling_mode(&mut self, scaling_mode: par_term_config::ImageScalingMode) {
        self.graphics_renderer
            .update_scaling_mode(self.cell_renderer.device(), scaling_mode);
        self.dirty = true;
    }

    /// Update whether inline images preserve their aspect ratio.
    pub fn update_image_preserve_aspect_ratio(&mut self, preserve: bool) {
        self.graphics_renderer.set_preserve_aspect_ratio(preserve);
        self.dirty = true;
    }

    /// Check if animation requires continuous rendering
    ///
    /// Returns true if shader animation is enabled or a cursor trail animation
    /// might still be in progress.
    pub fn needs_continuous_render(&self) -> bool {
        let custom_needs = self
            .custom_shader_renderer
            .as_ref()
            .is_some_and(|r| r.animation_enabled() || r.cursor_needs_animation());
        let cursor_needs = self
            .cursor_shader_renderer
            .as_ref()
            .is_some_and(|r| r.animation_enabled() || r.cursor_needs_animation());
        custom_needs || cursor_needs
    }

    /// Render a frame with optional egui overlay
    /// Returns true if rendering was performed, false if skipped
    pub fn render(
        &mut self,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
        show_scrollbar: bool,
        pane_background: Option<&par_term_config::PaneBackground>,
    ) -> Result<bool> {
        // Custom shader animation forces continuous rendering
        let force_render = self.needs_continuous_render();

        // Fast path: when nothing changed, render cells from cached buffers + egui overlay
        // This skips expensive shader passes, sixel uploads, etc.
        if !self.dirty && !force_render {
            if let Some((egui_output, egui_ctx)) = egui_data {
                let surface_texture = self.cell_renderer.render(show_scrollbar, pane_background)?;
                self.cell_renderer
                    .render_overlays(&surface_texture, show_scrollbar)?;
                self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
                surface_texture.present();
                return Ok(true);
            }
            return Ok(false);
        }

        // Check if shaders are enabled
        let has_custom_shader = self.custom_shader_renderer.is_some();
        // Only use cursor shader if it's enabled and not disabled for alt screen
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

        // Cell renderer renders terminal content
        let t1 = std::time::Instant::now();
        let surface_texture = if has_custom_shader {
            // When custom shader is enabled, always skip rendering background image
            // to the intermediate texture. The shader controls the background:
            // - If user wants background image in shader, enable use_background_as_channel0
            // - Otherwise, the shader's own effects provide the background
            // This prevents the background image from being treated as "terminal content"
            // and passed through unchanged by the shader.

            // Render terminal to intermediate texture for background shader
            self.cell_renderer.render_to_texture(
                self.custom_shader_renderer
                    .as_ref()
                    .unwrap()
                    .intermediate_texture_view(),
                true, // Always skip background image - shader handles background
            )?
        } else if use_cursor_shader {
            // Render terminal to intermediate texture for cursor shader
            // Skip background image - it will be handled via iBackgroundColor uniform
            // or passed as iChannel0. This ensures proper opacity handling.
            self.cell_renderer.render_to_texture(
                self.cursor_shader_renderer
                    .as_ref()
                    .unwrap()
                    .intermediate_texture_view(),
                true, // Skip background image - shader handles it
            )?
        } else {
            // Render directly to surface (no shaders, or cursor shader disabled for alt screen)
            // Note: scrollbar is rendered separately after egui so it appears on top
            self.cell_renderer.render(show_scrollbar, pane_background)?
        };
        let cell_render_time = t1.elapsed();

        // Apply background custom shader if enabled
        let t_custom = std::time::Instant::now();
        let custom_shader_time = if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            if use_cursor_shader {
                // Background shader renders to cursor shader's intermediate texture
                // Don't apply opacity here - cursor shader will apply it when rendering to surface
                custom_shader.render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    self.cursor_shader_renderer
                        .as_ref()
                        .unwrap()
                        .intermediate_texture_view(),
                    false, // Don't apply opacity - cursor shader will do it
                )?;
            } else {
                // Background shader renders directly to surface
                // (cursor shader disabled for alt screen or not configured)
                let surface_view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                custom_shader.render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &surface_view,
                    true, // Apply opacity - this is the final render
                )?;
            }
            t_custom.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        // Apply cursor shader if enabled (skip when alt screen is active for TUI apps)
        let t_cursor = std::time::Instant::now();
        let cursor_shader_time = if use_cursor_shader {
            log::trace!("Rendering cursor shader");
            let cursor_shader = self.cursor_shader_renderer.as_mut().unwrap();
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            cursor_shader.render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
                true, // Apply opacity - this is the final render to surface
            )?;
            t_cursor.elapsed()
        } else {
            if self.cursor_shader_disabled_for_alt_screen {
                log::trace!("Skipping cursor shader - alt screen active");
            }
            std::time::Duration::ZERO
        };

        // Render sixel graphics on top of cells
        let t2 = std::time::Instant::now();
        if !self.sixel_graphics.is_empty() {
            self.render_sixel_graphics(&surface_texture)?;
        }
        let sixel_render_time = t2.elapsed();

        // Render overlays (scrollbar, visual bell) BEFORE egui so that modal
        // dialogs (egui) render on top of the scrollbar. The scrollbar track
        // already accounts for status bar inset via content_inset_bottom.
        self.cell_renderer
            .render_overlays(&surface_texture, show_scrollbar)?;

        // Render egui overlay if provided
        let t3 = std::time::Instant::now();
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }
        let egui_render_time = t3.elapsed();

        // Present the surface texture - THIS IS WHERE VSYNC WAIT HAPPENS
        let t4 = std::time::Instant::now();
        surface_texture.present();
        let present_time = t4.elapsed();

        // Log timing breakdown
        let total = cell_render_time
            + custom_shader_time
            + cursor_shader_time
            + sixel_render_time
            + egui_render_time
            + present_time;
        if present_time.as_millis() > 10 || total.as_millis() > 10 {
            log::info!(
                "[RENDER] RENDER_BREAKDOWN: CellRender={:.2}ms BgShader={:.2}ms CursorShader={:.2}ms Sixel={:.2}ms Egui={:.2}ms PRESENT={:.2}ms Total={:.2}ms",
                cell_render_time.as_secs_f64() * 1000.0,
                custom_shader_time.as_secs_f64() * 1000.0,
                cursor_shader_time.as_secs_f64() * 1000.0,
                sixel_render_time.as_secs_f64() * 1000.0,
                egui_render_time.as_secs_f64() * 1000.0,
                present_time.as_secs_f64() * 1000.0,
                total.as_secs_f64() * 1000.0
            );
        }

        // Clear dirty flag after successful render
        self.dirty = false;

        Ok(true)
    }

    /// Render multiple panes to the surface
    ///
    /// This method renders each pane's content to its viewport region,
    /// handling focus indicators and inactive pane dimming.
    ///
    /// # Arguments
    /// * `panes` - List of panes to render with their viewport info
    /// * `egui_data` - Optional egui overlay data
    /// * `force_egui_opaque` - Force egui to render at full opacity
    ///
    /// # Returns
    /// `true` if rendering was performed, `false` if skipped
    #[allow(dead_code)]
    pub fn render_panes(
        &mut self,
        panes: &[PaneRenderInfo<'_>],
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
    ) -> Result<bool> {
        // Check if we need to render
        let force_render = self.needs_continuous_render();
        if !self.dirty && !force_render && egui_data.is_none() {
            return Ok(false);
        }

        // Get the surface texture
        let surface_texture = self.cell_renderer.surface.get_current_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Clear the surface first with the background color (respecting solid color mode)
        {
            let mut encoder = self.cell_renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("pane clear encoder"),
                },
            );

            let opacity = self.cell_renderer.window_opacity as f64;
            let clear_color = if self.cell_renderer.bg_is_solid_color {
                wgpu::Color {
                    r: self.cell_renderer.solid_bg_color[0] as f64 * opacity,
                    g: self.cell_renderer.solid_bg_color[1] as f64 * opacity,
                    b: self.cell_renderer.solid_bg_color[2] as f64 * opacity,
                    a: opacity,
                }
            } else {
                wgpu::Color {
                    r: self.cell_renderer.background_color[0] as f64 * opacity,
                    g: self.cell_renderer.background_color[1] as f64 * opacity,
                    b: self.cell_renderer.background_color[2] as f64 * opacity,
                    a: opacity,
                }
            };

            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("surface clear pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            self.cell_renderer
                .queue()
                .submit(std::iter::once(encoder.finish()));
        }

        // Render background image first (full-screen, before panes)
        let has_background_image = self
            .cell_renderer
            .render_background_only(&surface_view, false)?;

        // Render each pane (skip background image since we rendered it full-screen)
        for pane in panes {
            let separator_marks = compute_visible_separator_marks(
                &pane.marks,
                pane.scrollback_len,
                pane.scroll_offset,
                pane.grid_size.1,
            );
            self.cell_renderer.render_pane_to_view(
                &surface_view,
                &pane.viewport,
                pane.cells,
                pane.grid_size.0,
                pane.grid_size.1,
                pane.cursor_pos,
                pane.cursor_opacity,
                pane.show_scrollbar,
                false,                // Don't clear - we already cleared the surface
                has_background_image, // Skip background image if already rendered full-screen
                &separator_marks,
                pane.background.as_ref(),
            )?;
        }

        // Render egui overlay if provided
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }

        // Present the surface
        surface_texture.present();

        self.dirty = false;
        Ok(true)
    }

    /// Render split panes with dividers and focus indicator
    ///
    /// This is the main entry point for rendering a split pane layout.
    /// It handles:
    /// 1. Clearing the surface
    /// 2. Rendering each pane's content
    /// 3. Rendering dividers between panes
    /// 4. Rendering focus indicator around the focused pane
    /// 5. Rendering egui overlay if provided
    /// 6. Presenting the surface
    ///
    /// # Arguments
    /// * `panes` - List of panes to render with their viewport info
    /// * `dividers` - List of dividers between panes with hover state
    /// * `focused_viewport` - Viewport of the focused pane (for focus indicator)
    /// * `divider_settings` - Settings for divider and focus indicator appearance
    /// * `egui_data` - Optional egui overlay data
    /// * `force_egui_opaque` - Force egui to render at full opacity
    ///
    /// # Returns
    /// `true` if rendering was performed, `false` if skipped
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn render_split_panes(
        &mut self,
        panes: &[PaneRenderInfo<'_>],
        dividers: &[DividerRenderInfo],
        pane_titles: &[PaneTitleInfo],
        focused_viewport: Option<&PaneViewport>,
        divider_settings: &PaneDividerSettings,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
    ) -> Result<bool> {
        // Check if we need to render
        let force_render = self.needs_continuous_render();
        if !self.dirty && !force_render && egui_data.is_none() {
            return Ok(false);
        }

        let has_custom_shader = self.custom_shader_renderer.is_some();

        // Pre-load any per-pane background textures that aren't cached yet
        for pane in panes.iter() {
            if let Some(ref bg) = pane.background
                && let Some(ref path) = bg.image_path
                && let Err(e) = self.cell_renderer.load_pane_background(path)
            {
                log::error!("Failed to load pane background '{}': {}", path, e);
            }
        }

        // Get the surface texture
        let surface_texture = self.cell_renderer.surface.get_current_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Clear the surface with background color (respecting solid color mode)
        let opacity = self.cell_renderer.window_opacity as f64;
        let clear_color = if self.cell_renderer.bg_is_solid_color {
            wgpu::Color {
                r: self.cell_renderer.solid_bg_color[0] as f64 * opacity,
                g: self.cell_renderer.solid_bg_color[1] as f64 * opacity,
                b: self.cell_renderer.solid_bg_color[2] as f64 * opacity,
                a: opacity,
            }
        } else {
            wgpu::Color {
                r: self.cell_renderer.background_color[0] as f64 * opacity,
                g: self.cell_renderer.background_color[1] as f64 * opacity,
                b: self.cell_renderer.background_color[2] as f64 * opacity,
                a: opacity,
            }
        };

        // If custom shader is enabled, render it with the background clear color
        // (the shader's render pass will handle clearing the surface)
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            // Clear the intermediate texture to remove any old single-pane content
            // This prevents the shader from displaying stale terminal content
            custom_shader.clear_intermediate_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
            );

            // Render shader effect to surface with background color as clear
            // Don't apply opacity here - pane cells will blend on top
            custom_shader.render_with_clear_color(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
                false, // Don't apply opacity - let pane rendering handle it
                clear_color,
            )?;
        } else {
            // No custom shader - just clear the surface with background color
            let mut encoder = self.cell_renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("split pane clear encoder"),
                },
            );

            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("surface clear pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            self.cell_renderer
                .queue()
                .submit(std::iter::once(encoder.finish()));
        }

        // Render background image (full-screen, after shader but before panes)
        // Skip if custom shader is handling the background.
        // Also skip if any pane has a per-pane background configured -
        // per-pane backgrounds are rendered individually in render_pane_to_view.
        let any_pane_has_background = panes.iter().any(|p| p.background.is_some());
        let has_background_image = if !has_custom_shader && !any_pane_has_background {
            self.cell_renderer
                .render_background_only(&surface_view, false)?
        } else {
            false
        };

        // Update scrollbar state for the focused pane before rendering.
        // In single-pane mode this is done in the main render loop; in split mode
        // we must do it here, constrained to the pane's pixel bounds, so the
        // track and thumb appear inside the focused pane rather than spanning
        // the full window height/width.
        for pane in panes.iter() {
            if pane.viewport.focused && pane.show_scrollbar {
                let total_lines = pane.scrollback_len + pane.grid_size.1;
                let new_state = (pane.scroll_offset, pane.grid_size.1, total_lines);
                if new_state != self.last_scrollbar_state {
                    self.last_scrollbar_state = new_state;
                    self.cell_renderer.update_scrollbar_for_pane(
                        pane.scroll_offset,
                        pane.grid_size.1,
                        total_lines,
                        &pane.marks,
                        &pane.viewport,
                    );
                }
                break;
            }
        }

        // Render each pane's content (skip background image since we rendered it full-screen)
        for pane in panes {
            let separator_marks = compute_visible_separator_marks(
                &pane.marks,
                pane.scrollback_len,
                pane.scroll_offset,
                pane.grid_size.1,
            );
            self.cell_renderer.render_pane_to_view(
                &surface_view,
                &pane.viewport,
                pane.cells,
                pane.grid_size.0,
                pane.grid_size.1,
                pane.cursor_pos,
                pane.cursor_opacity,
                pane.show_scrollbar,
                false, // Don't clear - we already cleared the surface
                has_background_image || has_custom_shader, // Skip background if already rendered
                &separator_marks,
                pane.background.as_ref(),
            )?;
        }

        // Render inline graphics (Sixel/iTerm2/Kitty) for each pane, clipped to its bounds
        for pane in panes {
            if !pane.graphics.is_empty() {
                self.render_pane_sixel_graphics(
                    &surface_view,
                    &pane.viewport,
                    &pane.graphics,
                    pane.scroll_offset,
                    pane.scrollback_len,
                    pane.grid_size.1,
                )?;
            }
        }

        // Render dividers between panes
        if !dividers.is_empty() {
            self.render_dividers(&surface_view, dividers, divider_settings)?;
        }

        // Render pane title bars (background + text)
        if !pane_titles.is_empty() {
            self.render_pane_titles(&surface_view, pane_titles)?;
        }

        // Render focus indicator around focused pane (only if multiple panes)
        if panes.len() > 1
            && let Some(viewport) = focused_viewport
        {
            self.render_focus_indicator(&surface_view, viewport, divider_settings)?;
        }

        // Render egui overlay if provided
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }

        // Present the surface
        surface_texture.present();

        self.dirty = false;
        Ok(true)
    }

    /// Render pane dividers on top of pane content
    ///
    /// This should be called after rendering pane content but before egui.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `dividers` - List of dividers to render with hover state
    /// * `settings` - Divider appearance settings
    #[allow(dead_code)]
    pub fn render_dividers(
        &mut self,
        surface_view: &wgpu::TextureView,
        dividers: &[DividerRenderInfo],
        settings: &PaneDividerSettings,
    ) -> Result<()> {
        if dividers.is_empty() {
            return Ok(());
        }

        // Build divider instances using the cell renderer's background pipeline
        // We reuse the bg_instances buffer for dividers
        let mut instances = Vec::with_capacity(dividers.len() * 3); // Extra capacity for multi-rect styles

        let w = self.size.width as f32;
        let h = self.size.height as f32;

        for divider in dividers {
            let color = if divider.hovered {
                settings.hover_color
            } else {
                settings.divider_color
            };

            use par_term_config::DividerStyle;
            match settings.divider_style {
                DividerStyle::Solid => {
                    let x_ndc = divider.x / w * 2.0 - 1.0;
                    let y_ndc = 1.0 - (divider.y / h * 2.0);
                    let w_ndc = divider.width / w * 2.0;
                    let h_ndc = divider.height / h * 2.0;

                    instances.push(crate::cell_renderer::types::BackgroundInstance {
                        position: [x_ndc, y_ndc],
                        size: [w_ndc, h_ndc],
                        color: [color[0], color[1], color[2], 1.0],
                    });
                }
                DividerStyle::Double => {
                    // Two parallel lines with a visible gap between them
                    let is_horizontal = divider.width > divider.height;
                    let thickness = if is_horizontal {
                        divider.height
                    } else {
                        divider.width
                    };

                    if thickness >= 4.0 {
                        // Enough space for two 1px lines with visible gap
                        if is_horizontal {
                            // Top line
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            // Bottom line (gap in between shows background)
                            let bottom_y = divider.y + divider.height - 1.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (bottom_y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        } else {
                            // Left line
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            // Right line
                            let right_x = divider.x + divider.width - 1.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [right_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        }
                    } else {
                        // Divider too thin for double lines — render centered 1px line
                        // (visibly thinner than Solid to differentiate)
                        if is_horizontal {
                            let center_y = divider.y + (divider.height - 1.0) / 2.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (center_y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        } else {
                            let center_x = divider.x + (divider.width - 1.0) / 2.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [center_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        }
                    }
                }
                DividerStyle::Dashed => {
                    // Dashed line effect using segments
                    let is_horizontal = divider.width > divider.height;
                    let dash_len: f32 = 6.0;
                    let gap_len: f32 = 4.0;

                    if is_horizontal {
                        let mut x = divider.x;
                        while x < divider.x + divider.width {
                            let seg_w = dash_len.min(divider.x + divider.width - x);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [seg_w / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            x += dash_len + gap_len;
                        }
                    } else {
                        let mut y = divider.y;
                        while y < divider.y + divider.height {
                            let seg_h = dash_len.min(divider.y + divider.height - y);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (y / h * 2.0)],
                                size: [divider.width / w * 2.0, seg_h / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            y += dash_len + gap_len;
                        }
                    }
                }
                DividerStyle::Shadow => {
                    // Beveled/embossed effect — all rendering stays within divider bounds
                    // Highlight on top/left edge, shadow on bottom/right edge
                    let is_horizontal = divider.width > divider.height;
                    let thickness = if is_horizontal {
                        divider.height
                    } else {
                        divider.width
                    };

                    // Brighter highlight color
                    let highlight = [
                        (color[0] + 0.3).min(1.0),
                        (color[1] + 0.3).min(1.0),
                        (color[2] + 0.3).min(1.0),
                        1.0,
                    ];
                    // Darker shadow color
                    let shadow = [(color[0] * 0.3), (color[1] * 0.3), (color[2] * 0.3), 1.0];

                    if thickness >= 3.0 {
                        // 3+ px: highlight line / main body / shadow line
                        let edge = 1.0_f32;
                        if is_horizontal {
                            // Top highlight
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, edge / h * 2.0],
                                color: highlight,
                            });
                            // Main body (middle portion)
                            let body_y = divider.y + edge;
                            let body_h = divider.height - edge * 2.0;
                            if body_h > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [divider.x / w * 2.0 - 1.0, 1.0 - (body_y / h * 2.0)],
                                    size: [divider.width / w * 2.0, body_h / h * 2.0],
                                    color: [color[0], color[1], color[2], 1.0],
                                });
                            }
                            // Bottom shadow
                            let shadow_y = divider.y + divider.height - edge;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (shadow_y / h * 2.0)],
                                size: [divider.width / w * 2.0, edge / h * 2.0],
                                color: shadow,
                            });
                        } else {
                            // Left highlight
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [edge / w * 2.0, divider.height / h * 2.0],
                                color: highlight,
                            });
                            // Main body
                            let body_x = divider.x + edge;
                            let body_w = divider.width - edge * 2.0;
                            if body_w > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [body_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                    size: [body_w / w * 2.0, divider.height / h * 2.0],
                                    color: [color[0], color[1], color[2], 1.0],
                                });
                            }
                            // Right shadow
                            let shadow_x = divider.x + divider.width - edge;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [shadow_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [edge / w * 2.0, divider.height / h * 2.0],
                                color: shadow,
                            });
                        }
                    } else {
                        // 2px or less: top/left half highlight, bottom/right half shadow
                        if is_horizontal {
                            let half = (divider.height / 2.0).max(1.0);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, half / h * 2.0],
                                color: highlight,
                            });
                            let bottom_y = divider.y + half;
                            let bottom_h = divider.height - half;
                            if bottom_h > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [
                                        divider.x / w * 2.0 - 1.0,
                                        1.0 - (bottom_y / h * 2.0),
                                    ],
                                    size: [divider.width / w * 2.0, bottom_h / h * 2.0],
                                    color: shadow,
                                });
                            }
                        } else {
                            let half = (divider.width / 2.0).max(1.0);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [half / w * 2.0, divider.height / h * 2.0],
                                color: highlight,
                            });
                            let right_x = divider.x + half;
                            let right_w = divider.width - half;
                            if right_w > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [
                                        right_x / w * 2.0 - 1.0,
                                        1.0 - (divider.y / h * 2.0),
                                    ],
                                    size: [right_w / w * 2.0, divider.height / h * 2.0],
                                    color: shadow,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Write instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

        // Render dividers
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("divider render encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("divider render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.cell_renderer.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render focus indicator around a pane
    ///
    /// This draws a colored border around the focused pane to highlight it.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `viewport` - The focused pane's viewport
    /// * `settings` - Divider/focus settings
    #[allow(dead_code)]
    pub fn render_focus_indicator(
        &mut self,
        surface_view: &wgpu::TextureView,
        viewport: &PaneViewport,
        settings: &PaneDividerSettings,
    ) -> Result<()> {
        if !settings.show_focus_indicator {
            return Ok(());
        }

        let border_w = settings.focus_width;
        let color = [
            settings.focus_color[0],
            settings.focus_color[1],
            settings.focus_color[2],
            1.0,
        ];

        // Create 4 border rectangles (top, bottom, left, right)
        let instances = vec![
            // Top border
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - (viewport.y / self.size.height as f32 * 2.0),
                ],
                size: [
                    viewport.width / self.size.width as f32 * 2.0,
                    border_w / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Bottom border
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + viewport.height - border_w) / self.size.height as f32
                        * 2.0),
                ],
                size: [
                    viewport.width / self.size.width as f32 * 2.0,
                    border_w / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Left border (between top and bottom)
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + border_w) / self.size.height as f32 * 2.0),
                ],
                size: [
                    border_w / self.size.width as f32 * 2.0,
                    (viewport.height - border_w * 2.0) / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Right border (between top and bottom)
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    (viewport.x + viewport.width - border_w) / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + border_w) / self.size.height as f32 * 2.0),
                ],
                size: [
                    border_w / self.size.width as f32 * 2.0,
                    (viewport.height - border_w * 2.0) / self.size.height as f32 * 2.0,
                ],
                color,
            },
        ];

        // Write instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

        // Render focus indicator
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("focus indicator encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("focus indicator pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.cell_renderer.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render pane title bars (background rectangles + text)
    ///
    /// Title bars are rendered on top of pane content and dividers.
    /// Each title bar consists of a colored background rectangle and centered text.
    #[allow(dead_code)]
    pub fn render_pane_titles(
        &mut self,
        surface_view: &wgpu::TextureView,
        titles: &[PaneTitleInfo],
    ) -> Result<()> {
        if titles.is_empty() {
            return Ok(());
        }

        let width = self.size.width as f32;
        let height = self.size.height as f32;

        // Phase 1: Render title bar backgrounds
        let mut bg_instances = Vec::with_capacity(titles.len());
        for title in titles {
            let x_ndc = title.x / width * 2.0 - 1.0;
            let y_ndc = 1.0 - (title.y / height * 2.0);
            let w_ndc = title.width / width * 2.0;
            let h_ndc = title.height / height * 2.0;

            // Title bar must be fully opaque (alpha=1.0) to cover the background.
            // Differentiate focused/unfocused by lightening/darkening the color.
            let brightness = if title.focused { 1.0 } else { 0.7 };

            bg_instances.push(crate::cell_renderer::types::BackgroundInstance {
                position: [x_ndc, y_ndc],
                size: [w_ndc, h_ndc],
                color: [
                    title.bg_color[0] * brightness,
                    title.bg_color[1] * brightness,
                    title.bg_color[2] * brightness,
                    1.0, // Always fully opaque
                ],
            });
        }

        // Write background instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&bg_instances),
        );

        // Render title backgrounds
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane title bg encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane title bg pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.cell_renderer.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..bg_instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Phase 2: Render title text using glyph atlas
        let mut text_instances = Vec::new();
        let baseline_y = self.cell_renderer.font_ascent;

        for title in titles {
            let title_text = &title.title;
            if title_text.is_empty() {
                continue;
            }

            // Calculate starting X position (centered in title bar with left padding)
            let padding_x = 8.0;
            let mut x_pos = title.x + padding_x;
            let y_base = title.y + (title.height - self.cell_renderer.cell_height) / 2.0;

            let text_color = [
                title.text_color[0],
                title.text_color[1],
                title.text_color[2],
                if title.focused { 1.0 } else { 0.8 },
            ];

            // Truncate title if it would overflow the title bar
            let max_chars =
                ((title.width - padding_x * 2.0) / self.cell_renderer.cell_width) as usize;
            let display_text: String = if title_text.len() > max_chars && max_chars > 3 {
                let truncated: String = title_text.chars().take(max_chars - 1).collect();
                format!("{}\u{2026}", truncated) // ellipsis
            } else {
                title_text.clone()
            };

            for ch in display_text.chars() {
                if x_pos >= title.x + title.width - padding_x {
                    break;
                }

                if let Some((font_idx, glyph_id)) =
                    self.cell_renderer.font_manager.find_glyph(ch, false, false)
                {
                    let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                    // Check if this character should be rendered as a monochrome symbol
                    let force_monochrome = crate::cell_renderer::atlas::should_render_as_symbol(ch);
                    let info = if self.cell_renderer.glyph_cache.contains_key(&cache_key) {
                        self.cell_renderer.lru_remove(cache_key);
                        self.cell_renderer.lru_push_front(cache_key);
                        self.cell_renderer
                            .glyph_cache
                            .get(&cache_key)
                            .unwrap()
                            .clone()
                    } else if let Some(raster) =
                        self.cell_renderer
                            .rasterize_glyph(font_idx, glyph_id, force_monochrome)
                    {
                        let info = self.cell_renderer.upload_glyph(cache_key, &raster);
                        self.cell_renderer
                            .glyph_cache
                            .insert(cache_key, info.clone());
                        self.cell_renderer.lru_push_front(cache_key);
                        info
                    } else {
                        x_pos += self.cell_renderer.cell_width;
                        continue;
                    };

                    let glyph_left = x_pos + info.bearing_x;
                    let glyph_top = y_base + (baseline_y - info.bearing_y);

                    text_instances.push(crate::cell_renderer::types::TextInstance {
                        position: [
                            glyph_left / width * 2.0 - 1.0,
                            1.0 - (glyph_top / height * 2.0),
                        ],
                        size: [
                            info.width as f32 / width * 2.0,
                            info.height as f32 / height * 2.0,
                        ],
                        tex_offset: [info.x as f32 / 2048.0, info.y as f32 / 2048.0],
                        tex_size: [info.width as f32 / 2048.0, info.height as f32 / 2048.0],
                        color: text_color,
                        is_colored: if info.is_colored { 1 } else { 0 },
                    });
                }

                x_pos += self.cell_renderer.cell_width;
            }
        }

        if text_instances.is_empty() {
            return Ok(());
        }

        // Write text instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.text_instance_buffer,
            0,
            bytemuck::cast_slice(&text_instances),
        );

        // Render title text
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane title text encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane title text pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.text_pipeline);
            render_pass.set_bind_group(0, &self.cell_renderer.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cell_renderer.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.cell_renderer.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..text_instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Render egui overlay on top of the terminal
    fn render_egui(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
        egui_output: egui::FullOutput,
        egui_ctx: &egui::Context,
        force_opaque: bool,
    ) -> Result<()> {
        use wgpu::TextureViewDescriptor;

        // Create view of the surface texture
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        // Create command encoder for egui
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui encoder"),
                });

        // Convert egui output to screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: egui_output.pixels_per_point,
        };

        // Update egui textures
        for (id, image_delta) in &egui_output.textures_delta.set {
            self.egui_renderer.update_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                *id,
                image_delta,
            );
        }

        // Tessellate egui shapes into paint jobs
        let mut paint_jobs = egui_ctx.tessellate(egui_output.shapes, egui_output.pixels_per_point);

        // If requested, force all egui vertices to full opacity so UI stays solid
        if force_opaque {
            for job in paint_jobs.iter_mut() {
                match &mut job.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => {
                        for v in mesh.vertices.iter_mut() {
                            v.color[3] = 255;
                        }
                    }
                    egui::epaint::Primitive::Callback(_) => {}
                }
            }
        }

        // Update egui buffers
        self.egui_renderer.update_buffers(
            self.cell_renderer.device(),
            self.cell_renderer.queue(),
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render egui on top of the terminal content
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top of terminal
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Convert to 'static lifetime as required by egui_renderer.render()
            let mut render_pass = render_pass.forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        } // render_pass dropped here

        // Submit egui commands
        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Free egui textures
        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        Ok(())
    }

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
        if (self.cell_renderer.egui_bottom_inset - physical_inset).abs() > f32::EPSILON {
            self.cell_renderer.egui_bottom_inset = physical_inset;
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
        self.cell_renderer.egui_right_inset = physical_inset;
    }

    /// Check if a point (in pixel coordinates) is within the scrollbar bounds
    ///
    /// # Arguments
    /// * `x` - X coordinate in pixels (from left edge)
    /// * `y` - Y coordinate in pixels (from top edge)
    pub fn scrollbar_contains_point(&self, x: f32, y: f32) -> bool {
        self.cell_renderer.scrollbar_contains_point(x, y)
    }

    /// Get the scrollbar thumb bounds (top Y, height) in pixels
    pub fn scrollbar_thumb_bounds(&self) -> Option<(f32, f32)> {
        self.cell_renderer.scrollbar_thumb_bounds()
    }

    /// Check if an X coordinate is within the scrollbar track
    pub fn scrollbar_track_contains_x(&self, x: f32) -> bool {
        self.cell_renderer.scrollbar_track_contains_x(x)
    }

    /// Convert a mouse Y position to a scroll offset
    ///
    /// # Arguments
    /// * `mouse_y` - Mouse Y coordinate in pixels (from top edge)
    ///
    /// # Returns
    /// The scroll offset corresponding to the mouse position, or None if scrollbar is not visible
    pub fn scrollbar_mouse_y_to_scroll_offset(&self, mouse_y: f32) -> Option<usize> {
        self.cell_renderer
            .scrollbar_mouse_y_to_scroll_offset(mouse_y)
    }

    /// Find a scrollbar mark at the given mouse position for tooltip display.
    ///
    /// # Arguments
    /// * `mouse_x` - Mouse X coordinate in pixels
    /// * `mouse_y` - Mouse Y coordinate in pixels
    /// * `tolerance` - Maximum distance in pixels to match a mark
    ///
    /// # Returns
    /// The mark at that position, or None if no mark is within tolerance
    pub fn scrollbar_mark_at_position(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        tolerance: f32,
    ) -> Option<&par_term_config::ScrollbackMark> {
        self.cell_renderer
            .scrollbar_mark_at_position(mouse_x, mouse_y, tolerance)
    }

    /// Check if the renderer needs to be redrawn
    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the renderer as dirty, forcing a redraw on next render call
    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Set debug overlay text to be rendered
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn render_debug_overlay(&mut self, text: &str) {
        self.debug_text = Some(text.to_string());
        self.dirty = true; // Mark dirty to ensure debug overlay renders
    }

    /// Reconfigure the surface (call when surface becomes outdated or lost)
    /// This typically happens when dragging the window between displays
    pub fn reconfigure_surface(&mut self) {
        self.cell_renderer.reconfigure_surface();
        self.dirty = true;
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: par_term_config::VsyncMode) -> bool {
        self.cell_renderer.is_vsync_mode_supported(mode)
    }

    /// Update the vsync mode. Returns the actual mode applied (may differ if requested mode unsupported).
    /// Also returns whether the mode was changed.
    pub fn update_vsync_mode(
        &mut self,
        mode: par_term_config::VsyncMode,
    ) -> (par_term_config::VsyncMode, bool) {
        let result = self.cell_renderer.update_vsync_mode(mode);
        if result.1 {
            self.dirty = true;
        }
        result
    }

    /// Get the current vsync mode
    #[allow(dead_code)]
    pub fn current_vsync_mode(&self) -> par_term_config::VsyncMode {
        self.cell_renderer.current_vsync_mode()
    }

    /// Clear the glyph cache to force re-rasterization
    /// Useful after display changes where font rendering may differ
    pub fn clear_glyph_cache(&mut self) {
        self.cell_renderer.clear_glyph_cache();
        self.dirty = true;
    }

    /// Update font anti-aliasing setting
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_antialias(&mut self, enabled: bool) -> bool {
        let changed = self.cell_renderer.update_font_antialias(enabled);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Update font hinting setting
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_hinting(&mut self, enabled: bool) -> bool {
        let changed = self.cell_renderer.update_font_hinting(enabled);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Update thin strokes mode
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_thin_strokes(&mut self, mode: par_term_config::ThinStrokesMode) -> bool {
        let changed = self.cell_renderer.update_font_thin_strokes(mode);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Update minimum contrast ratio
    /// Returns true if the setting changed (requiring redraw)
    pub fn update_minimum_contrast(&mut self, ratio: f32) -> bool {
        let changed = self.cell_renderer.update_minimum_contrast(ratio);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Pause shader animations (e.g., when window loses focus)
    /// This reduces GPU usage when the terminal is not actively being viewed
    pub fn pause_shader_animations(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(false);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_animation_enabled(false);
        }
        log::info!("[SHADER] Shader animations paused");
    }

    /// Resume shader animations (e.g., when window regains focus)
    /// Only resumes if the user's config has animation enabled
    pub fn resume_shader_animations(
        &mut self,
        custom_shader_animation: bool,
        cursor_shader_animation: bool,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(custom_shader_animation);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_animation_enabled(cursor_shader_animation);
        }
        self.dirty = true;
        log::info!(
            "[SHADER] Shader animations resumed (custom: {}, cursor: {})",
            custom_shader_animation,
            cursor_shader_animation
        );
    }

    /// Take a screenshot of the current terminal content
    /// Returns an RGBA image that can be saved to disk
    ///
    /// This captures the fully composited output including shader effects.
    pub fn take_screenshot(&mut self) -> Result<image::RgbaImage> {
        log::info!(
            "take_screenshot: Starting screenshot capture ({}x{})",
            self.size.width,
            self.size.height
        );

        let width = self.size.width;
        let height = self.size.height;
        // Use the same format as the surface to match pipeline expectations
        let format = self.cell_renderer.surface_format();
        log::info!("take_screenshot: Using texture format {:?}", format);

        // Create a texture to render the final composited output to (with COPY_SRC for reading back)
        let screenshot_texture =
            self.cell_renderer
                .device()
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("screenshot texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });

        let screenshot_view =
            screenshot_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Render the full composited frame (cells + shaders + overlays)
        log::info!("take_screenshot: Rendering composited frame...");

        // Check if shaders are enabled
        let has_custom_shader = self.custom_shader_renderer.is_some();
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

        if has_custom_shader {
            // Render cells to the custom shader's intermediate texture
            let intermediate_view = self
                .custom_shader_renderer
                .as_ref()
                .unwrap()
                .intermediate_texture_view()
                .clone();
            self.cell_renderer
                .render_to_texture(&intermediate_view, true)?;

            if use_cursor_shader {
                // Background shader renders to cursor shader's intermediate texture
                let cursor_intermediate = self
                    .cursor_shader_renderer
                    .as_ref()
                    .unwrap()
                    .intermediate_texture_view()
                    .clone();
                self.custom_shader_renderer.as_mut().unwrap().render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &cursor_intermediate,
                    false,
                )?;
                // Cursor shader renders to screenshot texture
                self.cursor_shader_renderer.as_mut().unwrap().render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &screenshot_view,
                    true,
                )?;
            } else {
                // Background shader renders directly to screenshot texture
                self.custom_shader_renderer.as_mut().unwrap().render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &screenshot_view,
                    true,
                )?;
            }
        } else if use_cursor_shader {
            // Render cells to cursor shader's intermediate texture
            let cursor_intermediate = self
                .cursor_shader_renderer
                .as_ref()
                .unwrap()
                .intermediate_texture_view()
                .clone();
            self.cell_renderer
                .render_to_texture(&cursor_intermediate, true)?;
            // Cursor shader renders to screenshot texture
            self.cursor_shader_renderer.as_mut().unwrap().render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &screenshot_view,
                true,
            )?;
        } else {
            // No shaders - render directly to screenshot texture
            self.cell_renderer.render_to_view(&screenshot_view)?;
        }

        log::info!("take_screenshot: Render complete");

        // Get device and queue references for buffer operations
        let device = self.cell_renderer.device();
        let queue = self.cell_renderer.queue();

        // Create buffer for reading back the texture
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        // wgpu requires rows to be aligned to 256 bytes
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("screenshot encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &screenshot_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));
        log::info!("take_screenshot: Texture copy submitted");

        // Map the buffer and read the data
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Wait for GPU to finish
        log::info!("take_screenshot: Waiting for GPU...");
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        log::info!("take_screenshot: GPU poll complete, waiting for buffer map...");
        rx.recv()
            .map_err(|e| anyhow::anyhow!("Failed to receive map result: {}", e))?
            .map_err(|e| anyhow::anyhow!("Failed to map buffer: {:?}", e))?;
        log::info!("take_screenshot: Buffer mapped successfully");

        // Read the data
        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);

        // Check if format is BGRA (needs swizzle) or RGBA (direct copy)
        let is_bgra = matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Copy data row by row (to handle padding)
        for y in 0..height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (width * bytes_per_pixel) as usize;
            let row = &data[row_start..row_end];

            if is_bgra {
                // Convert BGRA to RGBA
                for chunk in row.chunks(4) {
                    pixels.push(chunk[2]); // R (was B)
                    pixels.push(chunk[1]); // G
                    pixels.push(chunk[0]); // B (was R)
                    pixels.push(chunk[3]); // A
                }
            } else {
                // Already RGBA, direct copy
                pixels.extend_from_slice(row);
            }
        }

        drop(data);
        output_buffer.unmap();

        // Create image
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from pixel data"))
    }
}
