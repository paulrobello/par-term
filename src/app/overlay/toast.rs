//! The toast queue (UX.md OV7, owner decision MD4).
//!
//! Toasts stack top-right, just below the tab bar, at most three at a
//! time (the oldest non-error drops first when a fourth arrives). Each
//! has a kind — info, success, warning, error — with its own accent and
//! icon. Errors persist until dismissed; the others expire. A toast can
//! carry one action button (a registry action id plus its label), and a
//! close button dismisses it.
//!
//! Mode banners (the action-prefix hint, resize mode, demote pick) are not
//! toasts: they describe a mode that is on, not an event that happened, so
//! they draw in their own layer (see `render_mode_banner`) and never take a
//! queue slot.

use super::theme;
use std::time::{Duration, Instant};

/// How many toasts are on screen at once.
pub(crate) const MAX_TOASTS: usize = 3;
/// How long a non-error toast stays up.
pub(crate) const TOAST_LIFETIME: Duration = Duration::from_secs(2);

/// A toast's kind: accent color, icon, and whether it persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Info,
    Success,
    Warning,
    /// Persists until dismissed.
    Error,
}

impl ToastKind {
    fn accent(self) -> egui::Color32 {
        match self {
            ToastKind::Info => theme::ACCENT,
            ToastKind::Success => theme::SUCCESS,
            ToastKind::Warning => theme::WARNING,
            ToastKind::Error => theme::DANGER,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            ToastKind::Info => "ℹ",
            ToastKind::Success => "✓",
            ToastKind::Warning => "⚠",
            ToastKind::Error => "✕",
        }
    }
}

/// An optional action button on a toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToastAction {
    /// Button label ("Undo", "Show").
    pub(crate) label: String,
    /// The registry action the button runs.
    pub(crate) action_id: String,
}

/// One queued toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Toast {
    pub(crate) id: u64,
    pub(crate) kind: ToastKind,
    pub(crate) message: String,
    pub(crate) action: Option<ToastAction>,
    /// `None` = persists until dismissed.
    pub(crate) expires: Option<Instant>,
}

/// What the user did to the toast stack this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToastEvent {
    Dismissed(u64),
    /// The toast's action button: run this registry action.
    Action {
        id: u64,
        action_id: String,
    },
}

/// The per-window toast queue.
#[derive(Debug, Default)]
pub(crate) struct ToastQueue {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl ToastQueue {
    /// Post a toast; returns its id. A repeat of the newest toast's text
    /// and kind refreshes that toast instead of stacking a duplicate.
    pub(crate) fn push(
        &mut self,
        kind: ToastKind,
        message: impl Into<String>,
        action: Option<ToastAction>,
        now: Instant,
    ) -> u64 {
        let message = message.into();
        let expires = (kind != ToastKind::Error).then(|| now + TOAST_LIFETIME);
        if let Some(last) = self.toasts.last_mut()
            && last.kind == kind
            && last.message == message
        {
            last.expires = expires;
            last.action = action;
            return last.id;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.toasts.push(Toast {
            id,
            kind,
            message,
            action,
            expires,
        });
        while self.toasts.len() > MAX_TOASTS {
            // Drop the oldest non-error first; errors are the toasts that
            // must be seen. With only errors left, the oldest error goes.
            let victim = self
                .toasts
                .iter()
                .position(|t| t.kind != ToastKind::Error)
                .unwrap_or(0);
            self.toasts.remove(victim);
        }
        id
    }

    /// Remove expired toasts; returns the earliest pending expiry, for the
    /// event loop's wake timer.
    pub(crate) fn expire(&mut self, now: Instant) -> (bool, Option<Instant>) {
        let before = self.toasts.len();
        self.toasts.retain(|t| t.expires.is_none_or(|e| e > now));
        let next = self.toasts.iter().filter_map(|t| t.expires).min();
        (self.toasts.len() != before, next)
    }

    /// Dismiss one toast.
    pub(crate) fn dismiss(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }

    /// The toasts on screen, oldest first.
    pub(crate) fn toasts(&self) -> &[Toast] {
        &self.toasts
    }

    /// The most recently posted toast still on screen — what tests assert
    /// was announced. A repeat refreshes in place, so the newest is always
    /// the last element.
    #[cfg(test)]
    pub(crate) fn newest(&self) -> Option<&Toast> {
        self.toasts.last()
    }

    /// Drop every toast (tests reset between phases).
    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.toasts.clear();
    }
}

/// Draw the stack top-right, `top_inset` points below the window top (the
/// tab bar's height when it sits at the top), newest on top. Returns what
/// the user clicked.
pub(crate) fn render_toasts(
    ctx: &egui::Context,
    queue: &ToastQueue,
    top_inset: f32,
) -> Vec<ToastEvent> {
    let mut events = Vec::new();
    let mut y = top_inset + 8.0;
    for toast in queue.toasts().iter().rev() {
        let area = egui::Area::new(egui::Id::new(("toast_queue", toast.id)))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(theme::TOAST_FILL)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(1.5, toast.kind.accent()))
                    .show(ui, |ui| {
                        ui.set_max_width(theme::WIDTH_SMALL);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(toast.kind.icon())
                                    .color(toast.kind.accent())
                                    .size(16.0),
                            );
                            ui.label(egui::RichText::new(&toast.message).color(theme::TEXT));
                            if let Some(action) = &toast.action
                                && ui.button(&action.label).clicked()
                            {
                                events.push(ToastEvent::Action {
                                    id: toast.id,
                                    action_id: action.action_id.clone(),
                                });
                            }
                            if ui.small_button("✕").on_hover_text("Dismiss").clicked() {
                                events.push(ToastEvent::Dismissed(toast.id));
                            }
                        });
                    });
            });
        y += area.response.rect.height() + 6.0;
    }
    events
}

/// A mode banner: the one-line status of an armed keyboard mode (action
/// prefix, resize mode, demote pick), top-center in its own layer so it
/// never collides with the toast stack (UX.md OV8).
pub(crate) fn render_mode_banner(ctx: &egui::Context, text: Option<&str>, top_inset: f32) {
    let Some(text) = text else {
        return;
    };
    egui::Area::new(egui::Id::new("mode_banner"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, top_inset + 8.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme::BANNER_FILL)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .corner_radius(8.0)
                .stroke(egui::Stroke::new(1.0, theme::ACCENT))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(text).color(theme::TEXT).size(14.0));
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_most_three_toasts_and_the_oldest_non_error_drops_first() {
        let now = Instant::now();
        let mut q = ToastQueue::default();
        q.push(ToastKind::Error, "e1", None, now);
        q.push(ToastKind::Info, "i1", None, now);
        q.push(ToastKind::Info, "i2", None, now);
        q.push(ToastKind::Success, "s1", None, now);
        let texts: Vec<&str> = q.toasts().iter().map(|t| t.message.as_str()).collect();
        assert_eq!(
            texts,
            ["e1", "i2", "s1"],
            "the error survives a fourth toast"
        );
    }

    #[test]
    fn errors_persist_and_others_expire() {
        let now = Instant::now();
        let mut q = ToastQueue::default();
        q.push(ToastKind::Error, "daemon gone", None, now);
        q.push(ToastKind::Info, "copied", None, now);
        let (changed, next) = q.expire(now + TOAST_LIFETIME + Duration::from_millis(1));
        assert!(changed);
        assert_eq!(next, None, "only a persistent toast is left");
        assert_eq!(q.toasts().len(), 1);
        assert_eq!(q.toasts()[0].kind, ToastKind::Error);
    }

    #[test]
    fn a_new_toast_does_not_overwrite_an_error() {
        // The old one-slot toast wiped a persistent par-mux error with the
        // next info toast (UX.md 23.1).
        let now = Instant::now();
        let mut q = ToastQueue::default();
        q.push(ToastKind::Error, "par-mux: split failed", None, now);
        q.push(ToastKind::Info, "Shader: crt", None, now);
        assert_eq!(q.toasts().len(), 2);
        assert_eq!(q.toasts()[0].message, "par-mux: split failed");
    }

    #[test]
    fn a_repeat_refreshes_instead_of_stacking() {
        let now = Instant::now();
        let mut q = ToastQueue::default();
        let a = q.push(ToastKind::Info, "Output paused", None, now);
        let b = q.push(ToastKind::Info, "Output paused", None, now);
        assert_eq!(a, b);
        assert_eq!(q.toasts().len(), 1);
    }

    #[test]
    fn dismiss_removes_a_persistent_error() {
        let now = Instant::now();
        let mut q = ToastQueue::default();
        let id = q.push(ToastKind::Error, "x", None, now);
        q.push(ToastKind::Info, "y", None, now);
        q.dismiss(id);
        let texts: Vec<&str> = q.toasts().iter().map(|t| t.message.as_str()).collect();
        assert_eq!(texts, ["y"]);
    }

    #[test]
    fn the_stack_draws_top_right_below_the_tab_bar() {
        // MD4: top-right, below the tab bar inset, on screen.
        let now = Instant::now();
        let mut q = ToastQueue::default();
        q.push(
            ToastKind::Info,
            "Tab closed",
            Some(ToastAction {
                label: "Undo".to_string(),
                action_id: "reopen_closed_tab".to_string(),
            }),
            now,
        );
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0));
        let run = |events: Vec<egui::Event>| {
            let mut got = Vec::new();
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| got = render_toasts(ui.ctx(), &q, 30.0),
            );
            out.textures_delta.clear();
            got
        };
        assert!(run(vec![]).is_empty(), "no click, no event");
        let area_rect = ctx
            .memory(|m| m.area_rect(egui::Id::new(("toast_queue", q.toasts()[0].id))))
            .expect("the toast drew");
        assert!(area_rect.max.x <= screen.max.x, "toast stays on screen");
        assert!(
            area_rect.center().x > screen.center().x,
            "toast sits on the right half"
        );
        assert!(
            area_rect.min.y >= 30.0,
            "toast sits below the tab bar inset"
        );
    }
}
