//! Plugin overlay placement and display-only painting (overlay kind).
//!
//! Resolves each live overlay's rect against the screen, then either hands it
//! to [`super::plugin_overlay_ui`] (interactive overlays) or paints its scene
//! as shapes here (display-only overlays; pointer events pass through).

/// Draw the live plugin overlays (overlay plugin kind).
///
/// Anchors and free rects resolve against the current screen rect; sizes are
/// window fractions clamped to half the screen per axis (design content
/// clamps). Drawn at `Order::Middle` — above terminal content, below the
/// modal chrome (`Order::Foreground`) so built-in modal modes (pane-hint
/// select) cover plugin overlays per the mode-stack contract.
///
/// Display-only overlays are painted shapes only (pointer events pass
/// through). Interactive overlays (manifest capability + `interactive: true`)
/// render through [`super::plugin_overlay_ui::render_interactive_overlay`]
/// instead — widgets, click-to-focus, semantic events.
pub(super) fn render_plugin_overlays(
    ctx: &egui::Context,
    overlays: &[(String, par_term_scripting::protocol::PluginOverlay)],
    interactions: &mut Vec<super::plugin_overlay_ui::OverlayInteraction>,
    focused_plugin_id: Option<&str>,
) {
    use par_term_scripting::protocol::{OverlayAnchor, OverlayPosition};

    let screen = ctx.content_rect();
    for (plugin_id, overlay) in overlays {
        let max_w = screen.width() * 0.5;
        let max_h = screen.height() * 0.5;
        let w = (overlay.size.w * screen.width()).clamp(60.0, max_w);
        let h = (overlay.size.h * screen.height()).clamp(24.0, max_h);

        let rect = match overlay.position {
            OverlayPosition::Anchor(anchor) => {
                let (x, y) = match anchor {
                    OverlayAnchor::TopLeft => (screen.left(), screen.top()),
                    OverlayAnchor::Top | OverlayAnchor::TopStrip => {
                        (screen.center().x - w / 2.0, screen.top())
                    }
                    OverlayAnchor::TopRight => (screen.right() - w, screen.top()),
                    OverlayAnchor::Left | OverlayAnchor::LeftStrip => {
                        (screen.left(), screen.center().y - h / 2.0)
                    }
                    OverlayAnchor::Center => {
                        (screen.center().x - w / 2.0, screen.center().y - h / 2.0)
                    }
                    OverlayAnchor::Right | OverlayAnchor::RightStrip => {
                        (screen.right() - w, screen.center().y - h / 2.0)
                    }
                    OverlayAnchor::BottomLeft => (screen.left(), screen.bottom() - h),
                    OverlayAnchor::Bottom | OverlayAnchor::BottomStrip => {
                        (screen.center().x - w / 2.0, screen.bottom() - h)
                    }
                    OverlayAnchor::BottomRight => (screen.right() - w, screen.bottom() - h),
                };
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
            }
            OverlayPosition::Free { x, y } => {
                // Clamp on-screen: the free rect's origin keeps the whole
                // rect inside the window.
                let px = (x * screen.width()).clamp(screen.left(), screen.right() - w);
                let py = (y * screen.height()).clamp(screen.top(), screen.bottom() - h);
                egui::Rect::from_min_size(egui::pos2(px, py), egui::vec2(w, h))
            }
        };

        if overlay.interactive {
            super::plugin_overlay_ui::render_interactive_overlay(
                ctx,
                plugin_id,
                overlay,
                rect,
                interactions,
                focused_plugin_id == Some(plugin_id.as_str()),
            );
            continue;
        }

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("plugin_overlay"),
        ));
        // Frame: translucent dark panel with a subtle border, opacity applied.
        let panel =
            egui::Color32::from_rgba_unmultiplied(20, 20, 28, (220.0 * overlay.opacity) as u8);
        let border =
            egui::Color32::from_rgba_unmultiplied(120, 140, 190, (200.0 * overlay.opacity) as u8);
        painter.rect_filled(rect, 6.0, panel);
        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.0, border),
            egui::StrokeKind::Middle,
        );

        // Inset the scene and lay it out top-down.
        let inner = rect.shrink(8.0);
        let mut layout_y = inner.top();
        render_scene(
            &painter,
            inner,
            &overlay.content,
            inner.left(),
            &mut layout_y,
            overlay.opacity,
        );
    }
}

/// Render one scene node into the overlay's inner rect, top-down and
/// left-to-right. Display-only vocabulary: text, row, markdown — the
/// interactive variants are unreachable here (they route through
/// [`super::plugin_overlay_ui`] before this function is called) and render
/// as plain text so a misrouted scene degrades instead of vanishing.
fn render_scene(
    painter: &egui::Painter,
    inner: egui::Rect,
    scene: &par_term_scripting::protocol::OverlayScene,
    cursor_x: f32,
    layout_y: &mut f32,
    opacity: f32,
) {
    use par_term_scripting::protocol::OverlayScene;
    match scene {
        OverlayScene::Text { text } => {
            let font = egui::FontId::monospace(13.0);
            let color =
                egui::Color32::from_rgba_unmultiplied(220, 220, 230, (255.0 * opacity) as u8);
            let galley = painter.layout(text.clone(), font, color, inner.width());
            painter.galley(egui::pos2(cursor_x, *layout_y), galley, color);
            *layout_y += 18.0;
        }
        OverlayScene::Row { children } => {
            let mut x = cursor_x;
            for child in children {
                render_scene(painter, inner, child, x, layout_y, opacity);
                x += 60.0;
                if x > inner.right() - 40.0 {
                    x = cursor_x;
                }
            }
        }
        OverlayScene::Markdown { text } => {
            let font = egui::FontId::proportional(13.0);
            let color =
                egui::Color32::from_rgba_unmultiplied(230, 230, 235, (255.0 * opacity) as u8);
            let galley = painter.layout(text.clone(), font, color, inner.width());
            let height = galley.size().y;
            painter.galley(egui::pos2(inner.left(), *layout_y), galley, color);
            *layout_y += height + 4.0;
        }
        OverlayScene::Button { label, .. } => {
            let font = egui::FontId::monospace(13.0);
            let color =
                egui::Color32::from_rgba_unmultiplied(200, 200, 210, (255.0 * opacity) as u8);
            let galley = painter.layout(format!("[{label}]"), font, color, inner.width());
            painter.galley(egui::pos2(cursor_x, *layout_y), galley, color);
            *layout_y += 20.0;
        }
        OverlayScene::TextInput { placeholder, .. } => {
            let font = egui::FontId::monospace(13.0);
            let color =
                egui::Color32::from_rgba_unmultiplied(200, 200, 210, (255.0 * opacity) as u8);
            let galley = painter.layout(placeholder.clone(), font, color, inner.width());
            painter.galley(egui::pos2(cursor_x, *layout_y), galley, color);
            *layout_y += 20.0;
        }
        OverlayScene::List { items, .. } => {
            let color =
                egui::Color32::from_rgba_unmultiplied(220, 220, 230, (255.0 * opacity) as u8);
            for item in items {
                let galley = painter.layout(
                    item.clone(),
                    egui::FontId::monospace(13.0),
                    color,
                    inner.width(),
                );
                painter.galley(egui::pos2(cursor_x, *layout_y), galley, color);
                *layout_y += 18.0;
            }
        }
    }
}
