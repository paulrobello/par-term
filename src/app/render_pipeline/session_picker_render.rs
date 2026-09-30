//! Draw the unified session picker (UX.md A16) from window state.
//!
//! A free function over the fields it reads, so the egui closure in
//! `egui_submit` can call it while it holds other `WindowState` borrows.

use crate::app::tmux_handler::tmux_state::TmuxState;
use crate::config::Config;
use crate::tmux_session_picker_ui::{
    SessionPickerAction, SessionPickerContext, TmuxSessionPickerUI,
};
use arc_swap::ArcSwap;

pub(super) fn show_session_picker(
    ctx: &egui::Context,
    picker: &mut TmuxSessionPickerUI,
    config: &ArcSwap<Config>,
    tmux_state: &TmuxState,
) -> SessionPickerAction {
    if !picker.visible {
        return SessionPickerAction::None;
    }
    let config = config.load();
    let tmux_path = config.resolve_tmux_path();
    #[cfg(feature = "mux")]
    let attached = tmux_state
        .transport
        .as_ref()
        .and(tmux_state.tmux_session_name.as_deref())
        .map(|name| (tmux_state.mux_daemon.as_deref().unwrap_or(name), name));
    #[cfg(feature = "mux")]
    let mux = Some(crate::session_picker_mux::MuxPickerInput {
        directory: tmux_state.mux_directory.as_ref(),
        loading: tmux_state.mux_directory_scan.is_some(),
        attached,
    });
    #[cfg(not(feature = "mux"))]
    let mux = {
        let _ = tmux_state;
        None
    };
    picker.show(
        ctx,
        &SessionPickerContext {
            tmux_path: &tmux_path,
            tmux_enabled: config.tmux.tmux_enabled,
            mux,
        },
    )
}
