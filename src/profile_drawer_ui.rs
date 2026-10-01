//! The Profiles drawer (UX.md PR2): Open Profiles… pinned to the window's
//! right edge.
//!
//! The drawer is a panel shell — its open state, width, and edge toggle
//! button — around the launcher's own list ([`ProfileLauncherUI`]): the same
//! rows, keys, and actions as the popup. The terminal reflows beside it
//! (`WindowState::profile_drawer_inset`) instead of drawing under it, it
//! joins the overlay stack as a Panel (keys go to it only while its filter
//! holds focus), and Escape in the filter closes it.

use crate::config::Config;
use crate::profile_launcher_ui::{LauncherChoice, ProfileLauncherUI};
use crate::ui_constants::{PROFILE_DRAWER_MAX_WIDTH, PROFILE_DRAWER_MIN_WIDTH};

/// The drawer's panel state.
pub struct ProfileDrawerUI {
    /// Whether the drawer is expanded (visible)
    pub expanded: bool,
    /// Drawer width in pixels
    pub width: f32,
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
            width: Self::DEFAULT_WIDTH,
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

    /// The width the open drawer takes from the terminal, in logical
    /// pixels (0 when collapsed): the panel plus its edge button.
    pub fn consumed_width(&self) -> f32 {
        if self.expanded {
            self.width + Self::COLLAPSED_WIDTH + 2.0
        } else {
            0.0
        }
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

    /// Render the drawer and return what its launcher list chose.
    ///
    /// `bottom_margin` should be set to the height of any floating status bar
    /// (e.g. the custom status bar rendered as an `egui::Area`) so the side
    /// panel stops above it rather than extending behind it.
    pub fn render(
        &mut self,
        ctx: &mut egui::Ui,
        launcher: &mut ProfileLauncherUI,
        config: &Config,
        bottom_margin: f32,
    ) -> Option<LauncherChoice> {
        let mut choice = None;
        let mut toggle_clicked = false;

        // Reserve space for any floating status bar rendered via egui::Area.
        // egui::Area does not participate in the panel layout, so without this
        // spacer the SidePanel would extend behind the status bar.
        if bottom_margin > 0.0 && self.expanded {
            egui::Panel::bottom("profile_drawer_bottom_margin")
                .exact_size(bottom_margin)
                .frame(egui::Frame::NONE)
                .show(ctx, |_ui| {});
        }

        // Render the side panel FIRST if expanded, so we get the current width
        // This ensures the toggle button position is accurate during resize
        let panel_rect = if self.expanded {
            let response = egui::Panel::right("profile_drawer")
                .resizable(true)
                .default_size(self.width)
                .min_size(PROFILE_DRAWER_MIN_WIDTH)
                .max_size(PROFILE_DRAWER_MAX_WIDTH)
                .frame(
                    egui::Frame::side_top_panel(&ctx.global_style())
                        .fill(crate::app::overlay::theme::PANEL_FILL)
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ctx, |ui| {
                    let (picked, close) = launcher.show_pinned(ui);
                    choice = picked;
                    if close {
                        self.expanded = false;
                    }
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

        // Render toggle button (skip if the button is disabled in config)
        if config.tabs.show_profile_drawer_button {
            egui::Area::new(egui::Id::new("profile_drawer_toggle_area"))
                .fixed_pos(button_rect.min)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let response = ui.allocate_response(button_rect.size(), egui::Sense::click());

                    let bg_color = if response.hovered() {
                        crate::app::overlay::theme::DETAIL_FILL
                    } else {
                        crate::app::overlay::theme::TOAST_FILL
                    };

                    ui.painter().rect_filled(response.rect, 4.0, bg_color);

                    let arrow = if self.expanded { "▶" } else { "◀" };
                    ui.painter().text(
                        response.rect.center(),
                        egui::Align2::CENTER_CENTER,
                        arrow,
                        egui::FontId::proportional(7.0),
                        crate::app::overlay::theme::TEXT_ON_FILL,
                    );

                    // Use clicked_by to only respond to mouse clicks, not keyboard Enter/Space
                    // This prevents Enter key in the terminal from toggling the drawer
                    if response.clicked_by(egui::PointerButton::Primary) {
                        toggle_clicked = true;
                    }
                    response.on_hover_text("Profiles");
                });

            if toggle_clicked {
                self.toggle();
                if self.expanded {
                    launcher.prepare_pinned();
                }
                ctx.request_repaint();
            }
        }

        choice
    }
}

impl Default for ProfileDrawerUI {
    fn default() -> Self {
        Self::new()
    }
}
