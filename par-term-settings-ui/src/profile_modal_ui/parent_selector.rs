//! Profile parent selector — cycle-safe parent picker dropdown.
//!
//! Implements `has_ancestor` (cycle detection) and `render_parent_selector`
//! (egui ComboBox) as `impl ProfileModalUI` methods.

use super::ProfileModalUI;
use par_term_config::ProfileId;

impl ProfileModalUI {
    /// Check if `ancestor_id` appears in the parent chain of `profile_id`.
    pub(super) fn has_ancestor(&self, profile_id: ProfileId, ancestor_id: ProfileId) -> bool {
        let mut current_id = profile_id;
        let mut visited = vec![current_id];
        while let Some(parent_id) = self
            .working_profiles
            .iter()
            .find(|p| p.id == current_id)
            .and_then(|p| p.parent_id)
        {
            if parent_id == ancestor_id {
                return true;
            }
            if visited.contains(&parent_id) {
                return false;
            }
            visited.push(parent_id);
            current_id = parent_id;
        }
        false
    }

    /// Render the parent profile selector dropdown.
    pub(super) fn render_parent_selector(&mut self, ui: &mut egui::Ui) {
        // Get valid parents (excludes self and profiles that would create cycles)
        let current_id = self.editing_id;
        let valid_parents: Vec<_> = self
            .working_profiles
            .iter()
            .filter(|p| {
                // Cannot select self as parent
                if Some(p.id) == current_id {
                    return false;
                }
                // Prevent cycles: reject if this candidate has current profile as ancestor
                if let Some(cid) = current_id
                    && self.has_ancestor(p.id, cid)
                {
                    return false;
                }
                true
            })
            .map(|p| (p.id, p.display_label()))
            .collect();

        let selected_label = self
            .temp_parent_id
            .and_then(|id| self.working_profiles.iter().find(|p| p.id == id))
            .map(|p| p.display_label())
            .unwrap_or_else(|| "(None)".to_string());

        egui::ComboBox::from_id_salt("parent_profile_selector")
            .selected_text(&selected_label)
            .show_ui(ui, |ui| {
                // Option to clear parent
                if ui
                    .selectable_label(self.temp_parent_id.is_none(), "(None)")
                    .clicked()
                {
                    self.temp_parent_id = None;
                }
                // List valid parents
                for (id, label) in valid_parents {
                    if ui
                        .selectable_label(self.temp_parent_id == Some(id), &label)
                        .clicked()
                    {
                        self.temp_parent_id = Some(id);
                    }
                }
            });
    }
}
