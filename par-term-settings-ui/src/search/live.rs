//! Search state the section helpers read while drawing (UX.md SQ4).
//!
//! `collapsing_section` has only a `Ui` and the collapse set, so the Settings
//! window publishes the current search to egui memory once per frame and
//! the helper reads it back: which sections to hide, which to force open,
//! and a pending jump (expand, scroll to, and flash a result).

use std::collections::HashSet;
use std::sync::Arc;

use super::harvest;

/// How long a jumped-to control stays highlighted, in seconds.
const FLASH_SECS: f64 = 1.5;

/// Frames a jump may wait for its target to settle before it settles for
/// the section header.
const JUMP_FRAMES: u32 = 90;

/// The current search, as the section helpers see it.
#[derive(Debug, Clone, Default)]
pub(crate) struct LiveView {
    /// A search is active.
    pub active: bool,
    /// The query changed this frame, so matching sections re-expand.
    pub query_changed: bool,
    /// Sections the registry knows; only these are ever hidden.
    pub known: HashSet<String>,
    /// Known sections with a match, and the sections enclosing them.
    pub visible: HashSet<String>,
}

impl LiveView {
    /// Whether a search is active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Whether the search hides section `id`.
    pub fn hides(&self, id: &str) -> bool {
        self.active && self.known.contains(id) && !self.visible.contains(id)
    }
}

/// A jump to a result: expand `path`, scroll to the target, flash it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Jump {
    /// Section ids to open, target section first, outermost last.
    pub path: Vec<String>,
    /// Caption of the target control; `None` targets the section header.
    pub label: Option<String>,
    frames: u32,
    last_rect: Option<egui::Rect>,
}

impl Jump {
    pub fn new(path: Vec<String>, label: Option<String>) -> Self {
        Self {
            path,
            label,
            frames: 0,
            last_rect: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Flash {
    section: String,
    on_header: bool,
    offset: egui::Vec2,
    size: egui::Vec2,
    started: f64,
}

/// What a section helper reports after drawing a section.
pub(crate) struct DrawnSection<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub keywords: &'a [&'a str],
    pub header: egui::Response,
    /// Body `Ui` id and rect; `None` while collapsed.
    pub body: Option<(egui::Id, egui::Rect)>,
    pub fully_open: bool,
}

fn view_id() -> egui::Id {
    egui::Id::new("settings_search_live_view")
}

fn jump_id() -> egui::Id {
    view_id().with("jump")
}

fn flash_id() -> egui::Id {
    view_id().with("flash")
}

/// Publish the search for this frame's section helpers.
pub(crate) fn set_live_view(ctx: &egui::Context, view: Arc<LiveView>) {
    ctx.data_mut(|d| d.insert_temp(view_id(), view));
}

/// The search published for this frame, if any.
pub(crate) fn live_view(ctx: &egui::Context) -> Option<Arc<LiveView>> {
    ctx.data(|d| d.get_temp::<Arc<LiveView>>(view_id()))
}

/// Start a jump. A control target needs AccessKit captions for one or two
/// frames to find the widget; it is switched off again when the jump ends.
pub(crate) fn start_jump(ctx: &egui::Context, jump: Jump) {
    if jump.label.is_some() {
        ctx.enable_accesskit();
    }
    ctx.data_mut(|d| {
        d.insert_temp(jump_id(), Some(jump));
        d.insert_temp::<Option<Flash>>(flash_id(), None);
    });
    ctx.request_repaint();
}

/// The pending jump, if any.
pub(crate) fn pending_jump(ctx: &egui::Context) -> Option<Jump> {
    ctx.data(|d| d.get_temp::<Option<Jump>>(jump_id()))
        .flatten()
}

/// Whether the section helper must open section `id` this frame.
pub(crate) fn forces_open(ctx: &egui::Context, view: Option<&LiveView>, id: &str) -> bool {
    view.is_some_and(|v| v.query_changed)
        || pending_jump(ctx).is_some_and(|j| j.path.iter().any(|p| p == id))
}

/// Bookkeeping after a section is drawn: register it during the harvest,
/// advance a jump that targets it, and paint its flash.
pub(crate) fn note_section(ui: &egui::Ui, drawn: DrawnSection<'_>) {
    let ctx = ui.ctx();
    if harvest::harvesting(ctx) {
        if let Some((body, _)) = drawn.body {
            harvest::record_section(
                ctx,
                drawn.id,
                drawn.title,
                drawn.keywords,
                drawn.header.id,
                body,
            );
        }
        return;
    }
    if let Some(jump) = pending_jump(ctx)
        && jump.path.first().is_some_and(|p| p == drawn.id)
    {
        advance_jump(ui, jump, &drawn);
    }
    paint_flash(ui, &drawn);
}

fn advance_jump(ui: &egui::Ui, mut jump: Jump, drawn: &DrawnSection<'_>) {
    let ctx = ui.ctx();
    jump.frames += 1;
    let header = drawn.header.rect;
    let target = match (&jump.label, drawn.body) {
        (None, _) => Some((header, true)),
        (Some(label), Some((_, body))) if drawn.fully_open => {
            find_caption(ctx, body, label).map(|r| (r, false))
        }
        _ => None,
    };
    let target = target.or_else(|| (jump.frames > JUMP_FRAMES).then_some((header, true)));
    let Some((rect, on_header)) = target else {
        store_jump(ctx, Some(jump));
        ctx.request_repaint();
        return;
    };
    // Settle first: an enclosing section may still be animating open, which
    // moves everything below it.
    let settled = jump
        .last_rect
        .is_some_and(|last| (last.min - rect.min).length() < 0.5)
        || jump.frames > JUMP_FRAMES;
    if !settled {
        jump.last_rect = Some(rect);
        store_jump(ctx, Some(jump));
        ctx.request_repaint();
        return;
    }
    ui.scroll_to_rect(rect, Some(egui::Align::Center));
    let anchor = if on_header {
        header.min
    } else {
        drawn.body.map_or(header.min, |(_, b)| b.min)
    };
    let flash = Flash {
        section: drawn.id.to_string(),
        on_header,
        offset: rect.min - anchor,
        size: rect.size(),
        started: ctx.input(|i| i.time),
    };
    ctx.data_mut(|d| d.insert_temp(flash_id(), Some(flash)));
    store_jump(ctx, None);
    ctx.disable_accesskit();
    ctx.request_repaint();
}

fn store_jump(ctx: &egui::Context, jump: Option<Jump>) {
    ctx.data_mut(|d| d.insert_temp(jump_id(), jump));
}

/// The rect of the widget captioned `label` inside `within`, from this
/// pass's AccessKit nodes.
fn find_caption(ctx: &egui::Context, within: egui::Rect, label: &str) -> Option<egui::Rect> {
    ctx.viewport(|vp| {
        let state = vp.this_pass.accesskit_state.as_ref()?;
        state
            .nodes
            .values()
            .filter(|node| harvest::caption(node).as_deref() == Some(label))
            .filter_map(|node| node.bounds())
            .map(|b| {
                egui::Rect::from_min_max(
                    egui::pos2(b.x0 as f32, b.y0 as f32),
                    egui::pos2(b.x1 as f32, b.y1 as f32),
                )
            })
            .filter(|r| within.intersects(*r))
            .min_by(|a, b| a.min.y.total_cmp(&b.min.y))
    })
}

fn paint_flash(ui: &egui::Ui, drawn: &DrawnSection<'_>) {
    let ctx = ui.ctx();
    let Some(flash) = ctx
        .data(|d| d.get_temp::<Option<Flash>>(flash_id()))
        .flatten()
    else {
        return;
    };
    if flash.section != drawn.id {
        return;
    }
    let age = ctx.input(|i| i.time) - flash.started;
    if age > FLASH_SECS {
        ctx.data_mut(|d| d.insert_temp::<Option<Flash>>(flash_id(), None));
        return;
    }
    let anchor = if flash.on_header {
        drawn.header.rect.min
    } else {
        match drawn.body {
            Some((_, body)) => body.min,
            None => return,
        }
    };
    let fade = (1.0 - age / FLASH_SECS) as f32;
    let rect = egui::Rect::from_min_size(anchor + flash.offset, flash.size).expand(3.0);
    let color = egui::Color32::from_rgb(255, 193, 7).gamma_multiply(fade);
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(2.0, color),
        egui::StrokeKind::Outside,
    );
    ctx.request_repaint();
}

/// Whether a flash is still showing (tests read it).
#[cfg(test)]
pub(crate) fn flashing(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<Option<Flash>>(flash_id()))
        .flatten()
        .map(|f| f.section)
}
