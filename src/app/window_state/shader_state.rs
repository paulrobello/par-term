//! Shader hot-reload and metadata-cache state for a window.
//!
//! Groups the fields that track the shader file-watcher, the two metadata
//! caches (background and cursor shader), and the last reload error so they
//! can be reasoned about independently from the rest of `WindowState`.

use crate::config::{CursorShaderMetadataCache, ShaderMetadataCache};
use crate::shader_watcher::ShaderWatcher;

/// An agent shader-path change awaiting the user's confirmation (SEC-003
/// residual): a value that differs from the active shader would activate new
/// or changed GPU code, so it is parked here for the confirmation dialog
/// instead of being applied (`agent_config.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingShaderChange {
    /// Config key the value was sent for (`custom_shader` or `cursor_shader`).
    pub(crate) key: String,
    /// The shader name or path the agent sent.
    pub(crate) value: String,
}

/// Shader file-watcher, metadata caches, and reload-error state.
pub(crate) struct ShaderState {
    /// Shader file watcher for hot reload support
    pub(crate) shader_watcher: Option<ShaderWatcher>,
    /// Cache for parsed background-shader metadata (used for config resolution)
    pub(crate) shader_metadata_cache: ShaderMetadataCache,
    /// Cache for parsed cursor-shader metadata (used for config resolution)
    pub(crate) cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Last shader reload error message (for display in UI)
    pub(crate) shader_reload_error: Option<String>,
    /// Last background shader compile/reload error for assistant diagnostics.
    pub(crate) background_shader_last_error: Option<String>,
    /// Last cursor shader compile/reload error for assistant diagnostics.
    pub(crate) cursor_shader_last_error: Option<String>,
    /// Agent shader-path changes awaiting the user's first-use confirmation,
    /// oldest queued first. The dialog answers the head (the last queued);
    /// the decision is drained outside the egui render pass.
    pub(crate) pending_shader_changes: Vec<PendingShaderChange>,
    /// The confirmation dialog's answer, bound to the exact change it
    /// displayed (`Some(true)` = activate, `Some(false)` = decline), drained
    /// by `about_to_wait` where `&mut WindowState` is free of the egui
    /// borrow. Binding to the change (not just the answer) means a change
    /// queued after the dialog drew cannot consume the earlier approval.
    pub(crate) shader_change_decision: Option<(PendingShaderChange, bool)>,
    /// Flicker guard: the frame the confirmation dialog opened, so a
    /// click that lands on the dialog's first frame cannot answer it.
    pub(crate) shader_confirm_dialog_activated_frame: Option<u64>,
    /// Background shader reload result: None = no change, Some(None) = success, Some(Some(err)) = error
    /// Used to propagate hot reload results to standalone settings window
    pub(crate) background_shader_reload_result: Option<Option<String>>,
    /// Cursor shader reload result: None = no change, Some(None) = success, Some(Some(err)) = error
    /// Used to propagate hot reload results to standalone settings window
    pub(crate) cursor_shader_reload_result: Option<Option<String>>,
}

impl ShaderState {
    /// Create with pre-built metadata caches (requires the shaders directory path).
    pub(crate) fn new(shaders_dir: std::path::PathBuf) -> Self {
        Self {
            shader_watcher: None,
            shader_metadata_cache: ShaderMetadataCache::with_shaders_dir(shaders_dir.clone()),
            cursor_shader_metadata_cache: CursorShaderMetadataCache::with_shaders_dir(shaders_dir),
            shader_reload_error: None,
            background_shader_last_error: None,
            cursor_shader_last_error: None,
            background_shader_reload_result: None,
            cursor_shader_reload_result: None,
            pending_shader_changes: Vec::new(),
            shader_change_decision: None,
            shader_confirm_dialog_activated_frame: None,
        }
    }

    /// Queue a shader change for the confirmation dialog unless an identical
    /// (key, value) pair is already pending, so an agent retry cannot stack
    /// duplicate dialogs. Returns whether the change was newly queued.
    pub(crate) fn queue_shader_change(&mut self, change: PendingShaderChange) -> bool {
        if self.pending_shader_changes.iter().any(|p| p == &change) {
            return false;
        }
        self.pending_shader_changes.push(change);
        true
    }

    /// Consume the dialog's decision. Returns the change it answered only if
    /// that exact change is still pending, so an approval or decline always
    /// applies to the request the user actually saw.
    pub(crate) fn take_shader_decision(&mut self) -> Option<(PendingShaderChange, bool)> {
        let (change, approved) = self.shader_change_decision.take()?;
        let pos = self
            .pending_shader_changes
            .iter()
            .position(|p| *p == change)?;
        self.pending_shader_changes.remove(pos);
        Some((change, approved))
    }

    /// Whether the confirmation dialog has anything to show.
    pub(crate) fn has_pending_shader_changes(&self) -> bool {
        !self.pending_shader_changes.is_empty()
    }
}
