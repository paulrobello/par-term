//! Data types shared across render pipeline sub-modules.
//!
//! - `RendererSizing`: snapshot of physical pixel dimensions from the renderer
//! - `FrameRenderData`: terminal state gathered by `gather_render_data()`
//! - `PostRenderActions`: UI actions collected during `submit_gpu_frame()`

use crate::ai_inspector::panel::InspectorAction;
use crate::clipboard_history_ui::ClipboardHistoryAction;
use crate::close_confirmation_ui::CloseConfirmAction;
use crate::command_history_ui::CommandHistoryAction;
use crate::integrations_ui::IntegrationsResponse;
use crate::paste_special_ui::PasteSpecialAction;
use crate::profile_drawer_ui::ProfileDrawerAction;
use crate::quit_confirmation_ui::QuitConfirmAction;
use crate::remote_shell_install_ui::RemoteShellInstallAction;
use crate::shader_install_ui::ShaderInstallResponse;
use crate::ssh_connect_ui::SshConnectAction;
use crate::tab_bar_ui::TabBarAction;
use crate::tmux_session_picker_ui::SessionPickerAction;
use winit::dpi::PhysicalSize;

/// Snapshot of physical pixel dimensions taken from the renderer before the borrow.
///
/// `pub(crate)` so that sibling modules within `render_pipeline` (e.g. `pane_render`) can
/// accept it as a parameter without duplicating the field list.
#[derive(Clone, Copy)]
pub(crate) struct RendererSizing {
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) content_offset_y: f32,
    pub(crate) content_offset_x: f32,
    pub(crate) content_inset_bottom: f32,
    pub(crate) content_inset_right: f32,
    pub(crate) cell_width: f32,
    pub(crate) cell_height: f32,
    pub(crate) padding: f32,
    pub(crate) status_bar_height: f32,
    pub(crate) scale_factor: f32,
}

/// Data computed during `gather_render_data()` and consumed by the rest of `render()`.
pub(super) struct FrameRenderData {
    /// Processed terminal cells (URL underlines + search highlights applied)
    pub(super) cells: Vec<crate::cell_renderer::Cell>,
    /// Cursor position on screen (col, row), None if hidden
    pub(super) cursor_pos: Option<(usize, usize)>,
    /// Cursor glyph style (from terminal or config overrides)
    pub(super) cursor_style: Option<par_term_emu_core_rust::cursor::CursorStyle>,
    /// Whether alternate screen is active (vim, htop, etc.)
    pub(super) is_alt_screen: bool,
    /// Total scrollback lines available
    pub(super) scrollback_len: usize,
    /// Whether the scrollbar should be shown
    pub(super) show_scrollbar: bool,
    /// Visible grid rows count
    pub(super) visible_lines: usize,
    /// Visible grid columns count
    pub(super) grid_cols: usize,
    /// Scrollback marks (command marks, trigger marks) for scrollbar and separators
    pub(super) scrollback_marks: Vec<crate::scrollback_metadata::ScrollbackMark>,
    /// Total renderable lines (visible + scrollback)
    pub(super) total_lines: usize,
    /// Time spent on URL detection this frame (Zero on cache hit)
    pub(super) debug_url_detect_time: std::time::Duration,
}

/// Actions collected during the egui/GPU render pass to be handled after the renderer borrow ends.
pub(super) struct PostRenderActions {
    pub(super) clipboard: ClipboardHistoryAction,
    pub(super) command_history: CommandHistoryAction,
    pub(super) paste_special: PasteSpecialAction,
    pub(super) session_picker: SessionPickerAction,
    pub(super) tab_action: TabBarAction,
    pub(super) shader_install: ShaderInstallResponse,
    pub(super) integrations: IntegrationsResponse,
    pub(super) search: crate::search::SearchAction,
    pub(super) inspector: InspectorAction,
    pub(super) profile_drawer: ProfileDrawerAction,
    pub(super) close_confirm: CloseConfirmAction,
    pub(super) quit_confirm: QuitConfirmAction,
    pub(super) remote_install: RemoteShellInstallAction,
    pub(super) ssh_connect: SshConnectAction,
    /// Whether config should be saved (debounced) after the render pass
    pub(super) save_config: bool,
}

impl Default for PostRenderActions {
    fn default() -> Self {
        Self {
            clipboard: ClipboardHistoryAction::None,
            command_history: CommandHistoryAction::None,
            paste_special: PasteSpecialAction::None,
            session_picker: SessionPickerAction::None,
            tab_action: TabBarAction::None,
            shader_install: ShaderInstallResponse::None,
            integrations: IntegrationsResponse::default(),
            search: crate::search::SearchAction::None,
            inspector: InspectorAction::None,
            profile_drawer: ProfileDrawerAction::None,
            close_confirm: CloseConfirmAction::None,
            quit_confirm: QuitConfirmAction::None,
            remote_install: RemoteShellInstallAction::None,
            ssh_connect: SshConnectAction::None,
            save_config: false,
        }
    }
}
