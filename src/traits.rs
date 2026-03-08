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
//! [`UIElement`] is **fully implemented** on [`crate::tab_bar_ui::TabBarUI`] and
//! [`crate::status_bar::StatusBarUI`].
//! See [`crate::traits_impl`] for the concrete impls and a compile-time test.
//!
//! `EventHandler` is still deferred — see the comment block at the end of this file.
//!
//! # Migration Path for `EventHandler`
//!
//! When the `WindowState` decomposition is further advanced:
//! 1. Define `EventHandler<E>` where `E = winit::event::WindowEvent` and implement it
//!    on each sub-handler struct extracted from `WindowState`.
//! 2. Add mock implementations in `#[cfg(test)]` blocks.

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

// ── AUD-041: UIElement ────────────────────────────────────────────────────────

/// Common interface for UI bar/panel components whose height and visibility
/// depend on per-frame context (configuration, window state).
///
/// # Design rationale
///
/// The previous stub used zero-argument methods for `height_logical`,
/// `width_logical`, and `is_visible`.  This was incompatible with the concrete
/// types (`TabBarUI`, `StatusBarUI`) which all require `&Config` and additional
/// per-frame parameters to compute these values. Forcing them to cache a config
/// snapshot would introduce a new class of stale-config bugs.
///
/// The solution is a GAT (`type Ctx<'a>`) that lets each implementor declare
/// exactly what context it needs.  The caller provides context at the call site;
/// no component stores a redundant snapshot.
///
/// # GAT note
///
/// GATs (`type X<'a>`) require Rust 1.65+.  They have been stable since late
/// 2022 and are available in all supported rustc versions for this project.
///
/// # Concrete context types
///
/// - `TabBarUI::Ctx<'a> = TabBarCtx<'a>` — needs `(&Config, tab_count: usize)`
/// - `StatusBarUI::Ctx<'a> = StatusBarCtx<'a>` — needs `(&Config, is_fullscreen: bool)`
///
/// # Components that are excluded
///
/// `TmuxStatusBarUI::height` is a static method, not `&self`, so it cannot
/// implement this trait without a wrapper.  It is documented as out-of-scope.
pub trait UIElement {
    /// The per-call context type.  Implementors define a concrete `'a`-lifetime
    /// struct holding the references they need (e.g. `&'a Config`).
    type Ctx<'a>;

    /// Returns `true` if this element is currently visible.
    ///
    /// When not visible the element contributes 0px to the layout.
    fn is_visible(&self, ctx: Self::Ctx<'_>) -> bool;

    /// Effective height of the element in logical (DPI-independent) pixels.
    ///
    /// Returns 0.0 when not visible or when the element is positioned on a
    /// vertical axis (e.g. a left-side tab bar contributes width, not height).
    fn height_logical(&self, ctx: Self::Ctx<'_>) -> f32;

    /// Effective width of the element in logical pixels.
    ///
    /// Returns 0.0 for horizontally-positioned elements (top/bottom bars).
    fn width_logical(&self, ctx: Self::Ctx<'_>) -> f32;

    /// Returns `true` if this element is currently capturing keyboard input.
    ///
    /// When `true`, key events should not be forwarded to the PTY.
    /// This method requires no context because it reflects live interaction state
    /// rather than layout state.
    fn is_capturing_input(&self) -> bool;
}

// ── R-10: OverlayComponent ────────────────────────────────────────────────────

/// Common interface for egui overlay UI components that follow the
/// `show(&mut self, ctx: &egui::Context) -> Self::Action` pattern.
///
/// # Design notes
///
/// Twelve or more UI dialogs in par-term share the same shape:
/// - they maintain a `visible: bool` field (or equivalent),
/// - they expose a `show` method that renders the dialog and returns an action,
/// - they can be hidden without producing an action.
///
/// This trait formalises that contract so callers can be written generically
/// and components become easier to test in isolation.
///
/// # Components that are excluded
///
/// The following components have additional required parameters on `show` and
/// cannot implement this trait without a wrapper:
/// - `HelpUI::show` — returns `()` (no action type)
/// - `TmuxSessionPickerUI::show` — requires `tmux_path: &str`
/// - `InspectorPanel::show` — requires `available_agents: &[AgentConfig]`
///
/// These are documented as out-of-scope in `docs/TRAITS.md` (future work).
pub trait OverlayComponent {
    /// The action type produced by this component's `show` call.
    type Action;

    /// Render the overlay and return any action produced by user interaction.
    ///
    /// When the component is not visible this must return the "no action"
    /// variant immediately, without touching `ctx`.
    fn show(&mut self, ctx: &egui::Context) -> Self::Action;

    /// Returns `true` if the overlay is currently visible.
    fn is_visible(&self) -> bool;

    /// Show or hide the overlay.
    ///
    /// Setting to `false` hides the dialog immediately.  Setting to `true`
    /// is equivalent to calling a parameter-free open method; components
    /// that require additional state to open (e.g. `show_for_tab`) should
    /// use their own specific API instead of relying on this method.
    fn set_visible(&mut self, visible: bool);
}

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
