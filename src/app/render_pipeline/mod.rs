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
mod egui_submit;
mod frame_setup;
mod gather_data;
mod gather_phases;
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

        let render_t0 = std::time::Instant::now();

        if !self.should_render_frame() {
            return;
        }

        self.update_frame_metrics();

        let t_anim = std::time::Instant::now();
        self.update_animations();
        let anim_ms = t_anim.elapsed().as_millis();

        let t_layout = std::time::Instant::now();
        self.sync_layout();
        let layout_ms = t_layout.elapsed().as_millis();

        let t_gather = std::time::Instant::now();
        let Some(frame_data) = self.gather_render_data() else {
            return;
        };
        let gather_ms = t_gather.elapsed().as_millis();

        let t_gpu = std::time::Instant::now();
        let actions = self.submit_gpu_frame(frame_data);
        let gpu_ms = t_gpu.elapsed().as_millis();

        let t_post = std::time::Instant::now();
        self.update_post_render_state(actions);
        self.process_pending_config_save();
        let post_ms = t_post.elapsed().as_millis();

        let total_ms = render_t0.elapsed().as_millis();
        if total_ms > 16 {
            crate::debug_info!(
                "FRAME_TIMING",
                "slow frame: total={}ms (anim={} layout={} gather={} gpu={} post={})",
                total_ms,
                anim_ms,
                layout_ms,
                gather_ms,
                gpu_ms,
                post_ms
            );
        }
    }
}
