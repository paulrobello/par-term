//! Per-window renderer config application for `apply_config_to_windows`.
//!
//! Extracted from `config_propagation` to keep that file under 500 lines.
//!
//! `apply_renderer_config` applies all renderer-related settings from a freshly-edited
//! `Config` to a single `WindowState` that already has its renderer initialised.
//! Returns the shader-error results for propagation to the standalone settings window.

mod phases;
mod shaders;

use crate::app::window_state::WindowState;
use crate::app::window_state::config_updates::ConfigChanges;
use crate::config::Config;

/// Apply all renderer-related config changes to a single window state.
///
/// Returns `(shader_result, cursor_shader_result)` where each is:
/// - `None`       — no change was attempted (no shader-related change detected)
/// - `Some(None)` — change applied successfully
/// - `Some(Some(err_msg))` — change attempted but failed with this error
pub(super) fn apply_renderer_config(
    window_state: &mut WindowState,
    config: &Config,
    changes: &ConfigChanges,
) -> (Option<Option<String>>, Option<Option<String>>) {
    if window_state.renderer.is_none() {
        return (None, None);
    }
    phases::apply_appearance(window_state, config, changes);
    phases::apply_cursor(window_state, config, changes);
    phases::apply_backgrounds(window_state, config, changes);
    phases::apply_terminal_settings(window_state, config, changes);
    shaders::apply_shaders(window_state, config, changes)
}
