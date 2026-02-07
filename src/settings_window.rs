//! Separate settings window for the terminal emulator.
//!
//! This module provides a standalone window for the settings UI,
//! allowing users to configure the terminal while viewing terminal content.

use crate::config::Config;
use crate::settings_ui::{CursorShaderEditorResult, SettingsUI, ShaderEditorResult};
use anyhow::{Context, Result};
use std::sync::Arc;
use wgpu::SurfaceError;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Result of processing a settings window event
#[derive(Debug, Clone)]
pub enum SettingsWindowAction {
    /// No action needed
    None,
    /// Close the settings window
    Close,
    /// Apply config changes to terminal windows (live update)
    ApplyConfig(Config),
    /// Save config to disk
    SaveConfig(Config),
    /// Apply background shader from editor
    ApplyShader(ShaderEditorResult),
    /// Apply cursor shader from editor
    ApplyCursorShader(CursorShaderEditorResult),
    /// Send a test notification to verify permissions
    TestNotification,
    /// Open the profile manager modal
    OpenProfileManager,
    /// Start a coprocess by config index on the active tab
    StartCoprocess(usize),
    /// Stop a coprocess by config index on the active tab
    StopCoprocess(usize),
    /// Open the debug log file in the system's default editor/viewer
    OpenLogFile,
}

/// Manages a separate settings window with its own egui context and wgpu renderer
pub struct SettingsWindow {
    /// The winit window
    window: Arc<Window>,
    /// Window ID for event routing
    window_id: WindowId,
    /// wgpu instance
    #[allow(dead_code)]
    instance: wgpu::Instance,
    /// wgpu surface
    surface: wgpu::Surface<'static>,
    /// wgpu device
    device: Arc<wgpu::Device>,
    /// wgpu queue
    queue: Arc<wgpu::Queue>,
    /// Surface configuration
    surface_config: wgpu::SurfaceConfiguration,
    /// egui context
    egui_ctx: egui::Context,
    /// egui-winit state
    egui_state: egui_winit::State,
    /// egui-wgpu renderer
    egui_renderer: egui_wgpu::Renderer,
    /// Settings UI component
    pub settings_ui: SettingsUI,
    /// Whether the window is ready for rendering
    ready: bool,
    /// Flag to indicate window should close
    should_close: bool,
    /// Pending paste text from menu accelerator (injected into egui next frame)
    pending_paste: Option<String>,
    /// Pending egui events from menu accelerators (Copy, Cut, SelectAll)
    pending_events: Vec<egui::Event>,
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
            .with_inner_size(winit::dpi::LogicalSize::new(700, 800))
            .with_min_inner_size(winit::dpi::LogicalSize::new(500, 400))
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
            instance,
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
        self.settings_ui.config.custom_shader_enabled = custom_shader_enabled;
        self.settings_ui.config.cursor_shader_enabled = cursor_shader_enabled;
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

    /// Render the settings window
    fn render(&mut self) -> SettingsWindowAction {
        if !self.ready {
            return SettingsWindowAction::None;
        }

        // Get surface texture
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return SettingsWindowAction::None;
            }
            Err(SurfaceError::Timeout) => {
                log::warn!("Settings window surface timeout");
                return SettingsWindowAction::None;
            }
            Err(e) => {
                log::error!("Settings window surface error: {:?}", e);
                return SettingsWindowAction::None;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Track settings results
        let mut config_to_save = None;
        let mut config_for_live = None;
        let mut shader_apply = None;
        let mut cursor_shader_apply = None;

        // Run egui
        let mut raw_input = self.egui_state.take_egui_input(&self.window);

        // Inject pending events from menu accelerators (Cmd+V/C/X/A intercepted by muda)
        if let Some(text) = self.pending_paste.take() {
            raw_input.events.push(egui::Event::Paste(text));
        }
        raw_input.events.append(&mut self.pending_events);

        let egui_output = self.egui_ctx.run(raw_input, |ctx| {
            // Show the settings UI as a panel (not a nested window) and capture results
            let (save, live, shader, cursor_shader) = self.settings_ui.show_as_panel(ctx);
            config_to_save = save;
            config_for_live = live;
            shader_apply = shader;
            cursor_shader_apply = cursor_shader;
        });

        // Handle platform output (clipboard, cursor)
        // Manually handle clipboard copy as a fallback for macOS menu accelerator issues.
        // In egui 0.33, copy commands are in platform_output.commands as OutputCommand::CopyText.
        for cmd in &egui_output.platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = cmd
                && let Ok(mut clipboard) = arboard::Clipboard::new()
                && let Err(e) = clipboard.set_text(text)
            {
                log::warn!("Settings window: failed to copy to clipboard: {}", e);
            }
        }
        self.egui_state
            .handle_platform_output(&self.window, egui_output.platform_output.clone());

        // Tessellate shapes
        let paint_jobs = self
            .egui_ctx
            .tessellate(egui_output.shapes, self.egui_ctx.pixels_per_point());

        // Upload egui textures
        for (id, delta) in &egui_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Settings Window Encoder"),
            });

        // Screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        // Update buffers
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render pass
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Settings Window Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.094,
                            g: 0.094,
                            b: 0.094,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Convert to 'static lifetime as required by egui_renderer.render()
            let mut render_pass = render_pass.forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        } // render_pass dropped here

        // Submit
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Free textures
        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        // Check for test notification request
        if self.settings_ui.take_test_notification_request() {
            return SettingsWindowAction::TestNotification;
        }

        // Check for profile manager request
        if self.settings_ui.take_open_profile_manager_request() {
            return SettingsWindowAction::OpenProfileManager;
        }

        // Check for open log file request
        if self.settings_ui.open_log_requested {
            self.settings_ui.open_log_requested = false;
            return SettingsWindowAction::OpenLogFile;
        }

        // Check for coprocess start/stop actions
        if let Some((index, start)) = self.settings_ui.pending_coprocess_actions.pop() {
            log::info!(
                "Settings window: popped coprocess action index={} start={}",
                index,
                start
            );
            // Request another redraw to process remaining actions (if any) and config changes
            self.window.request_redraw();
            return if start {
                SettingsWindowAction::StartCoprocess(index)
            } else {
                SettingsWindowAction::StopCoprocess(index)
            };
        }

        // Determine action based on settings UI results
        if let Some(config) = config_to_save {
            return SettingsWindowAction::SaveConfig(config);
        }
        if let Some(shader_result) = shader_apply {
            return SettingsWindowAction::ApplyShader(shader_result);
        }
        if let Some(cursor_shader_result) = cursor_shader_apply {
            return SettingsWindowAction::ApplyCursorShader(cursor_shader_result);
        }
        if let Some(config) = config_for_live {
            return SettingsWindowAction::ApplyConfig(config);
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
