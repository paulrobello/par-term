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
| `OSC 99 ; <metadata> ; <payload> ST` | Kitty desktop-notification spec | `<metadata>` is zero or more `:`-separated `key=value` pairs (see below); `<payload>` is the chunk text. |

Example, from a shell:

```bash
printf '\e]9;Build finished\e\\'                 # OSC 9
printf '\e]777;notify;Title;Body text\e\\'       # OSC 777
printf '\e]99;i=42:u=2;Build failed\e\\'         # OSC 99 (critical urgency)
```

## Kitty OSC 99 — full metadata support

par-term adopts the Kitty desktop-notification spec. The supported keys are:

| Key | Meaning | par-term behavior |
|-----|---------|-------------------|
| `<text>` (payload) | Notification body (or title when `p=title`) | Shown as the notification text (or title when paired with a `p=body` chunk). |
| `i=` | Identity | Notifications redelivered with the same `i=` **replace** the previous one instead of stacking. |
| `u=` | Urgency (`0` low / `1` normal default / `2` critical) | `critical` is made sticky on Linux and given an audible cue on macOS; Linux also gets the freedesktop urgency hint and urgency-scaled timeouts. |
| `a=` | Click actions (comma-separated list) | `focus` (the default) and/or `report`; each may be negated with a leading `-` (so `-focus` opts out of the default). See [Click actions](#click-actions). |
| `p=` | Payload type (`title` default, or `body`) | First chunk's text is the title; subsequent `p=body` chunks assemble the body. With no `p=body` chunk, the title text becomes the message (mirroring OSC 9/777). |
| `d=` | Done (`0` = more chunks follow, `1` = last chunk, default `1`) | Enables multi-chunk assembly for long notifications; the notification is only delivered when `d=1` (or unset) is seen. |
| `e=` | Encoding (`0` raw default, `1` base64) | Base64-decodes the payload before delivery, for binary-safe text. |

## Platform backends

| Platform | Backend |
|----------|---------|
| **macOS (bundled app)** | `UNUserNotificationCenter` via `objc2` — native same-id replacement, click delegate, foreground presentation. |
| **macOS (`cargo run`, unbundled)** | Automatic fallback to `osascript`, since `UNUserNotificationCenter` requires a signed/bundled app. |
| **Linux** | freedesktop DBus notifications (via `notify-rust`), with `replaces_id` for identity-based replacement. |
| **Windows** | Existing `notify-rust` behavior. |

## Click actions

Per the Kitty spec, `focus` is the default click action — it stays active unless the application explicitly opts out with `a=-focus`. `report` is never implied; it must be requested explicitly (typically as `a=focus,report`). When you click a notification:

- **`focus` (default)** — brings the par-term window to the front, activates the originating tab, and focuses the originating pane.
- **`report`** — writes the spec activation reply `OSC 99 ; i=<id> ; ST` back to the application through the PTY (using `i=0` when the original notification had no `i=`), so the app can react to the click.

The click registry is per-window, with a cross-window re-queue so a click is never lost if the originating window is not the focused one.

## Polling and background tabs

Every tab and pane is polled each frame for pending OSC 9/777/99 notifications, so a "build finished" alert raised in a background tab fires immediately rather than waiting for that tab to regain focus.

## Suppression and buffering

- `suppress_notifications_when_focused` (default `true`) suppresses desktop notifications while the par-term window has focus.
- `notification_max_buffer` (default `64`) caps how many OSC 9/777 notifications are retained. This cap is applied at terminal creation and on live config reload.

## Payload size cap

`max_osc_data_length` (default `134217728`, i.e. 128 MiB — matching the core) caps the total payload size of an OSC sequence before it is rejected as a memory-exhaustion guard. It is applied at terminal creation and on live config reload, and is exposed under **Settings → Advanced** (MiB units). See [Configuration Reference](../CONFIG_REFERENCE.md#security).

## Related docs

- [Configuration Reference — Notifications](../CONFIG_REFERENCE.md#notifications)
- [Assistant Panel / ACP](../ASSISTANT_PANEL.md)
- [Troubleshooting](../guides/TROUBLESHOOTING.md)
