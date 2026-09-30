//! Dependent controls (UX.md SC1).
//!
//! A control that only matters while another setting is on is drawn indented
//! under that setting and disabled, never hidden, while the setting is off.
//! Hiding made the control impossible to find (and to search for); leaving it
//! enabled made it do nothing with no explanation. The disabled group's hover
//! text names the setting it depends on.

/// Draw `add` indented under its parent setting, disabled unless `enabled`.
///
/// `parent` names the parent control as the user sees it (a checkbox label,
/// or `Label: value` for a choice), shown in the disabled hover text.
pub fn dependent<R>(
    ui: &mut egui::Ui,
    enabled: bool,
    parent: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    #[cfg(test)]
    record(parent, enabled);
    // egui can only indent a vertical layout; inside a row the group stays
    // inline, next to its parent.
    let response = if ui.layout().is_vertical() {
        ui.indent(("dependent", parent), |ui| {
            ui.add_enabled_ui(enabled, add).inner
        })
    } else {
        ui.add_enabled_ui(enabled, add)
    };
    if !enabled {
        response
            .response
            .on_disabled_hover_text(format!("Depends on \"{parent}\""));
    }
    response.inner
}

#[cfg(test)]
thread_local! {
    static DRAWN: std::cell::RefCell<Vec<(String, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn record(parent: &str, enabled: bool) {
    DRAWN.with(|d| d.borrow_mut().push((parent.to_string(), enabled)));
}

/// Dependent groups drawn on this thread since the last call, as
/// `(parent label, enabled)`.
#[cfg(test)]
pub(crate) fn take_drawn() -> Vec<(String, bool)> {
    DRAWN.with(|d| std::mem::take(&mut *d.borrow_mut()))
}
