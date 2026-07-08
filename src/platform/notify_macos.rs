//! Native macOS desktop notification backend (`UNUserNotificationCenter`),
//! with an `osascript` fallback for processes that aren't a signed/bundled app.
//!
//! ## Behavior matrix
//!
//! | Condition | Backend | Identity | Click |
//! |---|---|---|---|
//! | Bundled, signed app (`NSBundle.mainBundle.bundleIdentifier` is `Some`) | `UNUserNotificationCenter` | Native same-id replacement (`UNNotificationRequest` identifier) | Native, via `UNUserNotificationCenterDelegate` |
//! | Bare `cargo run` binary (no bundle identifier) | `osascript` | Ignored | Ignored |
//! | Native path errors at runtime (auth request or scheduling failure) | `osascript` (for all subsequent calls) | Ignored | Ignored |
//!
//! `UNUserNotificationCenter` only functions from a signed/bundled app; a bare
//! `cargo run` binary has no bundle identifier and the native API is a no-op
//! (or errors) there, so availability is probed once via [`NSBundle`] and cached.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, Once, OnceLock};

use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, extern_methods};
use objc2_foundation::{NSBundle, NSError, NSObject, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationDismissActionIdentifier, UNNotificationPresentationOptions,
    UNNotificationRequest, UNNotificationResponse, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};

use super::notify::{NotificationRequest, NotificationUrgency, escape_for_applescript};

/// Cached decision: does this process have a bundle identifier (i.e. is it
/// running as a signed/bundled `.app`, as opposed to a bare `cargo run`
/// binary)? `UNUserNotificationCenter` only works for bundled apps.
static NATIVE_BUNDLE_PRESENT: OnceLock<bool> = OnceLock::new();

/// Set once the native path has errored at runtime (authorization request or
/// notification scheduling failed) — subsequent calls fall back to
/// `osascript` for the rest of the process lifetime.
static NATIVE_DISABLED: AtomicBool = AtomicBool::new(false);

/// Ensures the "native backend failed, falling back" warning is logged only once.
static WARN_ONCE: Once = Once::new();

/// Ensures the delegate is installed on the shared notification center and
/// authorization is requested only once per process.
static NATIVE_INIT: Once = Once::new();

/// Monotonically increasing counter used to build a fresh notification
/// identifier when the caller doesn't supply an `identity`.
static ANON_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Maps a live `UNNotificationRequest` identifier to the click token that
/// should be emitted on the click channel if the user activates it.
static CLICK_TOKENS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();

fn click_tokens() -> &'static Mutex<HashMap<String, u64>> {
    CLICK_TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Entry point called from [`super::notify::deliver_desktop_notification_request`].
pub(super) fn deliver(req: &NotificationRequest<'_>) {
    if native_backend_usable() {
        deliver_native(req);
    } else {
        deliver_osascript(req);
    }
}

fn native_backend_usable() -> bool {
    let bundled =
        *NATIVE_BUNDLE_PRESENT.get_or_init(|| NSBundle::mainBundle().bundleIdentifier().is_some());
    bundled && !NATIVE_DISABLED.load(Ordering::Relaxed)
}

/// Mark the native backend as permanently unusable for the rest of the
/// process lifetime, and log why exactly once.
fn disable_native_after_error(context: &str) {
    NATIVE_DISABLED.store(true, Ordering::Relaxed);
    WARN_ONCE.call_once(|| {
        log::warn!(
            "macOS native notification backend failed ({context}); falling back to osascript for subsequent notifications"
        );
    });
}

// ---------------------------------------------------------------------------
// UNUserNotificationCenterDelegate
// ---------------------------------------------------------------------------

define_class!(
    // SAFETY:
    // - The superclass NSObject has no subclassing requirements.
    // - `NotificationDelegate` has no ivars and does not implement `Drop`.
    #[unsafe(super = NSObject)]
    struct NotificationDelegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for NotificationDelegate {}

    // SAFETY: The signatures below match the selectors they're registered under.
    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        /// Presents notifications (banner + sound) even while par-term is
        /// the foreground app — without this, foregrounded apps suppress
        /// their own notifications by default.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present_notification(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            completion_handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound,));
        }

        /// Looks up the click token registered for this notification's
        /// identifier (if any, and if the response isn't a plain dismissal)
        /// and emits it on the click channel.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive_notification_response(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            // SAFETY: `UNNotificationDismissActionIdentifier` is a valid
            // non-null extern NSString constant provided by the framework.
            let is_dismiss = unsafe { UNNotificationDismissActionIdentifier }.to_string()
                == response.actionIdentifier().to_string();
            if !is_dismiss {
                let identifier = response.notification().request().identifier().to_string();
                if let Ok(mut tokens) = click_tokens().lock()
                    && let Some(token) = tokens.remove(&identifier)
                {
                    let _ = super::notify::click_sender().send(token);
                }
            }
            completion_handler.call(());
        }
    }
);

impl NotificationDelegate {
    extern_methods!(
        #[unsafe(method(new))]
        fn new() -> Retained<Self>;
    );
}

// ---------------------------------------------------------------------------
// Native UNUserNotificationCenter path
// ---------------------------------------------------------------------------

/// Installs the delegate on the shared notification center and kicks off an
/// authorization request. Runs at most once per process.
fn ensure_native_initialized() {
    NATIVE_INIT.call_once(|| {
        let center = UNUserNotificationCenter::currentNotificationCenter();

        // The delegate is a weak property on the center (see `setDelegate`'s
        // docs), so something must own it for the process lifetime. We
        // intentionally leak the single instance for that purpose.
        let delegate = NotificationDelegate::new();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        std::mem::forget(delegate);

        let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
        let completion = block2::RcBlock::new(|granted: Bool, error: *mut NSError| {
            if !error.is_null() {
                disable_native_after_error("requestAuthorization");
            } else if !granted.as_bool() {
                log::warn!("macOS notification authorization was denied by the user");
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(options, &completion);
    });
}

fn deliver_native(req: &NotificationRequest<'_>) {
    ensure_native_initialized();

    let title = if !req.title.is_empty() {
        req.title
    } else {
        "Terminal Notification"
    };
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(req.message));
    // Newer interruption levels (timeSensitive/critical) require an
    // entitlement we don't request; Critical urgency only gets a sound cue.
    if req.urgency == NotificationUrgency::Critical {
        content.setSound(Some(&UNNotificationSound::defaultSound()));
    }

    let identifier = match req.identity {
        Some(identity) => identity.to_owned(),
        None => format!(
            "par-term-{}",
            ANON_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
        ),
    };

    if let Some(token) = req.click_token
        && let Ok(mut tokens) = click_tokens().lock()
    {
        tokens.insert(identifier.clone(), token);
    }

    let ns_identifier = NSString::from_str(&identifier);
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &ns_identifier,
        &content,
        None, // no trigger => deliver immediately
    );

    let has_click_token = req.click_token.is_some();
    let failed_identifier = identifier;
    let completion = block2::RcBlock::new(move |error: *mut NSError| {
        if !error.is_null() {
            disable_native_after_error("addNotificationRequest");
            if has_click_token && let Ok(mut tokens) = click_tokens().lock() {
                tokens.remove(&failed_identifier);
            }
        }
    });
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.addNotificationRequest_withCompletionHandler(&request, Some(&completion));
}

// ---------------------------------------------------------------------------
// osascript fallback
// ---------------------------------------------------------------------------

fn deliver_osascript(req: &NotificationRequest<'_>) {
    let title = if !req.title.is_empty() {
        req.title
    } else {
        "Terminal Notification"
    };
    let escaped_title = escape_for_applescript(title);
    let escaped_message = escape_for_applescript(req.message);
    // AppleScript's `display notification` has no urgency parameter; give
    // Critical notifications an audible cue instead.
    let script = if req.urgency == NotificationUrgency::Critical {
        format!(
            r#"display notification "{}" with title "{}" sound name "Basso""#,
            escaped_message, escaped_title,
        )
    } else {
        format!(
            r#"display notification "{}" with title "{}""#,
            escaped_message, escaped_title,
        )
    };
    // `osascript` spawn is a slow, variable blocking syscall, and these can
    // stack (check_notifications dispatches one per notification per frame on the
    // main thread). Run it on a worker thread so it can never freeze the event
    // loop. `script` is owned and moves into the thread.
    std::thread::spawn(move || {
        if let Err(e) = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
        {
            log::warn!("Failed to send macOS desktop notification: {}", e);
        }
    });
}
