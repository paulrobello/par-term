//! Post-render action dispatch for WindowState.
//!
//! All handlers are called from `update_post_render_state()` (in render_pipeline.rs)
//! after the renderer borrow is released.
//!
//! ## Handler groups
//!
//! - [`tab_bar`] — switch, close, new tab, reorder, color, rename, icon, and assistant-panel toggle.
//! - [`inspector`] — all 17 `InspectorAction` variants.
//! - [`integrations`] — shader install, shell integration install, conflict resolution.
//! - Clipboard history (inline below) — paste, clear-all, clear-slot (too small for its own file).

mod inspector;
mod integrations;
mod tab_bar;

use crate::app::window_state::WindowState;
use crate::clipboard_history_ui::ClipboardHistoryAction;

impl WindowState {
    /// Handle clipboard history actions collected during egui rendering.
    pub(crate) fn handle_clipboard_history_action_after_render(
        &mut self,
        action: crate::clipboard_history_ui::ClipboardHistoryAction,
    ) {
        // Handle clipboard actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
            ClipboardHistoryAction::Paste(content) => {
                self.paste_text(&content);
            }
            ClipboardHistoryAction::ClearAll => {
                self.with_active_tab(|tab| {
                    if let Ok(term) = tab.terminal.try_write() {
                        term.clear_all_clipboard_history();
                        log::info!("Cleared all clipboard history");
                    }
                });
                self.overlay_ui
                    .clipboard_history_ui
                    .update_entries(Vec::new());
            }
            ClipboardHistoryAction::ClearSlot(slot) => {
                self.with_active_tab(|tab| {
                    if let Ok(term) = tab.terminal.try_write() {
                        term.clear_clipboard_history(slot);
                        log::info!("Cleared clipboard history for slot {:?}", slot);
                    }
                });
            }
            ClipboardHistoryAction::None => {}
        }
    }
}
