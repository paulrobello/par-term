//! Per-pane state accessor routing for `Tab`.
//!
//! R-32: All four pairs of fallback fields (`scroll_state`, `mouse`, `bell`, `cache`)
//! have been removed from `Tab`.  `pane_manager` is always `Some` (initialised in
//! `Tab::new_internal` with a single primary pane that wraps `Tab::terminal`).
//!
//! All external callers should use these methods rather than any field directly.
//! Each accessor panics if the pane manager is unexpectedly absent (which cannot
//! happen in a correctly constructed `Tab`).

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::scroll_state::ScrollState;
use crate::tab::Tab;

impl Tab {
    /// Get the mouse state for selection operations.
    ///
    /// Returns the focused pane's mouse state. In split-pane mode this isolates
    /// selection coordinates per-pane; in single-pane mode the primary pane is
    /// always the focused one.
    pub(crate) fn selection_mouse(&self) -> &MouseState {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane())
            .map(|pane| &pane.mouse)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Get mutable mouse state for selection operations.
    ///
    /// Returns the focused pane's mouse state.
    pub(crate) fn selection_mouse_mut(&mut self) -> &mut MouseState {
        self.pane_manager
            .as_mut()
            .and_then(|pm| pm.focused_pane_mut())
            .map(|pane| &mut pane.mouse)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    // =========================================================================
    // R-32: Per-pane state accessors — no fallback branches.
    //
    // `pane_manager` is always `Some` after R-32; each accessor panics on
    // None to surface construction bugs early rather than silently using stale
    // state.
    // =========================================================================

    /// Active scroll state — focused pane.
    #[inline]
    pub(crate) fn active_scroll_state(&self) -> &ScrollState {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane())
            .map(|pane| &pane.scroll_state)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Mutable active scroll state — focused pane.
    #[inline]
    pub(crate) fn active_scroll_state_mut(&mut self) -> &mut ScrollState {
        self.pane_manager
            .as_mut()
            .and_then(|pm| pm.focused_pane_mut())
            .map(|pane| &mut pane.scroll_state)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Active mouse state — focused pane.
    #[inline]
    pub(crate) fn active_mouse(&self) -> &MouseState {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane())
            .map(|pane| &pane.mouse)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Mutable active mouse state — focused pane.
    #[inline]
    pub(crate) fn active_mouse_mut(&mut self) -> &mut MouseState {
        self.pane_manager
            .as_mut()
            .and_then(|pm| pm.focused_pane_mut())
            .map(|pane| &mut pane.mouse)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Active render cache — focused pane.
    #[inline]
    pub(crate) fn active_cache(&self) -> &RenderCache {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane())
            .map(|pane| &pane.cache)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Mutable active render cache — focused pane.
    #[inline]
    pub(crate) fn active_cache_mut(&mut self) -> &mut RenderCache {
        self.pane_manager
            .as_mut()
            .and_then(|pm| pm.focused_pane_mut())
            .map(|pane| &mut pane.cache)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Active bell state — focused pane.
    #[inline]
    pub(crate) fn active_bell(&self) -> &BellState {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane())
            .map(|pane| &pane.bell)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }

    /// Mutable active bell state — focused pane.
    #[inline]
    pub(crate) fn active_bell_mut(&mut self) -> &mut BellState {
        self.pane_manager
            .as_mut()
            .and_then(|pm| pm.focused_pane_mut())
            .map(|pane| &mut pane.bell)
            .expect("Tab must always have a pane_manager with a focused pane (R-32)")
    }
}
