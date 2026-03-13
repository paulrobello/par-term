use super::CellRenderer;

/// Font configuration (base values), scaled metrics, shaping, and rendering options.
pub(crate) struct FontState {
    // Base configuration (before scale factor)
    pub(crate) base_font_size: f32,
    pub(crate) line_spacing: f32,
    pub(crate) char_spacing: f32,
    // Scaled metrics (scaled by current scale_factor)
    pub(crate) font_ascent: f32,
    pub(crate) font_descent: f32,
    pub(crate) font_leading: f32,
    pub(crate) font_size_pixels: f32,
    pub(crate) char_advance: f32,
    // Shaping option flags — written from config but not yet consumed by the cell
    // renderer itself. The active shaping pipeline lives in `par-term-fonts::TextShaper`
    // which reads these values via `CellRendererParams`. These fields are retained here
    // so the renderer can pass them down when a future direct-shaping path is added.
    #[allow(dead_code)] // Config stored for future direct text shaping pipeline integration
    pub(crate) enable_text_shaping: bool,
    #[allow(dead_code)] // Config stored for future direct text shaping pipeline integration
    pub(crate) enable_ligatures: bool,
    #[allow(dead_code)] // Config stored for future direct text shaping pipeline integration
    pub(crate) enable_kerning: bool,
    // Rendering options
    /// Enable anti-aliasing for font rendering
    pub(crate) font_antialias: bool,
    /// Enable hinting for font rendering
    pub(crate) font_hinting: bool,
    /// Thin strokes mode for font rendering
    pub(crate) font_thin_strokes: par_term_config::ThinStrokesMode,
    /// Minimum contrast between text and background (iTerm2-compatible)
    /// 0.0 = disabled, values near 1.0 = nearly black & white
    pub(crate) minimum_contrast: f32,
}

/// Threshold below which the background is considered "dark" for contrast purposes.
const DARK_BACKGROUND_THRESHOLD: f32 = 0.5;

/// Minimum contrast ratio change that triggers a re-render of all rows.
/// Changes smaller than this are ignored to avoid unnecessary redraws.
const CONTRAST_CHANGE_EPSILON: f32 = 0.001;

impl CellRenderer {
    /// Update font anti-aliasing setting.
    /// Returns true if the setting changed (requiring glyph cache clear).
    pub fn update_font_antialias(&mut self, enabled: bool) -> bool {
        if self.font.font_antialias != enabled {
            self.font.font_antialias = enabled;
            self.clear_glyph_cache();
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Update font hinting setting.
    /// Returns true if the setting changed (requiring glyph cache clear).
    pub fn update_font_hinting(&mut self, enabled: bool) -> bool {
        if self.font.font_hinting != enabled {
            self.font.font_hinting = enabled;
            self.clear_glyph_cache();
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Update thin strokes mode.
    /// Returns true if the setting changed (requiring glyph cache clear).
    pub fn update_font_thin_strokes(&mut self, mode: par_term_config::ThinStrokesMode) -> bool {
        if self.font.font_thin_strokes != mode {
            self.font.font_thin_strokes = mode;
            self.clear_glyph_cache();
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Update minimum contrast value.
    /// Returns true if the setting changed (requiring redraw).
    pub fn update_minimum_contrast(&mut self, value: f32) -> bool {
        // Clamp to valid range: 0.0 (disabled) to 1.0 (max contrast)
        let value = value.clamp(0.0, 1.0);
        if (self.font.minimum_contrast - value).abs() > CONTRAST_CHANGE_EPSILON {
            self.font.minimum_contrast = value;
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Adjust foreground color to meet minimum contrast against background.
    /// Uses iTerm2-compatible perceived brightness algorithm:
    /// brightness = 0.30*R + 0.59*G + 0.11*B
    /// Ensures the absolute brightness difference between fg and bg meets the threshold.
    /// Returns the adjusted color [R, G, B, A] with preserved alpha.
    pub(crate) fn ensure_minimum_contrast(&self, fg: [f32; 4], bg: [f32; 4]) -> [f32; 4] {
        let min_contrast = self.font.minimum_contrast;
        // If minimum_contrast is 0.0 (disabled) or negligible, no adjustment needed
        if min_contrast <= 0.0 {
            return fg;
        }

        /// Perceived brightness using iTerm2's coefficients (BT.601 luma).
        fn perceived_brightness(r: f32, g: f32, b: f32) -> f32 {
            0.30 * r + 0.59 * g + 0.11 * b
        }

        let fg_brightness = perceived_brightness(fg[0], fg[1], fg[2]);
        let bg_brightness = perceived_brightness(bg[0], bg[1], bg[2]);
        let brightness_diff = (fg_brightness - bg_brightness).abs();

        // If already meets minimum contrast, return unchanged
        if brightness_diff >= min_contrast {
            return fg;
        }

        // Need to adjust. Determine target brightness.
        let error = min_contrast - brightness_diff;
        let mut target_brightness = if fg_brightness < bg_brightness {
            // fg is darker — try to make it even darker
            fg_brightness - error
        } else {
            // fg is brighter — try to make it even brighter
            fg_brightness + error
        };

        // If target is out of range, try the opposite direction
        if target_brightness < 0.0 {
            let alternative = bg_brightness + min_contrast;
            let base_contrast = bg_brightness;
            let alt_contrast = alternative.min(1.0) - bg_brightness;
            if alt_contrast > base_contrast {
                target_brightness = alternative;
            }
        } else if target_brightness > 1.0 {
            let alternative = bg_brightness - min_contrast;
            let base_contrast = 1.0 - bg_brightness;
            let alt_contrast = bg_brightness - alternative.max(0.0);
            if alt_contrast > base_contrast {
                target_brightness = alternative;
            }
        }

        target_brightness = target_brightness.clamp(0.0, 1.0);

        // Interpolate from current color toward black (k=0) or white (k=1)
        // to reach target brightness. Solve for parameter p analytically.
        let k: f32 = if fg_brightness < target_brightness {
            1.0 // move toward white
        } else {
            0.0 // move toward black
        };

        let denom = perceived_brightness(k - fg[0], k - fg[1], k - fg[2]);
        let p = if denom.abs() < 1e-10 {
            0.0
        } else {
            ((target_brightness - perceived_brightness(fg[0], fg[1], fg[2])) / denom)
                .clamp(0.0, 1.0)
        };

        [
            p * k + (1.0 - p) * fg[0],
            p * k + (1.0 - p) * fg[1],
            p * k + (1.0 - p) * fg[2],
            fg[3],
        ]
    }

    /// Check if thin strokes should be applied based on current mode and context.
    pub(crate) fn should_use_thin_strokes(&self) -> bool {
        use par_term_config::ThinStrokesMode;

        // Check if we're on a Retina/HiDPI display (scale factor > 1.5)
        let is_retina = self.scale_factor > 1.5;

        // Check if background is dark (average < 128)
        let bg_brightness =
            (self.background_color[0] + self.background_color[1] + self.background_color[2]) / 3.0;
        let is_dark_background = bg_brightness < DARK_BACKGROUND_THRESHOLD;

        match self.font.font_thin_strokes {
            ThinStrokesMode::Never => false,
            ThinStrokesMode::Always => true,
            ThinStrokesMode::RetinaOnly => is_retina,
            ThinStrokesMode::DarkBackgroundsOnly => is_dark_background,
            ThinStrokesMode::RetinaDarkBackgroundsOnly => is_retina && is_dark_background,
        }
    }
}
