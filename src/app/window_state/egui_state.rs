//! egui overlay state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

/// State for the egui GUI overlay layer: context, input handling, and lifecycle.
///
/// egui is used for the tab bar, modal dialogs, and the AI inspector side-panel.
/// These four fields are always accessed together and have no meaningful relationship
/// to the terminal rendering or PTY logic, making them a natural extraction candidate.
#[derive(Default)]
pub(crate) struct EguiState {
    /// egui context for GUI rendering (None before renderer initialization)
    pub(crate) ctx: Option<egui::Context>,
    /// egui-winit state for event handling (None before renderer initialization)
    pub(crate) state: Option<egui_winit::State>,
    /// Pending egui events to inject into the next frame's `raw_input`.
    ///
    /// Used to inject synthetic events (paste, copy, clipboard image) that cannot
    /// be delivered via the normal winit event path (e.g. macOS menu accelerators
    /// intercept Cmd+V before winit sees them).
    pub(crate) pending_events: Vec<egui::Event>,
    /// Whether egui has completed its first `ctx.run()` call.
    ///
    /// Before the first run, egui state is unreliable — `is_using_pointer()` may
    /// return false even when egui owns the cursor. Code that gates on egui state
    /// should check this flag first.
    pub(crate) initialized: bool,
}

