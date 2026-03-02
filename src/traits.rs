//! Shared trait definitions for par-term components.
//!
//! These traits document the contracts between major components and enable
//! mock implementations for unit testing the `app/` module without needing
//! a live PTY session or GPU context.
//!
//! # Status
//!
//! [`TerminalAccess`] is **fully implemented** on [`crate::terminal::TerminalManager`].
//! See [`crate::traits_impl`] for the concrete `impl` and a `MockTerminal` test helper.
//!
//! `UIElement` and `EventHandler` were previously stub-only definitions in this file.
//! Both have been **removed** because their signatures were incompatible with the existing
//! concrete types:
//!
//! - **`UIElement`** (`is_visible`, `height_logical`, `width_logical`, `is_capturing_input`)
//!   required zero-argument methods, but `TabBarUI::get_height()` and
//!   `StatusBarUI::height()` both require `&Config` (and sometimes `tab_count: usize`
//!   or `is_fullscreen: bool`) as parameters. Implementing the trait would force these
//!   components to store a redundant config snapshot, which is worse than no trait.
//!   A correct design would parameterise the trait itself: `trait UIElement<Ctx>` or
//!   pass context through a `render_context()` accessor, but that is a larger design
//!   decision tracked as a future improvement.
//!
//! - **`EventHandler`** (`handle_event(&mut self, event: Self::Event) -> bool`) is a
//!   generic marker trait with an associated type. Wiring it up requires choosing a
//!   single concrete `Event` type (`winit::event::WindowEvent`), touching the monolithic
//!   `WindowState` dispatch chain, and updating all call sites simultaneously. That scope
//!   exceeds a targeted refactor of this file. The trait will be reintroduced as part of
//!   the `WindowState` decomposition effort.
//!
//! # Migration Path for future `UIElement` / `EventHandler`
//!
//! When the `WindowState` decomposition is ready:
//! 1. Define `UIElement` with a `RenderCtx<'_>` parameter that holds `&Config` and any
//!    other per-frame data (`tab_count`, `is_fullscreen`).
//! 2. Define `EventHandler<E>` where `E = winit::event::WindowEvent` and implement it
//!    on each sub-handler struct extracted from `WindowState`.
//! 3. Add mock implementations in `#[cfg(test)]` blocks.

// ── AUD-040: TerminalAccess ──────────────────────────────────────────────────

/// Provides read-only access to terminal mode and state for input/mouse handlers.
///
/// Implemented by `TerminalManager` (and mock types in tests). Decouples
/// `app/mouse_events/` and `app/input_events/` from the concrete terminal type,
/// enabling unit tests without a live PTY session.
///
/// # Notes
///
/// All methods take `&self` — this trait is intentionally read-only. State
/// mutation goes through `TerminalManager` directly (write, resize, etc.).
pub trait TerminalAccess {
    /// Returns `true` if the alternate screen buffer (DECSC/smcup) is active.
    ///
    /// Used by mouse handlers to suppress scrollback scrolling while TUI apps
    /// (vim, htop, etc.) own the display.
    fn is_alt_screen_active(&self) -> bool;

    /// Returns `true` if mouse motion events should be reported to the PTY.
    ///
    /// `button_pressed` — whether a mouse button is currently held. Some
    /// modes (ButtonEvent) only report motion while a button is pressed.
    fn should_report_mouse_motion(&self, button_pressed: bool) -> bool;

    /// Returns the current modifyOtherKeys level (0 = off, 1 = basic, 2 = full).
    ///
    /// Controls how modifier-key combinations are encoded in key sequences.
    fn modify_other_keys_mode(&self) -> u8;

    /// Returns `true` if DECCKM (application cursor key) mode is active.
    ///
    /// In application mode, cursor keys send `\x1bO[ABCD]` instead of `\x1b[ABCD]`.
    fn application_cursor(&self) -> bool;

    /// Encode a mouse event into the bytes to send to the PTY.
    ///
    /// Parameters match the X10 / SGR / UTF-8 encoding conventions used by
    /// `par-term-emu-core-rust`:
    /// - `button` — button index (0-2 for primary/middle/secondary, 64+ for scroll)
    /// - `col` / `row` — 0-based cell coordinates
    /// - `pressed` — `true` for press, `false` for release
    /// - `modifiers` — bitmask: bit 2 = Shift, bit 3 = Alt, bit 4 = Ctrl
    fn encode_mouse_event(
        &self,
        button: u8,
        col: usize,
        row: usize,
        pressed: bool,
        modifiers: u8,
    ) -> Vec<u8>;
}

// ── AUD-041: UIElement — REMOVED ─────────────────────────────────────────────
//
// The `UIElement` trait was removed because its zero-argument methods
// (`height_logical`, `width_logical`, `is_visible`) are incompatible with the
// concrete types (`TabBarUI`, `StatusBarUI`, `TmuxStatusBarUI`), which all
// require `&Config` and additional per-frame parameters to compute these values.
//
// Forcing those components to cache config state just to satisfy the trait
// would introduce a new source of stale-config bugs.
//
// A correct design (future work):
//
//   pub trait UIElement {
//       type Ctx<'a>;
//       fn is_visible(&self, ctx: Self::Ctx<'_>) -> bool;
//       fn height_logical(&self, ctx: Self::Ctx<'_>) -> f32;
//       fn width_logical(&self, ctx: Self::Ctx<'_>) -> f32;
//       fn is_capturing_input(&self) -> bool;
//   }
//
// This requires GATs (stable since Rust 1.65) and can be introduced as part of
// the `WindowState` decomposition effort without breaking existing call sites.

// ── AUD-042: EventHandler — REMOVED ──────────────────────────────────────────
//
// The `EventHandler` trait was removed because wiring it up to the concrete
// `WindowState` dispatch chain is a larger structural refactor than can be done
// in this file alone.  The trait will be reintroduced as part of that effort.
//
// Proposed future definition (kept here as a design record):
//
//   pub trait EventHandler {
//       fn handle_event(&mut self, event: winit::event::WindowEvent) -> bool;
//   }
//
// To use this trait, each handler extracted from `WindowState` would implement it
// and `WindowState::on_window_event` would iterate a `Vec<Box<dyn EventHandler>>`.
// That decomposition is tracked in the `WindowState` refactor issue.
