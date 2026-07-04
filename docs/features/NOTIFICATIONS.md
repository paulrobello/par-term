# Notifications

par-term can raise desktop notifications in response to terminal events: the bell (`BEL`), activity/silence on a session, session exit, and application-driven notifications emitted via the OSC 9, OSC 777, and OSC 99 escape sequences. This document covers the application-driven paths; the bell and activity/silence options are listed in the [Configuration Reference](../CONFIG_REFERENCE.md#notifications).

## Table of Contents

- [Application notifications (OSC 9 / 777 / 99)](#application-notifications-osc-9--777--99)
- [Kitty OSC 99 — full metadata support](#kitty-osc-99--full-metadata-support)
- [Platform backends](#platform-backends)
- [Click actions](#click-actions)
- [Polling and background tabs](#polling-and-background-tabs)
- [Suppression and buffering](#suppression-and-buffering)
- [Payload size cap](#payload-size-cap)
- [Related docs](#related-docs)

## Application notifications (OSC 9 / 777 / 99)

Programs running in a pane can request a desktop notification directly:

| Sequence | Source | Body |
|----------|--------|------|
| `OSC 9 ; <text> ST` | iTerm2-style | The whole payload is the notification text. |
| `OSC 777 ; notify ; <title> ; <body> ST` | rxvt-style | Title and body are taken from the two `;`-separated fields. |
| `OSC 99 ; <kvs> ST` | Kitty desktop-notification spec | Key/value metadata (see below). |

Example, from a shell:

```bash
printf '\e]9;Build finished\e\\'                 # OSC 9
printf '\e]777;notify;Title;Body text\e\\'       # OSC 777
printf '\e]99;i=42;u=critical;Build failed\e\\'  # OSC 99
```

## Kitty OSC 99 — full metadata support

par-term adopts the Kitty desktop-notification spec on top of `par-term-emu-core-rust` 0.44.0. The supported keys are:

| Key | Meaning | par-term behavior |
|-----|---------|-------------------|
| `<text>` (payload) | Notification body | Shown as the notification text. |
| `i=` | Identity | Notifications redelivered with the same `i=` **replace** the previous one instead of stacking. |
| `u=` | Urgency (`low`/`normal`/`critical`) | `critical` is made sticky on Linux and given an audible cue on macOS; Linux also gets the freedesktop urgency hint and urgency-scaled timeouts. |
| `a=` | Click action | `focus` (the default) or `report` — see [Click actions](#click-actions). |

## Platform backends

| Platform | Backend |
|----------|---------|
| **macOS (bundled app)** | `UNUserNotificationCenter` via `objc2` — native same-id replacement, click delegate, foreground presentation. |
| **macOS (`cargo run`, unbundled)** | Automatic fallback to `osascript`, since `UNUserNotificationCenter` requires a signed/bundled app. |
| **Linux** | freedesktop DBus notifications (via `notify-rust`), with `replaces_id` for identity-based replacement. |
| **Windows** | Existing `notify-rust` behavior. |

## Click actions

Per the Kitty spec, `focus` is the default click action. When you click a notification:

- **`focus` (default)** — brings the par-term window to the front, activates the originating tab, and focuses the originating pane.
- **`a=report`** — writes the spec activation reply `OSC 99 ; i=<id> ; ST` back to the application through the PTY (using `i=0` when the original notification had no `i=`), so the app can react to the click.

The click registry is per-window, with a cross-window re-queue so a click is never lost if the originating window is not the focused one.

## Polling and background tabs

Every tab and pane is polled each frame for pending OSC 9/777/99 notifications, so a "build finished" alert raised in a background tab fires immediately rather than waiting for that tab to regain focus.

## Suppression and buffering

- `suppress_notifications_when_focused` (default `true`) suppresses desktop notifications while the par-term window has focus.
- `notification_max_buffer` (default `64`) caps how many OSC 9/777 notifications are retained. This cap is applied at terminal creation and on live config reload.

## Payload size cap

`max_osc_data_length` (default `134217728`, i.e. 128 MiB — matching the core) caps the total payload size of an OSC sequence before it is rejected as a memory-exhaustion guard. It is applied at terminal creation and on live config reload, and is exposed under **Settings → Advanced** (MiB units). See [Configuration Reference](../CONFIG_REFERENCE.md#terminal).

## Related docs

- [Configuration Reference — Notifications](../CONFIG_REFERENCE.md#notifications)
- [Assistant Panel / ACP](../ASSISTANT_PANEL.md)
- [Troubleshooting](../guides/TROUBLESHOOTING.md)
