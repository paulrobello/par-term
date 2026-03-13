//! Separate settings window for the terminal emulator.
//!
//! This module provides a standalone window for the settings UI,
//! allowing users to configure the terminal while viewing terminal content.

mod render;

use crate::settings_ui::SettingsUI;
use anyhow::{Context, Result};
use par_term_config::Config;

// Re-export SettingsWindowAction so the rest of the crate can use it via
// `crate::settings_window::SettingsWindowAction` as before.
pub use crate::settings_ui::SettingsWindowAction;
use std::sync::Arc;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Manages a separate settings window with its own egui context and wgpu renderer
pub struct SettingsWindow {
    /// The winit window
    pub(super) window: Arc<Window>,
    /// Window ID for event routing
    window_id: WindowId,
    /// wgpu surface
    pub(super) surface: wgpu::Surface<'static>,
    /// wgpu device
    pub(super) device: Arc<wgpu::Device>,
    /// wgpu queue
    pub(super) queue: Arc<wgpu::Queue>,
    /// Surface configuration
    pub(super) surface_config: wgpu::SurfaceConfiguration,
    /// egui context
    pub(super) egui_ctx: egui::Context,
    /// egui-winit state
    pub(super) egui_state: egui_winit::State,
    /// egui-wgpu renderer
    pub(super) egui_renderer: egui_wgpu::Renderer,
    /// Settings UI component
    pub settings_ui: SettingsUI,
    /// Whether the window is ready for rendering
    pub(super) ready: bool,
    /// Flag to indicate window should close
    should_close: bool,
    /// Pending paste text from menu accelerator (injected into egui next frame)
    pub(super) pending_paste: Option<String>,
    /// Pending egui events from menu accelerators (Copy, Cut, SelectAll)
    pub(super) pending_events: Vec<egui::Event>,
}

impl SettingsWindow {
    /// Create a new settings window
    pub async fn new(
        event_loop: &ActiveEventLoop,
        config: Config,
        supported_vsync_modes: Vec<crate::config::VsyncMode>,
    ) -> Result<Self> {
        // Create the window
        let window_attrs = Window::default_attributes()
            .with_title("Settings")
            .with_inner_size(winit::dpi::LogicalSize::new(770, 800))
            .with_min_inner_size(winit::dpi::LogicalSize::new(550, 400))
            .with_resizable(true);

        let window = Arc::new(event_loop.create_window(window_attrs)?);
        let window_id = window.id();
        let size = window.inner_size();

        // Create wgpu instance
        // Platform-specific backend selection for better VM compatibility
        #[cfg(target_os = "windows")]
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        #[cfg(target_os = "macos")]
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        #[cfg(target_os = "linux")]
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone())?;

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find suitable GPU adapter")?;

        // Request device
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Select alpha mode for window transparency (consistent with main window)
        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Auto)
        {
            wgpu::CompositeAlphaMode::Auto
        } else {
            surface_caps.alpha_modes[0]
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Initialize egui
        let scale_factor = window.scale_factor() as f32;
        let egui_ctx = egui::Context::default();
        crate::settings_ui::nerd_font::configure_nerd_font(&egui_ctx);
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(scale_factor),
            None,
            None,
        );

        // Create egui renderer
        let egui_renderer = egui_wgpu::Renderer::new(
            &device,
            surface_format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );

        // Create settings UI
        let mut settings_ui = SettingsUI::new(config);
        settings_ui.visible = true; // Always visible in settings window
        settings_ui.update_supported_vsync_modes(supported_vsync_modes);

        Ok(Self {
            window,
            window_id,
            surface,
            device,
            queue,
            surface_config,
            egui_ctx,
            egui_state,
            egui_renderer,
            settings_ui,
            ready: true,
            should_close: false,
            pending_paste: None,
            pending_events: Vec::new(),
        })
    }

    /// Get the window ID
    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Check if the window should close
    pub fn should_close(&self) -> bool {
        self.should_close
    }

    /// Queue a paste event for the next egui frame.
    ///
    /// Used when the macOS menu accelerator intercepts Cmd+V before
    /// the keypress reaches egui.
    pub fn inject_paste(&mut self, text: String) {
        self.pending_paste = Some(text);
        self.window.request_redraw();
    }

    /// Queue an egui event for the next frame.
    ///
    /// Used when menu accelerators intercept Cmd+C, Cmd+X, Cmd+A, etc.
    /// before egui sees them.
    pub fn inject_event(&mut self, event: egui::Event) {
        self.pending_events.push(event);
        self.window.request_redraw();
    }

    /// Update the config in the settings UI
    pub fn update_config(&mut self, config: Config) {
        self.settings_ui.update_config(config);
    }

    /// Force-update the config, bypassing the `has_changes` guard.
    /// Used when the ACP agent changes config — must propagate even if
    /// the user has pending edits in the settings window.
    pub fn force_update_config(&mut self, config: Config) {
        self.settings_ui.force_update_config(config);
    }

    /// Set a shader compilation error message
    pub fn set_shader_error(&mut self, error: Option<String>) {
        self.settings_ui.set_shader_error(error);
    }

    /// Set a cursor shader compilation error message
    pub fn set_cursor_shader_error(&mut self, error: Option<String>) {
        self.settings_ui.set_cursor_shader_error(error);
    }

    /// Clear shader error
    pub fn clear_shader_error(&mut self) {
        self.settings_ui.clear_shader_error();
    }

    /// Clear cursor shader error
    pub fn clear_cursor_shader_error(&mut self) {
        self.settings_ui.clear_cursor_shader_error();
    }

    /// Sync shader enabled states from external source (e.g., keybinding toggle)
    /// This prevents the settings window from overwriting externally toggled states
    pub fn sync_shader_states(&mut self, custom_shader_enabled: bool, cursor_shader_enabled: bool) {
        self.settings_ui.config.shader.custom_shader_enabled = custom_shader_enabled;
        self.settings_ui.config.shader.cursor_shader_enabled = cursor_shader_enabled;
    }

    /// Handle a window event
    pub fn handle_window_event(&mut self, event: WindowEvent) -> SettingsWindowAction {
        // Let egui handle the event
        let event_response = self.egui_state.on_window_event(&self.window, &event);

        match event {
            WindowEvent::CloseRequested => {
                self.should_close = true;
                return SettingsWindowAction::Close;
            }

            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    self.surface_config.width = new_size.width;
                    self.surface_config.height = new_size.height;
                    self.surface.configure(&self.device, &self.surface_config);
                    self.window.request_redraw();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Handle Escape to close window (if egui didn't consume it)
                if !event_response.consumed
                    && event.state.is_pressed()
                    && matches!(event.logical_key, Key::Named(NamedKey::Escape))
                {
                    // Only close if no shader editor is open
                    if !self.settings_ui.shader_editor_visible
                        && !self.settings_ui.cursor_shader_editor_visible
                    {
                        self.should_close = true;
                        return SettingsWindowAction::Close;
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                return self.render();
            }

            _ => {}
        }

        // Request redraw if egui needs it
        if event_response.repaint {
            self.window.request_redraw();
        }

        SettingsWindowAction::None
    }

    /// Request a redraw
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Bring the window to the front and focus it
    pub fn focus(&self) {
        self.window.focus_window();
        self.window.request_redraw();
    }

    /// Check if the settings window currently has focus.
    pub fn is_focused(&self) -> bool {
        self.window.has_focus()
    }
}
