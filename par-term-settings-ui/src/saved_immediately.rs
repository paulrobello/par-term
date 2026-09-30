//! "Saved immediately" notice for actions that write to disk as you act
//! (UX.md SS6).
//!
//! The prompt library, agent commands, plugin install/update/remove, shader
//! install, and shell integration manage files, not config.yaml settings, so
//! they keep writing at once instead of waiting for Save. This line says what
//! is written right away and where, so Revert is not mistaken for an undo of
//! them.

use std::path::Path;

/// One small line: `what` is written right away, to `path`, and Save and
/// Revert do not apply to it. `what` names only the immediate actions, so a
/// section that also holds ordinary config settings stays accurate.
pub fn saved_immediately(ui: &mut egui::Ui, what: &str, path: &Path) {
    ui.label(
        egui::RichText::new(format!(
            "{what} saved immediately to {}; Save and Revert do not apply to them.",
            path.display()
        ))
        .small()
        .color(egui::Color32::from_rgb(33, 150, 243)),
    );
}
