//! Cross-platform keyboard modifier helpers.
//!
//! Terminal emulators face a fundamental conflict on non-macOS platforms: `Ctrl`
//! is the standard OS-level modifier *and* also generates POSIX control codes
//! inside a terminal (e.g. `Ctrl+C` = SIGINT, `Ctrl+W` = delete-word).
//!
//! par-term resolves this by using different "primary" modifiers per platform:
//!
//! | Platform | Primary modifier | Rationale |
//! |---|---|---|
//! | macOS | `Cmd` (`super_key`) | Separate from Ctrl; no terminal conflicts |
//! | Windows / Linux | `Ctrl` (`control_key`) | macOS `Cmd` key unavailable |
//!
//! When a shortcut needs `Cmd+X` on macOS and `Ctrl+Shift+X` on others (to avoid
//! clobbering Ctrl-only terminal bindings), callers should:
//! 1. Check `primary_modifier(mods)` for the first modifier.
//! 2. Check `primary_modifier_with_shift(mods)` when `Shift` must also be held.
//!
//! This avoids scattered `#[cfg(target_os = "macos")]` pairs at every call site.

use winit::keyboard::ModifiersState;

/// Returns `true` when the platform's **primary** modifier key is held and
/// **Shift** is NOT held, i.e.:
///
/// - macOS: `Cmd` pressed, `Shift` not pressed
/// - Windows/Linux: `Ctrl` pressed, `Shift` not pressed
///
/// Use this for single-key shortcuts (`Cmd+T` on macOS / `Ctrl+T` elsewhere)
/// when the shortcut explicitly avoids Shift.
pub fn primary_modifier(mods: &ModifiersState) -> bool {
    #[cfg(target_os = "macos")]
    {
        mods.super_key() && !mods.shift_key()
    }
    #[cfg(not(target_os = "macos"))]
    {
        mods.control_key() && !mods.shift_key()
    }
}

/// Returns `true` when the platform's **primary** modifier key is held and
/// **Shift is also held**, i.e.:
///
/// - macOS: `Cmd+Shift`
/// - Windows/Linux: `Ctrl+Shift`
///
/// Use this for shortcuts that require Shift to avoid conflicts
/// (`Cmd+Shift+]` on macOS / `Ctrl+Shift+]` elsewhere).
pub fn primary_modifier_with_shift(mods: &ModifiersState) -> bool {
    #[cfg(target_os = "macos")]
    {
        mods.super_key() && mods.shift_key()
    }
    #[cfg(not(target_os = "macos"))]
    {
        mods.control_key() && mods.shift_key()
    }
}
