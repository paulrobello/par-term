//! Per-frame render decisions and reusable scratch buffers.
//!
//! Distinct from `DebugState`: everything here **drives rendering**. `cache_hit`
//! gates the GPU cell upload, URL detection, and the cache flush, so it must not
//! live alongside the F3 overlay's timing metrics where a future
//! "compile the FPS overlay out" change would take rendering with it.

use std::sync::Arc;

use crate::config::Cell;

/// Rendering decisions and scratch storage for the current frame.
pub(crate) struct FrameState {
    /// Whether this frame reused the cached cell buffer (no new terminal output).
    ///
    /// Load-bearing, not diagnostic: on a hit the renderer skips `update_cells`,
    /// URL detection is skipped, and `flush_cell_cache` is a no-op.
    pub(crate) cache_hit: bool,

    /// Persistent full-grid buffer holding the focused pane's cells with the
    /// transient text overlays (search highlights, URL underlines) applied.
    ///
    /// Reused across frames so refreshing it is a `Vec::clone_from` — which
    /// reuses each `Cell`'s existing `grapheme` allocation (see the hand-written
    /// `Clone for Cell`) — rather than a fresh full-grid deep clone. Held as an
    /// `Arc` because the pane render data owns its cells as one; the render call
    /// drops its clone at the end of each frame, so the refcount is back to 1
    /// before the next refresh and `Arc::make_mut` does not copy.
    pub(crate) overlay_scratch: Arc<Vec<Cell>>,
}

impl Default for FrameState {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameState {
    /// Create a `FrameState` with no cached frame and an empty scratch buffer.
    pub(crate) fn new() -> Self {
        Self {
            cache_hit: false,
            overlay_scratch: Arc::new(Vec::new()),
        }
    }

    /// Refresh the scratch buffer from `source` and return it for mutation.
    ///
    /// Reuses the existing element allocations; the returned buffer is a
    /// content-identical copy of `source` that the caller may freely mutate.
    pub(crate) fn refresh_overlay_scratch(&mut self, source: &[Cell]) -> &mut Vec<Cell> {
        let scratch = Arc::make_mut(&mut self.overlay_scratch);
        // `Vec::clone_from` takes a `&Vec` and the caller holds a slice, so
        // mirror it here: `clone_from` the overlapping prefix (reusing each
        // cell's existing `String` buffer), then fix up the length difference.
        let overlap = scratch.len().min(source.len());
        for (dst, src) in scratch[..overlap].iter_mut().zip(&source[..overlap]) {
            dst.clone_from(src);
        }
        if source.len() > overlap {
            scratch.extend_from_slice(&source[overlap..]);
        } else {
            scratch.truncate(source.len());
        }
        scratch
    }
}

#[cfg(test)]
mod tests {
    use super::FrameState;
    use crate::config::Cell;

    fn cells(graphemes: &[&str]) -> Vec<Cell> {
        graphemes
            .iter()
            .map(|g| Cell {
                grapheme: (*g).to_string(),
                ..Cell::default()
            })
            .collect()
    }

    #[test]
    fn refresh_matches_the_source_contents() {
        let mut state = FrameState::new();

        assert_eq!(
            state.refresh_overlay_scratch(&cells(&["a", "b"])),
            &cells(&["a", "b"])
        );
        // Growing.
        assert_eq!(
            state.refresh_overlay_scratch(&cells(&["a", "b", "c"])),
            &cells(&["a", "b", "c"])
        );
        // Shrinking.
        assert_eq!(
            state.refresh_overlay_scratch(&cells(&["z"])),
            &cells(&["z"])
        );
    }

    #[test]
    fn refresh_reuses_the_backing_allocations_across_frames() {
        let mut state = FrameState::new();
        state.refresh_overlay_scratch(&cells(&["placeholder", "placeholder"]));
        let before: Vec<*const u8> = state
            .overlay_scratch
            .iter()
            .map(|c| c.grapheme.as_ptr())
            .collect();

        state.refresh_overlay_scratch(&cells(&["x", "y"]));
        let after: Vec<*const u8> = state
            .overlay_scratch
            .iter()
            .map(|c| c.grapheme.as_ptr())
            .collect();

        assert_eq!(
            before, after,
            "a same-size refresh must not reallocate any grapheme"
        );
    }

    #[test]
    fn mutating_the_scratch_does_not_disturb_a_handed_out_clone() {
        let mut state = FrameState::new();
        state.refresh_overlay_scratch(&cells(&["a"]));
        let handed_out = std::sync::Arc::clone(&state.overlay_scratch);

        // With a live clone outstanding, `Arc::make_mut` must copy rather than
        // mutate through — this is what keeps the render path's borrow sound.
        state.refresh_overlay_scratch(&cells(&["b"]));

        assert_eq!(handed_out.as_slice(), cells(&["a"]).as_slice());
        assert_eq!(state.overlay_scratch.as_slice(), cells(&["b"]).as_slice());
    }
}
