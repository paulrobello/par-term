//! egui frame pieces split from `egui_submit.rs` (which stays inside the
//! 800-line limit): the demote pick-mode overlays, the command palette's
//! runtime-rows assembly, and the self-update dialog with its install
//! flow. These are free functions over the specific `WindowState` fields
//! they touch — the egui closure in `render_egui_frame` captures fields
//! disjointly (a whole-`*self` method call there breaks that capture, and
//! with it the outer `window` borrow).

use super::egui_overlays;
use super::types::{DemoteAction, DemoteSnapshot, PostRenderActions};
use crate::agent_commands_store::AgentCommandStore;
use crate::command_palette::CommandPalette;
use crate::config::Config;
use crate::crash_triage::CrashTriageState;
use crate::pane::{PaneBounds, SplitDirection};
use crate::status_bar::StatusBarUI;
use arc_swap::ArcSwap;

#[cfg(feature = "mux")]
use crate::app::tmux_handler::tmux_state::TmuxState;

/// Demote pick-mode overlays: the toast hint while a pick mode is armed,
/// and the split-direction chooser dialog at the target pane's center
/// once both tab and pane are chosen. Pure egui rendering — state arrives
/// as the snapshot captured before the egui closure, and the chosen
/// direction leaves through `actions.demote`.
pub(super) fn render_demote_overlays(
    ctx: &egui::Context,
    snapshot: DemoteSnapshot,
    pane_bounds: Option<PaneBounds>,
    actions: &mut PostRenderActions,
) {
    match snapshot {
        DemoteSnapshot::PickTab => {
            egui_overlays::render_toast_overlay(
                ctx,
                Some("Click a tab to merge into (Esc to cancel)"),
            );
        }
        DemoteSnapshot::PickPane => {
            egui_overlays::render_toast_overlay(
                ctx,
                Some("Click a pane to merge into (Esc to cancel)"),
            );
        }
        // QA-004: destructure ChooseDirection ONCE here so the click
        // handlers below can reference the bound IDs directly. The outer
        // match guarantees the variant, so there is no failing variant
        // check inside the closures — a wrong variant skips this arm
        // entirely (falling through to PickTab/PickPane/Idle) instead
        // of panicking mid-frame via `unreachable!()`.
        DemoteSnapshot::ChooseDirection {
            source_tab_id,
            target_tab_id,
            target_pane_id,
        } => {
            if let Some(bounds) = pane_bounds {
                let center_x = bounds.x + bounds.width / 2.0;
                let center_y = bounds.y + bounds.height / 2.0;

                egui::Area::new(egui::Id::new("demote_direction_overlay"))
                    .fixed_pos(egui::pos2(center_x - 100.0, center_y - 30.0))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                            .inner_margin(egui::Margin::symmetric(16, 10))
                            .corner_radius(8.0)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80)))
                            .show(ui, |ui| {
                                ui.style_mut().visuals.override_text_color =
                                    Some(egui::Color32::from_rgb(255, 255, 255));
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new("Split direction:").size(14.0));
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(egui::RichText::new("Horizontal").size(14.0))
                                            .clicked()
                                        {
                                            actions.demote = DemoteAction::Execute {
                                                source_tab_id,
                                                target_tab_id,
                                                target_pane_id,
                                                direction: SplitDirection::Horizontal,
                                            };
                                        }
                                        if ui
                                            .button(egui::RichText::new("Vertical").size(14.0))
                                            .clicked()
                                        {
                                            actions.demote = DemoteAction::Execute {
                                                source_tab_id,
                                                target_tab_id,
                                                target_pane_id,
                                                direction: SplitDirection::Vertical,
                                            };
                                        }
                                    });
                                });
                            });
                    });
            }
        }
        DemoteSnapshot::Idle => {}
    }
}

/// Open the command palette joined with every runtime-rows source —
/// plugin actions, the mux agent roster, agent-authored commands,
/// configured launchable agents, captured crashes, and the attached
/// par-mux session's detach row. Opened the same way the
/// `toggle_command_palette` keybinding opens it; blocked agents lead the
/// empty-query view.
pub(super) fn open_command_palette_with_runtime_rows(
    status_bar_ui: &StatusBarUI,
    agent_commands: &AgentCommandStore,
    crash_triage: &mut CrashTriageState,
    config: &ArcSwap<Config>,
    command_palette: &mut CommandPalette,
    #[cfg(feature = "mux")] tmux_state: &TmuxState,
) {
    // The `mut` serves only the mux arm's extend below.
    #[cfg_attr(not(feature = "mux"), allow(unused_mut))]
    let mut plugin_rows = crate::command_palette::catalog::plugin_palette_entries(
        &status_bar_ui.plugin_host().palette_actions(),
    );
    #[cfg(feature = "mux")]
    {
        let map = &tmux_state.tmux_pane_owners;
        plugin_rows.extend(
            tmux_state
                .agent_roster
                .palette_rows(&|pane| map.contains_key(&pane)),
        );
    }
    // Agent-authored commands join the palette the same way (runtime
    // rows from the store).
    plugin_rows.extend(agent_commands.palette_rows());
    // Configured launchable agents (`agents:` config list) — same
    // runtime-rows pattern.
    plugin_rows.extend(crate::command_palette::catalog::agent_palette_entries(
        &config.load().agents,
    ));
    // Captured crashes — the triage consent surface (crash_triage module).
    plugin_rows.extend(crash_triage.palette_entries());
    // The attached par-mux session's detach row — same runtime-rows
    // pattern as the roster.
    #[cfg(feature = "mux")]
    plugin_rows.extend(tmux_state.mux_palette_rows());
    command_palette.open(plugin_rows);
}

/// The self-update dialog: poll an in-flight install, render the dialog,
/// and apply its actions (dismiss, skip version — which writes config,
/// or install — which spawns the updater thread and reports back through
/// `install_receiver`). Runs inside the egui closure; the save-config
/// request leaves through `actions`.
pub(super) fn render_update_dialog(
    ctx: &egui::Context,
    update_state: &mut crate::app::window_state::UpdateState,
    status_bar_ui: &mut StatusBarUI,
    config: &ArcSwap<Config>,
    actions: &mut PostRenderActions,
) {
    if !update_state.show_dialog {
        return;
    }
    // Poll for update install completion
    if let Some(ref rx) = update_state.install_receiver
        && let Ok(result) = rx.try_recv()
    {
        match result {
            Ok(update_result) => {
                update_state.install_status = Some(format!(
                    "Updated to v{}! Restart par-term to use the new version.",
                    update_result.new_version
                ));
                update_state.installing = false;
                status_bar_ui.update_available_version = None;
            }
            Err(e) => {
                update_state.install_status = Some(format!("Update failed: {}", e));
                update_state.installing = false;
            }
        }
        update_state.install_receiver = None;
    }

    if let Some(ref update_result) = update_state.last_result {
        let dialog_action = crate::update_dialog::render(
            ctx,
            update_result,
            env!("CARGO_PKG_VERSION"),
            update_state.installation_type,
            update_state.installing,
            update_state.install_status.as_deref(),
        );
        match dialog_action {
            crate::update_dialog::UpdateDialogAction::Dismiss => {
                if !update_state.installing {
                    update_state.show_dialog = false;
                    update_state.install_status = None;
                }
            }
            crate::update_dialog::UpdateDialogAction::SkipVersion(v) => {
                config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.updates.skipped_version = Some(v.clone());
                    std::sync::Arc::new(new)
                });
                update_state.show_dialog = false;
                status_bar_ui.update_available_version = None;
                update_state.install_status = None;
                actions.save_config = true;
            }
            crate::update_dialog::UpdateDialogAction::InstallUpdate(v) => {
                if !update_state.installing {
                    update_state.installing = true;
                    update_state.install_status = Some("Downloading update...".to_string());
                    let (tx, rx) = std::sync::mpsc::channel();
                    update_state.install_receiver = Some(rx);
                    let version = v.clone();
                    let current_version = crate::VERSION.to_string();
                    std::thread::spawn(move || {
                        let result = par_term_update::self_updater::perform_update(
                            &version,
                            &current_version,
                        );
                        let _ = tx.send(result);
                    });
                }
                // Don't close dialog while installing
            }
            crate::update_dialog::UpdateDialogAction::None => {}
        }
    } else {
        update_state.show_dialog = false;
    }
}
