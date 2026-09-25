//! Interactive plugin overlay rendering (overlay kind, phase 2).
//!
//! Display-only overlays paint shapes in [`super::plugin_overlay_render`]; an
//! overlay whose manifest carries the `overlay.interactive` capability
//! AND whose `SetOverlay` requested `interactive: true` renders here
//! instead — real egui widgets inside a fixed [`egui::Area`] at the
//! overlay's resolved rect.
//!
//! Focus follows the design's mode stack: an overlay never gains focus on
//! appear (O4) — a click on one of its widgets focuses it, Escape returns
//! focus to the terminal, and at most one overlay is focused at a time.
//! Widget interactions never run host code inside the egui closure (the
//! closure borrows `*self`); they are collected as [`OverlayInteraction`]s
//! and applied after the closure returns.

use par_term_scripting::protocol::{OverlayScene, OverlayWidgetEvent, PluginOverlay};

/// One widget interaction the renderer collected while drawing a focused
/// overlay's widgets. Applied (converted to a semantic event written to the
/// plugin's stdin) after the egui closure, never inside it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OverlayInteraction {
    /// Plugin id whose overlay was interacted with.
    pub(crate) plugin_id: String,
    /// The widget's scene id.
    pub(crate) widget_id: String,
    /// What happened.
    pub(crate) event: OverlayWidgetEvent,
    /// True when a widget claimed the click that starts focus (the host
    /// focuses this overlay).
    pub(crate) focus_request: bool,
}

/// Render one interactive overlay: panel frame plus widget scene.
///
/// `focused` is the host's current focus state for this overlay. When
/// false, widgets render inert (visually identical, unclickable) — the
/// overlay gains focus only through a click on the panel, which is
/// collected as a focus request rather than applied here.
pub(super) fn render_interactive_overlay(
    ctx: &egui::Context,
    plugin_id: &str,
    overlay: &PluginOverlay,
    rect: egui::Rect,
    interactions: &mut Vec<OverlayInteraction>,
    focused: bool,
) {
    // A fixed-position Area (not anchored) at the resolved rect keeps the
    // interactive layer exactly where the display-only painter would have
    // drawn it; `Order::Middle` preserves the mode-stack contract (below
    // modal chrome, above terminal content).
    egui::Area::new(egui::Id::new(("plugin_overlay_interactive", plugin_id)))
        .fixed_pos(rect.min)
        .order(egui::Order::Middle)
        .interactable(true)
        .show(ctx, |ui| {
            // Constrain the Area to the resolved overlay size so widgets
            // wrap rather than growing the panel.
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());

            let frame = egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(
                    20,
                    20,
                    28,
                    (220.0 * overlay.opacity) as u8,
                ))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(6.0)
                .stroke(egui::Stroke::new(
                    if focused { 2.0 } else { 1.0 },
                    if focused {
                        egui::Color32::from_rgba_unmultiplied(
                            130,
                            160,
                            255,
                            (230.0 * overlay.opacity) as u8,
                        )
                    } else {
                        egui::Color32::from_rgba_unmultiplied(
                            120,
                            140,
                            190,
                            (200.0 * overlay.opacity) as u8,
                        )
                    },
                ));
            frame.show(ui, |ui| {
                render_widget_scene(ui, plugin_id, &overlay.content, interactions, focused);
            })
        });

    // The Area body's response is inside the closure; collect the focus
    // request through the interactions vec instead. A click that egui
    // routed to a widget reports via that widget's response; a click on
    // the panel frame itself is detected through the frame's response.
    let panel_clicked = ctx.input(|i| {
        i.pointer.any_click() && rect.contains(i.pointer.interact_pos().unwrap_or_default())
    });
    if panel_clicked && !focused {
        interactions.push(OverlayInteraction {
            plugin_id: plugin_id.to_string(),
            widget_id: String::new(),
            event: OverlayWidgetEvent::Click {},
            focus_request: true,
        });
    }
}

/// Render the interactive widget scene. Display-only nodes (text, row,
/// markdown) render through the same egui widgets the settings UI uses so
/// an interactive overlay is not limited to buttons.
fn render_widget_scene(
    ui: &mut egui::Ui,
    plugin_id: &str,
    scene: &OverlayScene,
    interactions: &mut Vec<OverlayInteraction>,
    focused: bool,
) {
    match scene {
        OverlayScene::Text { text } => {
            ui.label(egui::RichText::new(text).monospace().size(13.0));
        }
        OverlayScene::Markdown { text } => {
            // No markdown widget in this egui version — render as wrapped
            // proportional text (the display-only painter does the same).
            ui.label(egui::RichText::new(text).size(13.0));
        }
        OverlayScene::Row { children } => {
            ui.horizontal(|ui| {
                for child in children {
                    render_widget_scene(ui, plugin_id, child, interactions, focused);
                }
            });
        }
        OverlayScene::Button { id, label } => {
            if ui.button(egui::RichText::new(label).size(13.0)).clicked() {
                interactions.push(OverlayInteraction {
                    plugin_id: plugin_id.to_string(),
                    widget_id: id.clone(),
                    event: OverlayWidgetEvent::Click {},
                    focus_request: true,
                });
            }
        }
        OverlayScene::TextInput {
            id,
            value,
            placeholder,
        } => {
            // The scene's value is the plugin's state; edits flow back as
            // text_changed events and the plugin re-pushes the scene.
            let mut buffer = value.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut buffer)
                    .hint_text(placeholder)
                    .desired_width(ui.available_width() - 12.0),
            );
            if focused && response.changed() {
                interactions.push(OverlayInteraction {
                    plugin_id: plugin_id.to_string(),
                    widget_id: id.clone(),
                    event: OverlayWidgetEvent::TextChanged { value: buffer },
                    focus_request: false,
                });
            }
        }
        OverlayScene::List {
            id,
            items,
            selected,
        } => {
            for (index, item) in items.iter().enumerate() {
                let is_selected = *selected == Some(index);
                if ui
                    .selectable_label(
                        is_selected,
                        egui::RichText::new(item).monospace().size(13.0),
                    )
                    .clicked()
                {
                    interactions.push(OverlayInteraction {
                        plugin_id: plugin_id.to_string(),
                        widget_id: id.clone(),
                        event: OverlayWidgetEvent::Select { index },
                        focus_request: true,
                    });
                }
            }
        }
    }
}
