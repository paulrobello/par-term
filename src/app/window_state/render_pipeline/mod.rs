//! GPU render pipeline for WindowState.
//!
//! Contains the full rendering cycle:
//! - `render`: per-frame orchestration entry point
//! - `frame_setup`: `should_render_frame`, `update_frame_metrics`, `update_animations`, `sync_layout`
//! - `gather_data`: `gather_render_data` — snapshot terminal state into `FrameRenderData`
//! - `gpu_submit`: `submit_gpu_frame` — egui + wgpu render pass, returns `PostRenderActions`
//! - `post_render`: `update_post_render_state` — dispatch post-render action queue
//! - `pane_render`: `gather_pane_render_data` + `render_split_panes_with_data` + `PaneRenderData`
//! - `egui_overlays`: standalone egui overlay renderers (FPS, resize, toast, tooltip, pane-id)
//! - `types`: shared data-transfer types (`RendererSizing`, `FrameRenderData`, `PostRenderActions`)

mod claude_code_bridge;
mod egui_overlays;
mod frame_setup;
mod gather_data;
mod gpu_submit;
mod pane_render;
mod post_render;
mod prettifier_cells;
mod renderer_ops;
mod tab_snapshot;
mod types;
mod viewport;

use types::{FrameRenderData, PostRenderActions};

use crate::app::window_state::WindowState;
use crate::config::ShaderInstallPrompt;

impl WindowState {
    /// Main render function for this window
    pub(crate) fn render(&mut self) {
        // Skip rendering if shutting down
        if self.is_shutting_down {
            return;
        }

        if !self.should_render_frame() {
            return;
        }

        self.update_frame_metrics();
        self.update_animations();
        self.sync_layout();

        let Some(frame_data) = self.gather_render_data() else {
            return;
        };

        let actions = self.submit_gpu_frame(frame_data);
        self.update_post_render_state(actions);

        // Process any pending config saves that were deferred by debouncing
        self.process_pending_config_save();
    }
}
