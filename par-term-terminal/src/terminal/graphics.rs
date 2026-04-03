use super::TerminalManager;
use par_term_emu_core_rust::graphics::TerminalGraphic;
use std::collections::HashSet;

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

    /// Mark the terminal as clean (reset dirty row tracking).
    ///
    /// Call this at the end of each render frame so that `get_dirty_rows()`
    /// returns only rows modified since the last frame.
    pub fn mark_clean(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.mark_clean();
    }

    /// Get the set of rows that have been modified since the last `mark_clean()` call.
    pub fn get_dirty_rows(&self) -> Vec<usize> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_dirty_rows()
    }

    /// Remove graphics whose rows have been overwritten by cell writes.
    ///
    /// This detects the case where a program (e.g. tmux) redraws cells on top
    /// of an inline graphic without sending ED 2 (erase display).  The graphics
    /// protocol handlers do NOT mark rows dirty, so if a graphic's row appears
    /// in the dirty set it means a regular cell write has overwritten it.
    ///
    /// Three-layer protection prevents false positives:
    ///
    /// 1. **Scroll detection** — if scrollback length changed this frame,
    ///    dirty rows are from scrolling (which already repositions graphics
    ///    via `adjust_for_scroll_up_with_scrollback`), not overwrites.
    ///
    /// 2. **Time-based grace period** (500 ms) — graphics are immune to
    ///    invalidation for a short window after first appearing.  This
    ///    survives tmux's post-command pane redraw (which happens within
    ///    milliseconds) without removing newly-placed images.
    ///
    /// 3. **Per-frame dirty-row threshold** — after the grace period, a
    ///    graphic is only removed when a majority of its rows are dirtied
    ///    in a single frame, distinguishing a full-screen redraw (tmux
    ///    split/clear) from normal 1-2 row terminal activity.
    ///
    /// Returns the number of graphics removed.
    pub fn invalidate_overwritten_graphics(&self) -> usize {
        use std::time::{Duration, Instant};

        /// Graphics are immune to dirty-row invalidation for this duration
        /// after first appearing.  Survives tmux post-command pane redraws.
        const GRACE_PERIOD: Duration = Duration::from_millis(500);

        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        let dirty: HashSet<usize> = term.get_dirty_rows().into_iter().collect();
        term.mark_clean();

        // Detect scrolling: if scrollback length changed, the dirty rows are
        // from scroll (which already repositions graphics), not overwrites.
        let current_scrollback = term.active_grid().scrollback_len();
        let mut prev_sb = self.prev_scrollback_len.lock();
        let scrolled = current_scrollback != *prev_sb;
        *prev_sb = current_scrollback;
        drop(prev_sb);

        let graphics = term.all_graphics();
        let now = Instant::now();

        // Update tracking: register new graphics, prune stale entries.
        let current_ids: HashSet<u64> = graphics.iter().map(|g| g.id).collect();
        let mut known = self.known_graphic_times.lock();
        for id in &current_ids {
            known.entry(*id).or_insert(now);
        }
        known.retain(|id, _| current_ids.contains(id));

        if graphics.is_empty() || dirty.is_empty() || scrolled {
            return 0;
        }

        let to_remove: Vec<u64> = graphics
            .iter()
            .filter(|g| {
                // Time-based grace period.
                let first_seen = known.get(&g.id).copied().unwrap_or(now);
                if now.duration_since(first_seen) < GRACE_PERIOD {
                    return false;
                }
                // Dirty-row threshold over the graphic's row span.
                let (_, row) = g.position;
                let cell_height: usize = g.cell_dimensions.map(|(_, h)| h as usize).unwrap_or(16);
                let height_in_rows: usize = if cell_height > 0 {
                    g.height.div_ceil(cell_height)
                } else {
                    1
                };
                let dirty_count = (row..row + height_in_rows)
                    .filter(|r| dirty.contains(r))
                    .count();
                // Small graphics: all rows must be dirty.
                // Larger graphics: >50% (min 3).
                let threshold = if height_in_rows <= 4 {
                    height_in_rows
                } else {
                    height_in_rows.div_ceil(2).max(3)
                };
                dirty_count >= threshold
            })
            .map(|g| g.id)
            .collect();

        let removed = to_remove.len();
        for id in &to_remove {
            term.graphics_store_mut().remove_graphic(*id);
            known.remove(id);
        }

        if removed > 0 {
            log::debug!(
                "invalidate_overwritten_graphics: removed {} stale graphics ({} dirty rows)",
                removed,
                dirty.len(),
            );
        }

        removed
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
