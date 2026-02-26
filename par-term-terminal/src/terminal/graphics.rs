use super::TerminalManager;
use par_term_emu_core_rust::graphics::TerminalGraphic;

impl TerminalManager {
    /// Get all graphics (Sixel, iTerm2, Kitty)
    /// Returns a vector of cloned TerminalGraphic objects for rendering
    pub fn get_graphics(&self) -> Vec<TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let graphics: Vec<_> = term.all_graphics().to_vec();
        if !graphics.is_empty() {
            log::debug!("Returning {} graphics from core library", graphics.len());
            for (i, g) in graphics.iter().enumerate() {
                log::trace!(
                    "  [{}] protocol={:?}, pos=({},{}), size={}x{}",
                    i,
                    g.protocol,
                    g.position.0,
                    g.position.1,
                    g.width,
                    g.height
                );
            }
        }
        graphics
    }

    /// Get graphics at a specific row
    pub fn get_graphics_at_row(&self, row: usize) -> Vec<TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.graphics_at_row(row)
            .iter()
            .map(|g| (*g).clone())
            .collect()
    }

    /// Get total graphics count
    pub fn graphics_count(&self) -> usize {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.graphics_count()
    }

    /// Get all scrollback graphics
    pub fn get_scrollback_graphics(&self) -> Vec<TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.all_scrollback_graphics().to_vec()
    }

    /// Update animations and return true if any frames changed
    pub fn update_animations(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        let changed_images = term.graphics_store_mut().update_animations();
        !changed_images.is_empty()
    }

    /// Get all graphics with current animation frames
    pub fn get_graphics_with_animations(&self) -> Vec<TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        let mut graphics = Vec::new();

        let base_graphics: Vec<_> = term.all_graphics().to_vec();

        log::debug!(
            "get_graphics_with_animations() - base_graphics count: {}",
            base_graphics.len()
        );

        for (idx, graphic) in base_graphics.iter().enumerate() {
            log::trace!(
                "Processing graphic {} - pos=({},{}), size={}x{}, kitty_id={:?}",
                idx,
                graphic.position.0,
                graphic.position.1,
                graphic.width,
                graphic.height,
                graphic.kitty_image_id
            );

            if let Some(image_id) = graphic.kitty_image_id
                && let Some(anim) = term.graphics_store().get_animation(image_id)
                && let Some(current_frame) = anim.current_frame()
            {
                let mut animated_graphic = graphic.clone();
                animated_graphic.pixels = current_frame.pixels.clone();
                animated_graphic.width = current_frame.width;
                animated_graphic.height = current_frame.height;

                log::debug!(
                    "Using animated frame {} for image {}",
                    anim.current_frame,
                    image_id
                );

                graphics.push(animated_graphic);
                continue;
            }
            log::trace!("Using static graphic {}", idx);
            graphics.push(graphic.clone());
        }

        log::debug!("Returning {} graphics total", graphics.len());
        graphics
    }
}
