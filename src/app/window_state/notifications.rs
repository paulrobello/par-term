//! Notification and alert handling for the terminal.
//!
//! This module handles:
//! - Desktop notifications (OSC 9/777)
//! - OSC 99 (Kitty) notification metadata: id-based grouping/replacement and
//!   click-to-activate `focus`/`report` actions
//! - Bell events (audio, visual, desktop)
//!
//! ## OSC 99 click-to-activate
//!
//! Per the Kitty desktop notifications spec
//! (<https://sw.kovidgoyal.net/kitty/desktop-notifications/>), the `a=` actions
//! key accepts a comma-separated list of `report`/`focus`, each optionally
//! negated with a leading `-`. When `a=` is never sent, the default active
//! action is `focus` alone; `a=-focus` opts out of it, and `report` is never
//! implied — it must be requested explicitly. Activation (when `report` is
//! active) is reported back to the application as `OSC 99 ; i=<id> ; ST`,
//! using `i=0` when the original notification had no id.
//!
//! Each such notification registers a [`PendingNotificationClick`] under a
//! fresh `click_token` (see [`crate::platform::notify`]) in this window's
//! [`NotificationClickState`]. `check_notification_clicks` (called from
//! `about_to_wait`) resolves clicked tokens and performs the focus/report
//! actions. See [`NotificationClickState`] docs for why the registry is
//! per-window rather than a single process-global map.

use super::WindowState;
use crate::pane::PaneId;
use crate::tab::TabId;
use crate::terminal::TerminalManager;
use par_term_emu_core_rust::terminal::Urgency;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Instant;
use tokio::sync::RwLock;

/// Maximum number of pending click registrations retained per window. Oldest
/// entries are evicted once exceeded so that notifications nobody clicks
/// don't leak memory indefinitely.
const MAX_PENDING_NOTIFICATION_CLICKS: usize = 256;

/// Maximum number of times a drained click token may be re-queued onto
/// [`CLICK_TOKEN_REQUEUE`] before being dropped, bounding memory if the
/// owning window closed before the notification was clicked.
const MAX_CLICK_TOKEN_REQUEUES: u32 = 600;

/// A pending click-to-action registration for an OSC 99 notification
/// delivered with a `click_token`.
pub(crate) struct PendingNotificationClick {
    tab_id: TabId,
    pane_id: Option<PaneId>,
    terminal: Weak<RwLock<TerminalManager>>,
    osc_id: Option<String>,
    wants_focus: bool,
    wants_report: bool,
    registered_at: Instant,
}

/// Per-window registry of pending OSC 99 notification-click actions.
///
/// The click channel in [`crate::platform::notify`] is a single process-global
/// channel shared by every window. `TabId`/`PaneId` values, however, are
/// per-window counters that can collide across windows, so a window may only
/// act on tokens *it* registered — acting on a foreign token could focus or
/// write to the wrong tab/pane. `check_notification_clicks` therefore
/// re-queues tokens it doesn't recognize onto [`CLICK_TOKEN_REQUEUE`] so
/// another window gets a chance to claim them, typically within the same
/// event-loop tick since `WindowManager::about_to_wait` polls all windows
/// sequentially.
#[derive(Default)]
pub(crate) struct NotificationClickState {
    pending: Vec<(u64, PendingNotificationClick)>,
}

/// Monotonically increasing counter for notification click tokens. Shared by
/// every window — tokens must be globally unique so the re-queue mechanism
/// above can match a drained token to at most one registry entry.
static NEXT_CLICK_TOKEN: AtomicU64 = AtomicU64::new(1);

fn next_click_token() -> u64 {
    NEXT_CLICK_TOKEN.fetch_add(1, Ordering::Relaxed)
}

/// Cross-window re-route buffer: tokens drained by a window that doesn't own
/// them are pushed here (with a retry count) so another window polled later
/// this tick, or on a subsequent tick, can claim them. See
/// [`NotificationClickState`] docs.
static CLICK_TOKEN_REQUEUE: OnceLock<Mutex<Vec<(u64, u32)>>> = OnceLock::new();

fn click_token_requeue() -> &'static Mutex<Vec<(u64, u32)>> {
    CLICK_TOKEN_REQUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// A notification collected from a tab/pane's terminal, still carrying its
/// origin so `deliver_osc99_notification` can register click actions.
struct CollectedNotification {
    title: String,
    message: String,
    urgency: Urgency,
    tab_id: TabId,
    pane_id: Option<PaneId>,
    terminal: Arc<RwLock<TerminalManager>>,
    osc_id: Option<String>,
    actions: Vec<String>,
}

impl WindowState {
    /// Check for OSC 9/777/99 notifications across every tab and pane's terminal.
    ///
    /// Polls all tabs (not just the active one) and, for split-pane tabs, every pane
    /// (not just the focused one) — falling back to `tab.terminal` when there is no
    /// pane manager — so notifications emitted in a background tab/pane are delivered
    /// promptly instead of sitting queued until the user focuses it.
    pub(crate) fn check_notifications(&mut self) {
        // Collect notifications from all tabs/panes first, deliver after releasing
        // the terminal locks (matches the borrow-safety pattern used elsewhere in
        // this file, e.g. `check_session_exit_notifications`).
        let mut notifications_to_send: Vec<CollectedNotification> = Vec::new();

        for tab in self.tab_manager.tabs() {
            // Every pane's terminal (falls back to tab.terminal when there's no pane manager),
            // paired with the pane id so a click can re-focus the exact origin pane.
            let terminals: Vec<(Option<PaneId>, Arc<RwLock<TerminalManager>>)> = tab
                .pane_manager
                .as_ref()
                .map(|pm| {
                    pm.all_panes()
                        .into_iter()
                        .map(|pane| (Some(pane.id), Arc::clone(&pane.terminal)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![(None, Arc::clone(&tab.terminal))]);

            for (pane_id, terminal) in terminals {
                // try_lock: intentional — OSC notification polling in about_to_wait (sync loop).
                // On miss: notifications are deferred to the next poll frame. Low risk; OSC
                // notifications are informational and a one-frame delay is imperceptible.
                // A read lock suffices here: `has_notifications`/`take_notifications` only
                // require `&self` on `TerminalManager` (they lock the PTY/terminal internally).
                if let Ok(term) = terminal.try_read()
                    && term.has_notifications()
                {
                    for notif in term.take_notifications() {
                        notifications_to_send.push(CollectedNotification {
                            title: notif.title,
                            message: notif.message,
                            urgency: notif.urgency,
                            tab_id: tab.id,
                            pane_id,
                            terminal: Arc::clone(&terminal),
                            osc_id: notif.id,
                            actions: notif.actions,
                        });
                    }
                }
            }
        }

        for n in notifications_to_send {
            self.deliver_osc99_notification(
                n.tab_id, n.pane_id, n.terminal, &n.title, &n.message, n.urgency, n.osc_id,
                n.actions,
            );
        }
    }

    /// Deliver an OSC 9/777/99 notification, honoring focus suppression and
    /// (for OSC 99) wiring up identity-based replacement and click-to-activate
    /// `focus`/`report` actions per the Kitty spec (see module docs).
    #[allow(clippy::too_many_arguments)]
    fn deliver_osc99_notification(
        &mut self,
        tab_id: TabId,
        pane_id: Option<PaneId>,
        terminal: Arc<RwLock<TerminalManager>>,
        title: &str,
        message: &str,
        urgency: Urgency,
        osc_id: Option<String>,
        actions: Vec<String>,
    ) {
        // Always log notifications (mirrors `deliver_notification_inner`).
        if !title.is_empty() {
            log::info!("=== Notification: {} ===", title);
            log::info!("{}", message);
            log::info!("===========================");
        } else {
            log::info!("=== Notification ===");
            log::info!("{}", message);
            log::info!("===================");
        }

        // Skip desktop notification if window is focused and suppression is enabled
        // (OSC 99 notifications respect suppression, same as `deliver_notification_inner`).
        if self
            .config
            .load()
            .notifications
            .suppress_notifications_when_focused
            && self.focus_state.is_focused
        {
            log::debug!(
                "Suppressing desktop notification (window is focused): {}",
                title
            );
            return;
        }

        // Kitty spec: `a=` defaults to `focus` active when never sent by the
        // application; `-focus` opts out of that default; `report` is never
        // implied and must be requested explicitly (and `-report` defensively
        // cancels it back out).
        let mut wants_focus = true;
        let mut wants_report = false;
        for action in &actions {
            match action.as_str() {
                "focus" => wants_focus = true,
                "-focus" => wants_focus = false,
                "report" => wants_report = true,
                "-report" => wants_report = false,
                _ => {}
            }
        }

        // Prefix the OSC 99 id with the tab id so ids from different terminals
        // can't collide, while staying stable across redeliveries with the
        // same terminal + id (required for platform-side replacement).
        let identity = osc_id.as_ref().map(|id| format!("osc99-{tab_id}-{id}"));

        let click_token = if wants_focus || wants_report {
            let token = next_click_token();
            self.register_notification_click(
                token,
                PendingNotificationClick {
                    tab_id,
                    pane_id,
                    terminal: Arc::downgrade(&terminal),
                    osc_id,
                    wants_focus,
                    wants_report,
                    registered_at: Instant::now(),
                },
            );
            Some(token)
        } else {
            None
        };

        let platform_urgency = match urgency {
            Urgency::Low => crate::platform::NotificationUrgency::Low,
            Urgency::Normal => crate::platform::NotificationUrgency::Normal,
            Urgency::Critical => crate::platform::NotificationUrgency::Critical,
        };
        crate::platform::deliver_desktop_notification_request(
            &crate::platform::NotificationRequest {
                title,
                message,
                timeout_ms: 3000,
                urgency: platform_urgency,
                identity: identity.as_deref(),
                click_token,
            },
        );
    }

    /// Register a pending click action, evicting the oldest entry once the
    /// per-window cap is exceeded (see [`MAX_PENDING_NOTIFICATION_CLICKS`]).
    fn register_notification_click(&mut self, token: u64, entry: PendingNotificationClick) {
        let pending = &mut self.notification_click_state.pending;
        pending.push((token, entry));
        if pending.len() > MAX_PENDING_NOTIFICATION_CLICKS {
            pending.remove(0);
        }
    }

    /// Drain clicked notification tokens (own and re-queued) and perform the
    /// registered focus/report actions. Called once per frame from
    /// `about_to_wait`, alongside `check_notifications`.
    pub(crate) fn check_notification_clicks(&mut self) {
        let fresh = crate::platform::drain_notification_clicks()
            .into_iter()
            .map(|token| (token, 0u32));
        let requeued = std::mem::take(
            &mut *click_token_requeue()
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );

        for (token, retries) in fresh.chain(requeued) {
            if let Some(pos) = self
                .notification_click_state
                .pending
                .iter()
                .position(|(t, _)| *t == token)
            {
                let (_, entry) = self.notification_click_state.pending.remove(pos);
                self.execute_notification_click(entry);
                continue;
            }

            // Not registered by this window — offer it back for another window
            // (or a later tick) to claim, bounded so an unclaimed token from a
            // closed window doesn't grow the buffer forever.
            if retries + 1 < MAX_CLICK_TOKEN_REQUEUES {
                click_token_requeue()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((token, retries + 1));
            } else {
                log::debug!(
                    "Dropping notification click token {} after {} unclaimed re-queues",
                    token,
                    retries + 1
                );
            }
        }
    }

    /// Perform the focus/report actions for a resolved notification click.
    fn execute_notification_click(&mut self, entry: PendingNotificationClick) {
        log::debug!(
            "Handling notification click for tab {} (age: {:?}, focus={}, report={})",
            entry.tab_id,
            entry.registered_at.elapsed(),
            entry.wants_focus,
            entry.wants_report
        );

        if entry.wants_focus {
            if let Some(window) = &self.window {
                window.focus_window();
            }
            self.tab_manager.switch_to(entry.tab_id);
            if let Some(pane_id) = entry.pane_id
                && let Some(tab) = self.tab_manager.get_tab_mut(entry.tab_id)
                && let Some(pm) = tab.pane_manager.as_mut()
            {
                pm.focus_pane(pane_id);
            }
        }

        if entry.wants_report {
            // Kitty spec: activation is reported as `OSC 99 ; i=<id> ; ST`,
            // using `i=0` when the original notification had no id.
            let report = format!(
                "\x1b]99;i={};\x1b\\",
                entry.osc_id.as_deref().unwrap_or("0")
            );
            match entry.terminal.upgrade() {
                Some(terminal) => match terminal.try_read() {
                    Ok(term) => {
                        if let Err(e) = term.write_str(&report) {
                            log::debug!("Failed to write OSC 99 activation report: {}", e);
                        }
                    }
                    Err(_) => {
                        log::debug!("Skipped OSC 99 activation report: terminal lock contended")
                    }
                },
                None => log::debug!("Skipped OSC 99 activation report: terminal no longer exists"),
            }
        }

        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Check for bell events and trigger appropriate feedback.
    pub(crate) fn check_bell(&mut self) {
        // Skip if all bell notifications are disabled
        if self.config.load().notifications.notification_bell_sound == 0
            && !self.config.load().notifications.notification_bell_visual
            && !self.config.load().notifications.notification_bell_desktop
        {
            return;
        }

        // Get current bell count from focused pane's terminal (not tab.terminal,
        // which may differ from the focused pane's terminal after a split).
        let (current_bell_count, last_count) = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };

            // Get the focused pane's terminal (falls back to tab terminal if no pane manager)
            let terminal = tab
                .pane_manager
                .as_ref()
                .and_then(|pm| pm.focused_pane())
                .map(|pane| std::sync::Arc::clone(&pane.terminal))
                .unwrap_or_else(|| std::sync::Arc::clone(&tab.terminal));

            // try_lock: intentional — bell count polling in about_to_wait (sync event loop).
            // On miss: bell detection is skipped this frame. The bell event will be seen
            // on the next poll. A one-frame delay in bell feedback is imperceptible.
            if let Ok(term) = terminal.try_write() {
                (term.bell_count(), tab.active_bell().last_count)
            } else {
                return;
            }
        };

        if current_bell_count > last_count {
            // Bell event(s) occurred
            let bell_events = current_bell_count - last_count;
            log::info!("Bell event detected ({} bell(s))", bell_events);
            log::info!(
                "  Config: sound={}, visual={}, desktop={}",
                self.config.load().notifications.notification_bell_sound,
                self.config.load().notifications.notification_bell_visual,
                self.config.load().notifications.notification_bell_desktop
            );

            // Play audio bell if enabled (volume > 0)
            // Check alert_sounds config first, fall back to legacy bell_sound setting
            if let Some(alert_cfg) = self
                .config
                .load()
                .notifications
                .alert_sounds
                .get(&crate::config::AlertEvent::Bell)
            {
                if alert_cfg.enabled
                    && alert_cfg.volume > 0
                    && let Some(tab) = self.tab_manager.active_tab()
                    && let Some(ref audio_bell) = tab.active_bell().audio
                {
                    log::info!(
                        "  Playing alert sound for bell at {}% volume",
                        alert_cfg.volume
                    );
                    audio_bell.play_alert(alert_cfg);
                }
            } else if self.config.load().notifications.notification_bell_sound > 0 {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Some(ref audio_bell) = tab.active_bell().audio
                {
                    log::info!(
                        "  Playing audio bell at {}% volume",
                        self.config.load().notifications.notification_bell_sound
                    );
                    audio_bell.play(self.config.load().notifications.notification_bell_sound);
                } else {
                    log::warn!("  Audio bell requested but not initialized");
                }
            } else {
                log::debug!("  Audio bell disabled (volume=0)");
            }

            // Trigger visual bell flash if enabled
            if self.config.load().notifications.notification_bell_visual {
                log::info!("  Triggering visual bell flash");
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.active_bell_mut().visual_flash = Some(std::time::Instant::now());
                }
                // Request immediate redraw to show flash
                self.request_redraw();
            } else {
                log::debug!("  Visual bell disabled");
            }

            // Send desktop notification if enabled
            if self.config.load().notifications.notification_bell_desktop {
                log::info!("  Sending desktop notification");
                let message = if bell_events == 1 {
                    "Terminal bell".to_string()
                } else {
                    format!("Terminal bell ({} events)", bell_events)
                };
                self.deliver_notification("Terminal", &message);
            } else {
                log::debug!("  Desktop notification disabled");
            }

            // Update last count
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_bell_mut().last_count = current_bell_count;
            }
        }
    }

    /// Play an alert sound for the given event, if configured.
    pub(crate) fn play_alert_sound(&self, event: crate::config::AlertEvent) {
        if let Some(alert_cfg) = self.config.load().notifications.alert_sounds.get(&event)
            && alert_cfg.enabled
            && alert_cfg.volume > 0
            && let Some(tab) = self.tab_manager.active_tab()
            && let Some(ref audio_bell) = tab.active_bell().audio
        {
            log::info!(
                "Playing alert sound for {:?} at {}% volume",
                event,
                alert_cfg.volume
            );
            audio_bell.play_alert(alert_cfg);
        }
    }

    /// Check for session exit notifications across all tabs.
    ///
    /// Notifies the user when a shell/process exits, useful for long-running commands
    /// where the user may have switched to other applications.
    pub(crate) fn check_session_exit_notifications(&mut self) {
        if !self.config.load().notifications.notification_session_ended {
            return;
        }

        let mut notifications_to_send: Vec<(String, String)> = Vec::new();

        for tab in self.tab_manager.tabs_mut() {
            // Skip if already notified for this tab
            if tab.activity.exit_notified {
                continue;
            }

            // Check if the terminal has exited
            // try_lock: intentional — exit check in about_to_wait (sync event loop).
            // On miss: this tab's exit is not detected this frame; it will be on the next.
            let has_exited = if let Ok(term) = tab.terminal.try_write() {
                !term.is_running()
            } else {
                continue; // Skip if terminal is locked
            };

            if has_exited {
                tab.activity.exit_notified = true;
                let title = format!("Session Ended: {}", tab.title);
                let message = "The shell process has exited".to_string();
                log::info!("Session exit notification: {} has exited", tab.title);
                notifications_to_send.push((title, message));
            }
        }

        // Send collected notifications (after releasing mutable borrow)
        for (title, message) in notifications_to_send {
            self.deliver_notification(&title, &message);
        }
    }

    /// Check for activity/idle notifications across all tabs.
    ///
    /// This method handles two types of notifications:
    /// - **Activity notification**: Triggered when terminal output resumes after a period of
    ///   inactivity (useful for long-running commands completing).
    /// - **Silence notification**: Triggered when a terminal has been idle for longer than the
    ///   configured threshold (useful for detecting stalled processes).
    pub(crate) fn check_activity_idle_notifications(&mut self) {
        // Skip if both notification types are disabled
        if !self
            .config
            .load()
            .notifications
            .notification_activity_enabled
            && !self
                .config
                .load()
                .notifications
                .notification_silence_enabled
        {
            return;
        }

        let now = std::time::Instant::now();
        let activity_threshold = std::time::Duration::from_secs(
            self.config
                .load()
                .notifications
                .notification_activity_threshold,
        );
        let silence_threshold = std::time::Duration::from_secs(
            self.config
                .load()
                .notifications
                .notification_silence_threshold,
        );

        // Collect notification data for all tabs to avoid borrow conflicts
        let mut notifications_to_send: Vec<(String, String)> = Vec::new();

        for tab in self.tab_manager.tabs_mut() {
            // Get current terminal generation to detect new output
            // try_lock: intentional — activity/generation check in about_to_wait (sync loop).
            // On miss: activity tracking skipped for this tab this frame. Harmless.
            let current_generation = if let Ok(term) = tab.terminal.try_write() {
                term.update_generation()
            } else {
                continue; // Skip if terminal is locked
            };

            let time_since_activity = now.duration_since(tab.activity.last_activity_time);

            // Check if there's new terminal output
            if current_generation > tab.activity.last_seen_generation {
                // New output detected - this is "activity"
                let was_idle = time_since_activity >= activity_threshold;

                // Update tracking state
                tab.activity.last_seen_generation = current_generation;
                tab.activity.last_activity_time = now;
                tab.activity.silence_notified = false; // Reset silence notification flag

                // Activity notification: notify if we were idle long enough
                if self
                    .config
                    .load()
                    .notifications
                    .notification_activity_enabled
                    && was_idle
                {
                    let title = format!("Activity in {}", tab.title);
                    let message = format!(
                        "Terminal output resumed after {} seconds of inactivity",
                        time_since_activity.as_secs()
                    );
                    log::info!(
                        "Activity notification: {} idle for {}s, now active",
                        tab.title,
                        time_since_activity.as_secs()
                    );
                    notifications_to_send.push((title, message));
                }
            } else {
                // No new output - check for silence notification
                if self
                    .config
                    .load()
                    .notifications
                    .notification_silence_enabled
                    && !tab.activity.silence_notified
                    && time_since_activity >= silence_threshold
                {
                    // Terminal has been silent for longer than threshold
                    tab.activity.silence_notified = true;
                    let title = format!("Silence in {}", tab.title);
                    let message =
                        format!("No output for {} seconds", time_since_activity.as_secs());
                    log::info!(
                        "Silence notification: {} silent for {}s",
                        tab.title,
                        time_since_activity.as_secs()
                    );
                    notifications_to_send.push((title, message));
                }
            }
        }

        // Send collected notifications (after releasing mutable borrow)
        for (title, message) in notifications_to_send {
            self.deliver_notification(&title, &message);
        }
    }

    /// Deliver a notification unconditionally (bypasses focus suppression).
    ///
    /// Used for trigger-generated notifications which the user explicitly configured,
    /// so they should always be delivered regardless of window focus state.
    pub(crate) fn deliver_notification_force(&self, title: &str, message: &str) {
        self.deliver_notification_inner(title, message, true, Urgency::Normal);
    }

    /// Deliver a notification via desktop notification system and logs.
    ///
    /// If `suppress_notifications_when_focused` is enabled and the window is focused,
    /// only log the notification without sending a desktop notification (since the user
    /// is already looking at the terminal).
    pub(crate) fn deliver_notification(&self, title: &str, message: &str) {
        self.deliver_notification_inner(title, message, false, Urgency::Normal);
    }

    /// Inner notification delivery with force option.
    ///
    /// When `force` is true, bypasses focus suppression (used for trigger notifications).
    fn deliver_notification_inner(
        &self,
        title: &str,
        message: &str,
        force: bool,
        urgency: Urgency,
    ) {
        // Always log notifications
        if !title.is_empty() {
            log::info!("=== Notification: {} ===", title);
            log::info!("{}", message);
            log::info!("===========================");
        } else {
            log::info!("=== Notification ===");
            log::info!("{}", message);
            log::info!("===================");
        }

        // Skip desktop notification if window is focused and suppression is enabled
        // (unless force is set, e.g. for trigger-generated notifications)
        if !force
            && self
                .config
                .load()
                .notifications
                .suppress_notifications_when_focused
            && self.focus_state.is_focused
        {
            log::debug!(
                "Suppressing desktop notification (window is focused): {}",
                title
            );
            return;
        }

        // Send desktop notification via the platform abstraction layer
        let platform_urgency = match urgency {
            Urgency::Low => crate::platform::NotificationUrgency::Low,
            Urgency::Normal => crate::platform::NotificationUrgency::Normal,
            Urgency::Critical => crate::platform::NotificationUrgency::Critical,
        };
        crate::platform::deliver_desktop_notification(title, message, 3000, platform_urgency);
    }
}
