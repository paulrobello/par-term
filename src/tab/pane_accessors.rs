//! Per-pane state accessor routing for `Tab`.
//!
//! AUD-002 / AUD-062: These accessor methods route through the focused pane in
//! split-pane mode and fall back to the tab-level field in single-pane mode.
//!
//! All external callers should use these methods rather than the fields directly so that:
//!
//! 1. Split-pane behaviour is correct today (each pane has isolated state).
//! 2. Removing the tab-level fallback fields in a future clean-up step
//!    only requires deleting the `else` branch here — no call site changes.
//!
//! The tab-level fields (`scroll_state`, `mouse`, `cache`, `bell`) are kept as the
//! single-pane fallback until per-pane state is validated as the sole source
//! of truth across the entire codebase.

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::scroll_state::ScrollState;
use crate::tab::Tab;

impl Tab {
    /// Get the mouse state for selection operations.
    ///
    /// In split-pane mode, returns the focused pane's mouse state so that
    /// selection coordinates are isolated per-pane. In single-pane mode,
    /// returns the tab's own mouse state.
    pub(crate) fn selection_mouse(&self) -> &MouseState {
        if let Some(ref pm) = self.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            &focused_pane.mouse
        } else {
            &self.mouse
        }
    }

    /// Get mutable mouse state for selection operations.
    ///
    /// In split-pane mode, returns the focused pane's mouse state so that
    /// selection coordinates are isolated per-pane. In single-pane mode,
    /// returns the tab's own mouse state.
    pub(crate) fn selection_mouse_mut(&mut self) -> &mut MouseState {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(focused_pane) = pm.focused_pane_mut()
        {
            &mut focused_pane.mouse
        } else {
            &mut self.mouse
        }
    }

    // =========================================================================
    // AUD-002 / AUD-062: Per-pane state accessors
    //
    // These accessors route through the focused pane in split-pane mode and
    // fall back to the tab-level field in single-pane mode.  All external
    // callers should use these methods rather than the fields directly so that:
    //
    //   1. Split-pane behaviour is correct today (each pane has isolated state).
    //   2. Removing the tab-level fallback fields in a future clean-up step
    //      only requires deleting the `else` branch here — no call site changes.
    //
    // The tab-level fields (scroll_state, mouse, cache, bell) are kept as the
    // single-pane fallback until per-pane state is validated as the sole source
    // of truth across the entire codebase.
    // =========================================================================

    /// Active scroll state — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_scroll_state(&self) -> &ScrollState {
        if let Some(ref pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane()
        {
            &pane.scroll_state
        } else {
            &self.scroll_state
        }
    }

    /// Mutable active scroll state — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_scroll_state_mut(&mut self) -> &mut ScrollState {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane_mut()
        {
            &mut pane.scroll_state
        } else {
            &mut self.scroll_state
        }
    }

    /// Active mouse state — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_mouse(&self) -> &MouseState {
        if let Some(ref pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane()
        {
            &pane.mouse
        } else {
            &self.mouse
        }
    }

    /// Mutable active mouse state — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_mouse_mut(&mut self) -> &mut MouseState {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane_mut()
        {
            &mut pane.mouse
        } else {
            &mut self.mouse
        }
    }

    /// Active render cache — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_cache(&self) -> &RenderCache {
        if let Some(ref pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane()
        {
            &pane.cache
        } else {
            &self.cache
        }
    }

    /// Mutable active render cache — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_cache_mut(&mut self) -> &mut RenderCache {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane_mut()
        {
            &mut pane.cache
        } else {
            &mut self.cache
        }
    }

    /// Active bell state — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_bell(&self) -> &BellState {
        if let Some(ref pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane()
        {
            &pane.bell
        } else {
            &self.bell
        }
    }

    /// Mutable active bell state — focused pane in split mode, tab-level otherwise.
    #[inline]
    pub(crate) fn active_bell_mut(&mut self) -> &mut BellState {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(pane) = pm.focused_pane_mut()
        {
            &mut pane.bell
        } else {
            &mut self.bell
        }
    }
}
