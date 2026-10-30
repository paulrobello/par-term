//! First-use confirmation dialog for agent-requested shader activations.
//!
//! An agent may set `custom_shader`/`cursor_shader` only to files inside the
//! shaders directory (SEC-003); a value that differs from the active shader
//! additionally needs the user's approval before it activates, so the agent
//! apply path (`agent_config.rs`) parks it in
//! `ShaderState::pending_shader_changes` and this dialog answers the last
//! queued change. Activate applies it through the agent apply path with the
//! gate bypassed (drained in `about_to_wait`); Ignore drops it. Escape stays
//! inert for the same reason as the agent-command dialog: declining is an
//! answer the user must click, not a key they brush.

/// Render the confirmation dialog for the pending agent shader changes.
pub(super) fn render_shader_change_confirm_dialog(
    ctx: &egui::Context,
    shader_state: &mut crate::app::window_state::shader_state::ShaderState,
) {
    if !shader_state.has_pending_shader_changes() {
        shader_state.shader_confirm_dialog_activated_frame = None;
        return;
    }

    if shader_state.shader_confirm_dialog_activated_frame.is_none() {
        shader_state.shader_confirm_dialog_activated_frame = Some(ctx.cumulative_frame_nr());
    }
    let activated_frame = shader_state
        .shader_confirm_dialog_activated_frame
        .unwrap_or(0);
    let current_frame = ctx.cumulative_frame_nr();

    // Display info extracted before the egui closure to avoid re-borrowing.
    // `answered` is the exact change this frame displayed, so the recorded
    // decision can never attach to a change queued after it drew.
    let answered = shader_state
        .pending_shader_changes
        .last()
        .expect("checked non-empty")
        .clone();
    let kind = match answered.key.as_str() {
        "custom_shader" => "background",
        "cursor_shader" => "cursor",
        _ => "shader",
    };
    let name = answered.value.clone();
    let queued_count = shader_state.pending_shader_changes.len();

    let mut activate = false;
    let mut decline = false;

    egui::Window::new("Agent Shader Change")
        .id(egui::Id::unique("agent_shader_confirm_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.set_max_width(520.0);

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("Activate {kind} shader \u{201c}{name}\u{201d}?"))
                    .strong()
                    .size(15.0),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Requested by an agent (config_update).")
                    .weak()
                    .small(),
            );

            ui.add_space(6.0);
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 40))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&name).monospace());
                });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "Agents can write GLSL into your shaders directory ({}). \
                     Activate only files you recognize.",
                    crate::config::Config::shaders_dir().display()
                ))
                .weak()
                .small(),
            );

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui
                    .button(
                        egui::RichText::new("Ignore").color(egui::Color32::from_rgb(220, 60, 60)),
                    )
                    .clicked()
                    && current_frame > activated_frame
                {
                    decline = true;
                }
                ui.add_space(4.0);
                if ui
                    .button(
                        egui::RichText::new("Activate").color(egui::Color32::from_rgb(80, 180, 80)),
                    )
                    .clicked()
                    && current_frame > activated_frame
                {
                    activate = true;
                }
            });

            if queued_count > 1 {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("({} more queued)", queued_count - 1))
                        .weak()
                        .small(),
                );
            }
        });

    if activate || decline {
        shader_state.shader_change_decision = Some((answered, activate));
        if queued_count > 1 {
            shader_state.shader_confirm_dialog_activated_frame = Some(ctx.cumulative_frame_nr());
        } else {
            shader_state.shader_confirm_dialog_activated_frame = None;
        }
    }
}
