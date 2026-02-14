//! Profile drawer UI using egui
//!
//! Provides a collapsible right-side drawer for quick profile access.

use crate::config::Config;
use crate::profile::{ProfileId, ProfileManager};

/// Actions that can be triggered from the profile drawer
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileDrawerAction {
    /// No action
    None,
    /// Open a profile (create new tab from profile)
    OpenProfile(ProfileId),
    /// Open the profile management modal
    ManageProfiles,
}

/// Profile drawer UI state
pub struct ProfileDrawerUI {
    /// Whether the drawer is expanded (visible)
    pub expanded: bool,
    /// Currently selected profile ID
    pub selected: Option<ProfileId>,
    /// Currently hovered profile ID
    pub hovered: Option<ProfileId>,
    /// Drawer width in pixels
    pub width: f32,
    /// Tag filter text (for searching/filtering profiles by tags)
    pub tag_filter: String,
}

impl ProfileDrawerUI {
    /// Default drawer width
    const DEFAULT_WIDTH: f32 = 220.0;
    /// Collapsed tab width (the toggle button)
    const COLLAPSED_WIDTH: f32 = 12.0;
    /// Toggle button height
    const BUTTON_HEIGHT: f32 = 30.0;

    /// Create a new profile drawer UI (collapsed by default)
    pub fn new() -> Self {
        Self {
            expanded: false,
            selected: None,
            hovered: None,
            width: Self::DEFAULT_WIDTH,
            tag_filter: String::new(),
        }
    }

    /// Calculate the toggle button rectangle given the window size
    pub fn get_toggle_button_rect(
        &self,
        window_width: f32,
        window_height: f32,
    ) -> (f32, f32, f32, f32) {
        // When expanded, button is at left edge of drawer; when collapsed, at right edge of window
        let x = if self.expanded {
            window_width - self.width - Self::COLLAPSED_WIDTH - 2.0
        } else {
            window_width - Self::COLLAPSED_WIDTH - 2.0
        };
        let y = (window_height - Self::BUTTON_HEIGHT) / 2.0;
        (x, y, Self::COLLAPSED_WIDTH, Self::BUTTON_HEIGHT)
    }

    /// Check if a point (in window coordinates) is inside the toggle button
    pub fn is_point_in_toggle_button(
        &self,
        px: f32,
        py: f32,
        window_width: f32,
        window_height: f32,
    ) -> bool {
        let (x, y, w, h) = self.get_toggle_button_rect(window_width, window_height);
        px >= x && px <= x + w && py >= y && py <= y + h
    }

    /// Toggle drawer expanded state
    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        log::info!(
            "Profile drawer toggled: {}",
            if self.expanded {
                "expanded"
            } else {
                "collapsed"
            }
        );
    }

    /// Render the profile drawer and return any action triggered
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        profile_manager: &ProfileManager,
        config: &Config,
        modal_visible: bool,
    ) -> ProfileDrawerAction {
        let mut action = ProfileDrawerAction::None;
        let mut toggle_clicked = false;

        // Render the side panel FIRST if expanded, so we get the current width
        // This ensures the toggle button position is accurate during resize
        let panel_rect = if self.expanded {
            let response = egui::SidePanel::right("profile_drawer")
                .resizable(true)
                .default_width(self.width)
                .min_width(180.0)
                .max_width(400.0)
                .frame(
                    egui::Frame::side_top_panel(&ctx.style())
                        .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 245))
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ctx, |ui| {
                    self.render_panel_contents(ui, profile_manager, &mut action);
                });

            // Update width from the panel's actual rect
            self.width = response.response.rect.width();
            Some(response.response.rect)
        } else {
            None
        };

        // Calculate toggle button position using the actual panel rect
        let button_width = Self::COLLAPSED_WIDTH;
        let button_height = Self::BUTTON_HEIGHT;
        let viewport_rect = ctx.input(|i| i.viewport_rect());

        let button_x = if let Some(rect) = panel_rect {
            // Position at left edge of the actual panel rect
            rect.left() - button_width - 2.0
        } else {
            // Collapsed: position at right edge of window
            viewport_rect.right() - button_width - 2.0
        };

        let button_rect = egui::Rect::from_min_size(
            egui::pos2(button_x, viewport_rect.center().y - button_height / 2.0),
            egui::vec2(button_width, button_height),
        );

        // Render toggle button (skip if modal is open to avoid z-order issues,
        // or if profile drawer button is disabled in config)
        if !modal_visible && config.show_profile_drawer_button {
            egui::Area::new(egui::Id::new("profile_drawer_toggle_area"))
                .fixed_pos(button_rect.min)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let response = ui.allocate_response(button_rect.size(), egui::Sense::click());

                    let bg_color = if response.hovered() {
                        egui::Color32::from_rgba_unmultiplied(60, 60, 60, 220)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200)
                    };

                    ui.painter().rect_filled(response.rect, 4.0, bg_color);

                    let arrow = if self.expanded { "▶" } else { "◀" };
                    ui.painter().text(
                        response.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        arrow,
                        egui::FontId::proportional(7.0),
                        egui::Color32::WHITE,
                    );

                    // Use clicked_by to only respond to mouse clicks, not keyboard Enter/Space
                    // This prevents Enter key in the terminal from toggling the drawer
                    if response.clicked_by(egui::PointerButton::Primary) {
                        toggle_clicked = true;
                    }
                });

            if toggle_clicked {
                self.toggle();
                ctx.request_repaint();
            }
        }

        action
    }

    /// Render the panel contents (extracted to allow panel-first rendering)
    fn render_panel_contents(
        &mut self,
        ui: &mut egui::Ui,
        profile_manager: &ProfileManager,
        action: &mut ProfileDrawerAction,
    ) {
        // Header
        ui.horizontal(|ui| {
            ui.heading("Profiles");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Manage").clicked() {
                    *action = ProfileDrawerAction::ManageProfiles;
                }
            });
        });

        // Tag filter (search box)
        ui.horizontal(|ui| {
            ui.label("🔍");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.tag_filter)
                    .hint_text("Filter by tag or name...")
                    .desired_width(ui.available_width() - 20.0),
            );
            if response.changed() {
                // Clear selection when filter changes
                self.selected = None;
            }
        });
        ui.separator();

        // Profile list
        crate::debug_info!(
            "PROFILE",
            "Profile drawer render: {} profiles",
            profile_manager.len()
        );
        if profile_manager.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("No profiles")
                        .italics()
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(10.0);
                if ui.button("Create Profile").clicked() {
                    *action = ProfileDrawerAction::ManageProfiles;
                }
            });
        } else {
            // Get filtered profiles
            let filtered_profiles = profile_manager.filter_by_tags(&self.tag_filter);

            // Reserve space for the action buttons at the bottom
            let available = ui.available_height();
            let button_area_height = 40.0;
            let scroll_height = (available - button_area_height).max(100.0);

            if filtered_profiles.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("No matching profiles")
                            .italics()
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(10.0);
                    if ui.small_button("Clear filter").clicked() {
                        self.tag_filter.clear();
                    }
                });
            } else {
                // Scrollable profile list
                egui::ScrollArea::vertical()
                    .max_height(scroll_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for profile in filtered_profiles {
                            let is_selected = self.selected == Some(profile.id);
                            let is_dynamic = profile.source.is_dynamic();

                            // Build the label text
                            let label = if let Some(icon) = &profile.icon {
                                format!("{} {}", icon, profile.name)
                            } else {
                                profile.name.clone()
                            };

                            // Add indicator for profiles with custom settings
                            let has_custom = profile.command.is_some()
                                || profile.working_directory.is_some()
                                || profile.parent_id.is_some();
                            let label = if has_custom {
                                format!("{} ...", label)
                            } else {
                                label
                            };

                            ui.horizontal(|ui| {
                                // Use selectable_label which has reliable click handling
                                let response = ui.selectable_label(is_selected, &label);

                                // Dynamic profile indicator
                                if is_dynamic {
                                    ui.label(
                                        egui::RichText::new("[dynamic]")
                                            .color(egui::Color32::from_rgb(100, 180, 255))
                                            .small(),
                                    );
                                }

                                // Single click selects
                                if response.clicked() {
                                    self.selected = Some(profile.id);
                                }

                                // Double click opens (using egui's built-in detection)
                                if response.double_clicked() {
                                    *action = ProfileDrawerAction::OpenProfile(profile.id);
                                }

                                // Show keyboard shortcut if defined
                                if let Some(shortcut) = &profile.keyboard_shortcut {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(shortcut)
                                                    .small()
                                                    .color(egui::Color32::DARK_GRAY),
                                            );
                                        },
                                    );
                                }
                            });

                            // Show tags as small labels below the profile name
                            if !profile.tags.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.add_space(16.0); // Indent
                                    for tag in &profile.tags {
                                        ui.label(
                                            egui::RichText::new(format!("#{}", tag))
                                                .small()
                                                .color(egui::Color32::from_rgb(100, 150, 200)),
                                        );
                                    }
                                });
                            }
                        }
                    });
            }

            // Action buttons (always visible at bottom)
            ui.separator();
            ui.horizontal(|ui| {
                let open_enabled = self.selected.is_some();
                if ui
                    .add_enabled(open_enabled, egui::Button::new("Open"))
                    .clicked()
                    && let Some(id) = self.selected
                {
                    *action = ProfileDrawerAction::OpenProfile(id);
                }
            });
        }
    }
}

impl Default for ProfileDrawerUI {
    fn default() -> Self {
        Self::new()
    }
}
