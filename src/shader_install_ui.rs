//! Shader installation prompt UI.
//!
//! Displays a dialog on first startup when the shaders folder is missing or empty,
//! offering to download and install the shader pack from GitHub releases.

use crate::config::Config;
use egui::{Align2, Color32, Context, Frame, RichText, Window, epaint::Shadow};

/// User's response to the shader install prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderInstallResponse {
    /// User clicked "Yes, Install" - download and install shaders
    Install,
    /// User clicked "Never" - save preference to config
    Never,
    /// User clicked "Later" - dismiss for this session only
    Later,
    /// No response yet (dialog still showing or not shown)
    None,
}

/// Shader install dialog UI manager
pub struct ShaderInstallUI {
    /// Whether the dialog is currently visible
    pub visible: bool,
    /// Whether installation is in progress
    pub installing: bool,
    /// Installation progress message
    pub progress_message: Option<String>,
    /// Installation error message
    pub error_message: Option<String>,
    /// Installation success message
    pub success_message: Option<String>,
}

impl ShaderInstallUI {
    /// Create a new shader install UI
    pub fn new() -> Self {
        Self {
            visible: false,
            installing: false,
            progress_message: None,
            error_message: None,
            success_message: None,
        }
    }

    /// Show the dialog
    pub fn show_dialog(&mut self) {
        self.visible = true;
        self.installing = false;
        self.progress_message = None;
        self.error_message = None;
        self.success_message = None;
    }

    /// Hide the dialog
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Render the shader install dialog
    /// Returns the user's response
    pub fn show(&mut self, ctx: &Context) -> ShaderInstallResponse {
        if !self.visible {
            return ShaderInstallResponse::None;
        }

        let mut response = ShaderInstallResponse::None;

        // Ensure dialog is fully opaque
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(32, 32, 32, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let viewport = ctx.input(|i| i.viewport_rect());

        Window::new("Shader Pack Available")
            .resizable(false)
            .collapsible(false)
            .default_width(450.0)
            .default_pos(viewport.center())
            .pivot(Align2::CENTER_CENTER)
            .frame(
                Frame::window(&ctx.style())
                    .fill(solid_bg)
                    .inner_margin(20.0)
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(80)))
                    .shadow(Shadow {
                        offset: [4, 4],
                        blur: 16,
                        spread: 4,
                        color: Color32::from_black_alpha(180),
                    }),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Header with icon
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Custom Shaders Available")
                            .size(20.0)
                            .strong(),
                    );
                    ui.add_space(16.0);
                });

                // Description
                ui.label(
                    "par-term includes 49+ custom background shaders and 12 cursor \
                     effect shaders that can transform your terminal experience.",
                );
                ui.add_space(8.0);

                ui.label("Effects include:");
                ui.indent("effects_list", |ui| {
                    ui.label("- CRT monitors, scanlines, and retro effects");
                    ui.label("- Matrix rain, starfields, and particle systems");
                    ui.label("- Plasma, fire, and abstract visualizations");
                    ui.label("- Cursor trails, glows, and ripple effects");
                });

                ui.add_space(16.0);

                // Show installation progress/error/success
                if self.installing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            self.progress_message
                                .as_deref()
                                .unwrap_or("Installing shaders..."),
                        );
                    });
                } else if let Some(error) = &self.error_message {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), error);
                    ui.add_space(8.0);
                    ui.label("You can try again later using: par-term install-shaders");
                } else if let Some(success) = &self.success_message {
                    ui.colored_label(Color32::from_rgb(100, 255, 100), success);
                    ui.add_space(8.0);
                    ui.label("Configure shaders in Settings (F12) under 'Background & Effects'.");
                }

                ui.add_space(16.0);

                // Buttons (centered)
                ui.vertical_centered(|ui| {
                    // Don't show buttons during installation or after success
                    if !self.installing && self.success_message.is_none() {
                        ui.horizontal(|ui| {
                            // Calculate button width for uniform sizing
                            let button_width = 120.0;

                            if ui
                                .add_sized([button_width, 32.0], egui::Button::new("Yes, Install"))
                                .clicked()
                            {
                                response = ShaderInstallResponse::Install;
                            }

                            ui.add_space(8.0);

                            if ui
                                .add_sized([button_width, 32.0], egui::Button::new("Never"))
                                .on_hover_text("Don't ask again")
                                .clicked()
                            {
                                response = ShaderInstallResponse::Never;
                            }

                            ui.add_space(8.0);

                            if ui
                                .add_sized([button_width, 32.0], egui::Button::new("Later"))
                                .on_hover_text("Ask again next time")
                                .clicked()
                            {
                                response = ShaderInstallResponse::Later;
                            }
                        });
                    } else if self.success_message.is_some() {
                        // Show OK button after successful install
                        if ui
                            .add_sized([120.0, 32.0], egui::Button::new("OK"))
                            .clicked()
                        {
                            self.visible = false;
                        }
                    }
                });

                ui.add_space(8.0);

                // Help text
                if !self.installing
                    && self.success_message.is_none()
                    && self.error_message.is_none()
                {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(
                                "You can always install later with: par-term install-shaders",
                            )
                            .weak()
                            .small(),
                        );
                    });
                }
            });

        response
    }

    /// Set installation in progress
    pub fn set_installing(&mut self, message: &str) {
        self.installing = true;
        self.progress_message = Some(message.to_string());
        self.error_message = None;
    }

    /// Set installation error
    pub fn set_error(&mut self, error: &str) {
        self.installing = false;
        self.progress_message = None;
        self.error_message = Some(error.to_string());
    }

    /// Set installation success
    pub fn set_success(&mut self, message: &str) {
        self.installing = false;
        self.progress_message = None;
        self.error_message = None;
        self.success_message = Some(message.to_string());
    }
}

impl Default for ShaderInstallUI {
    fn default() -> Self {
        Self::new()
    }
}

/// Install shaders from GitHub release (reuses CLI logic)
/// This is a blocking operation that downloads and extracts shaders
pub fn install_shaders_headless() -> Result<usize, String> {
    const REPO: &str = "paulrobello/par-term";
    let shaders_dir = Config::shaders_dir();

    // Fetch latest release info
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", REPO);
    let download_url = get_shaders_download_url(&api_url, REPO)?;

    // Download the zip file
    let zip_data = download_file(&download_url)?;

    // Create shaders directory if it doesn't exist
    std::fs::create_dir_all(&shaders_dir)
        .map_err(|e| format!("Failed to create shaders directory: {}", e))?;

    // Extract shaders
    extract_shaders(&zip_data, &shaders_dir)?;

    // Count installed shaders
    let count = count_shader_files(&shaders_dir);

    Ok(count)
}

/// Get the download URL for shaders.zip from the latest release
fn get_shaders_download_url(api_url: &str, repo: &str) -> Result<String, String> {
    let mut body = ureq::get(api_url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to fetch release info: {}", e))?
        .into_body();

    let body_str = body
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    // Parse JSON to find shaders.zip browser_download_url
    // We need the browser_download_url, not the api url
    let search_pattern = "\"browser_download_url\":\"";
    let target_file = "shaders.zip";

    // Find the shaders.zip entry by looking for browser_download_url containing shaders.zip
    for (i, _) in body_str.match_indices(search_pattern) {
        let url_start = i + search_pattern.len();
        if let Some(url_end) = body_str[url_start..].find('"') {
            let url = &body_str[url_start..url_start + url_end];
            if url.ends_with(target_file) {
                return Ok(url.to_string());
            }
        }
    }

    Err(format!(
        "Could not find shaders.zip in the latest release.\n\
         Please check https://github.com/{}/releases",
        repo
    ))
}

/// Download a file from URL and return its contents
fn download_file(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut body = ureq::get(url)
        .header("User-Agent", "par-term")
        .call()
        .map_err(|e| format!("Failed to download file: {}", e))?
        .into_body();

    let mut bytes = Vec::new();
    body.as_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read download: {}", e))?;

    Ok(bytes)
}

/// Extract shaders from zip data to target directory
fn extract_shaders(zip_data: &[u8], target_dir: &std::path::Path) -> Result<(), String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let reader = Cursor::new(zip_data);
    let mut archive = ZipArchive::new(reader).map_err(|e| format!("Failed to open zip: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        if file.is_dir() {
            continue;
        }

        // Handle paths - the zip contains "shaders/" prefix
        let relative_path = outpath.strip_prefix("shaders/").unwrap_or(&outpath);

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let final_path = target_dir.join(relative_path);

        // Create parent directories if needed
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        // Extract file
        let mut outfile = std::fs::File::create(&final_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|e| format!("Failed to write file: {}", e))?;
    }

    Ok(())
}

/// Count .glsl files in directory
fn count_shader_files(dir: &std::path::Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension()
                && ext == "glsl"
            {
                count += 1;
            }
        }
    }
    count
}
