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
    // Shaping options
    #[allow(dead_code)] // Config stored for future text shaping pipeline integration
    pub(crate) enable_text_shaping: bool,
    pub(crate) enable_ligatures: bool,
    pub(crate) enable_kerning: bool,
    // Rendering options
    /// Enable anti-aliasing for font rendering
    pub(crate) font_antialias: bool,
    /// Enable hinting for font rendering
    pub(crate) font_hinting: bool,
    /// Thin strokes mode for font rendering
    pub(crate) font_thin_strokes: par_term_config::ThinStrokesMode,
    /// Minimum contrast ratio for text against background (WCAG standard)
    /// 1.0 = disabled, 4.5 = WCAG AA, 7.0 = WCAG AAA
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

    /// Update minimum contrast ratio.
    /// Returns true if the setting changed (requiring redraw).
    pub fn update_minimum_contrast(&mut self, ratio: f32) -> bool {
        // Clamp to valid range: 1.0 (disabled) to 21.0 (max possible contrast)
        let ratio = ratio.clamp(1.0, 21.0);
        if (self.font.minimum_contrast - ratio).abs() > CONTRAST_CHANGE_EPSILON {
            self.font.minimum_contrast = ratio;
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Adjust foreground color to meet minimum contrast ratio against background.
    /// Uses WCAG luminance formula for accurate contrast calculation.
    /// Returns the adjusted color [R, G, B, A] with preserved alpha.
    pub(crate) fn ensure_minimum_contrast(&self, fg: [f32; 4], bg: [f32; 4]) -> [f32; 4] {
        // If minimum_contrast is 1.0 (disabled) or less, no adjustment needed
        if self.font.minimum_contrast <= 1.0 {
            return fg;
        }

        // Calculate luminance using WCAG formula
        fn luminance(color: [f32; 4]) -> f32 {
            let r = color[0].powf(2.2);
            let g = color[1].powf(2.2);
            let b = color[2].powf(2.2);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        }

        fn contrast_ratio(l1: f32, l2: f32) -> f32 {
            let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
            (lighter + 0.05) / (darker + 0.05)
        }

        let fg_lum = luminance(fg);
        let bg_lum = luminance(bg);
        let current_ratio = contrast_ratio(fg_lum, bg_lum);

        // If already meets minimum contrast, return unchanged
        if current_ratio >= self.font.minimum_contrast {
            return fg;
        }

        // Determine if we need to lighten or darken the foreground
        // If background is dark, lighten fg; if light, darken fg
        let bg_is_dark = bg_lum < DARK_BACKGROUND_THRESHOLD;

        // Binary search for the minimum adjustment needed
        let mut low = 0.0f32;
        let mut high = 1.0f32;

        for _ in 0..20 {
            // 20 iterations gives ~1/1_000_000 precision
            let mid = (low + high) / 2.0;

            let adjusted = if bg_is_dark {
                // Lighten: mix with white
                [
                    fg[0] + (1.0 - fg[0]) * mid,
                    fg[1] + (1.0 - fg[1]) * mid,
                    fg[2] + (1.0 - fg[2]) * mid,
                    fg[3],
                ]
            } else {
                // Darken: mix with black
                [
                    fg[0] * (1.0 - mid),
                    fg[1] * (1.0 - mid),
                    fg[2] * (1.0 - mid),
                    fg[3],
                ]
            };

            let adjusted_lum = luminance(adjusted);
            let new_ratio = contrast_ratio(adjusted_lum, bg_lum);

            if new_ratio >= self.font.minimum_contrast {
                high = mid;
            } else {
                low = mid;
            }
        }

        // Apply the final adjustment
        if bg_is_dark {
            [
                fg[0] + (1.0 - fg[0]) * high,
                fg[1] + (1.0 - fg[1]) * high,
                fg[2] + (1.0 - fg[2]) * high,
                fg[3],
            ]
        } else {
            [
                fg[0] * (1.0 - high),
                fg[1] * (1.0 - high),
                fg[2] * (1.0 - high),
                fg[3],
            ]
        }
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
