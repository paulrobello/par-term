# Overlay Plugin Kind — Design Document

**Date**: 2026-09-24
**Status**: Approved — 2026-09-24 (owner accepted all recommendations)
**Board card**: `01a0d1cbce2a7a72bf30521b3d340de3` (par-term, medium)
**Prior decisions**: D7 (deferred until bundling + agent-commands land), O1–O6
accepted 2026-09-24

## Overview

The fourth plugin kind. Where `status-bar-widget`, `action-contributor`, and
`panel` extend par-term at its edges, `overlay` gives a plugin a persistent,
plugin-owned surface drawn **over** the terminal — the model the PLUGINS.md
v1-limits note explicitly deferred. Use cases: HUDs, dashboards, sticky
notes, a Pomodoro timer floating over the work.

Implementation follows the card's criterion: this document settles the four
open areas — layout vocabulary, input/focus routing, update protocol, and
permissions.

## Substrate (O1)

Overlay content renders in the **egui UI layer** (L6 in
`docs/architecture/COMPOSITOR.md`) — the layer that already renders the
palette, settings, toasts, and help, after all shaders, with existing
input/focus plumbing. A wgpu-drawn overlay (L3 family) was rejected: it
would need a custom draw-protocol over the wire and none of the input
plumbing, for pixel-level control plugin scenes do not need.

**Boundary with built-in decoration** — the L3 wgpu overlay layer stays
home to par-term's native transient decorations (dividers, pane titles,
visual bell, and the built-in pane-hint selection mode). Those are never
expressible as plugin overlays, and plugins cannot hook, mimic, or suppress
them (see Mode stack).

## Layout vocabulary

Each plugin owns **one overlay** (mirroring one-widget and one-panel per
plugin). An overlay is positioned by either:

- **Anchor** — one of nine positions (corners, edge midpoints, center), or
  an edge strip (`top` / `bottom` / `left` / `right`) with a thickness; or
- **Free rect** — x, y, width, height as fractions of the window
  (0.0–1.0), clamped to stay on screen.

Plus an opacity (0.0–1.0, default 1.0). Z-order is **host-assigned by
creation order**; no user dragging and no plugin-chosen z in v1. The host
may clamp overlay size (e.g. half the window per axis).

## Update protocol

Two new commands on the plugin's stdout, generalizing the `SetPanel` /
`ClearPanel` pattern; kind-purity holds — only an `overlay`-kind process
may send them:

```json
{"type": "SetOverlay", "overlay": {
  "id": "hud",
  "anchor": "top-right",
  "size": {"w": 0.25, "h": 0.4},
  "opacity": 0.9,
  "interactive": false,
  "content": {"type": "markdown", "text": "## Build\nPASSING"}
}}
{"type": "ClearOverlay", "id": "hud"}
```

- **Idempotent upsert by id** — same id replaces the whole overlay
  (full-scene replace, no diffs; scenes are small). Last write wins, like
  `SetWidget` and `SetPanel`.
- **Rate clamp** — the host drops updates beyond a cap (e.g. 30/sec) with
  a warning, so a runaway plugin cannot spin the renderer.
- **Scene vocabulary** (declarative tree, host renders through egui):
  `text`, `row` (layout of children), `markdown`, `button`, `text_input`,
  `list`, `progress`. No images in v1 (deferred; when added, paths
  resolve inside the plugin directory under the existing entry-confinement
  rule).
- **Clearing is guaranteed** — plugin stop or disable clears its overlay,
  same invariant as panels: a disabled plugin leaves no orphaned surface.

## Input & focus routing

- Overlays are **non-interactive by default**: they render, and mouse
  events pass through to the terminal beneath.
- `interactive: true` requires an `overlay` capability flag in the plugin
  manifest; a plugin without it has the flag forced off.
- An overlay **never steals focus on appear** (O4). It gains focus only by
  a click on one of its widgets, or by an explicit summon (palette action
  the plugin contributes). Escape returns focus to the terminal. At most
  one overlay is focused at a time.
- While an overlay is focused, the host sends the plugin **semantic events
  only** over its stdin — never raw keys:

```json
{"type": "OverlayEvent", "id": "hud", "event": {"type": "click", "widget": "deploy-btn"}}
{"type": "OverlayEvent", "id": "hud", "event": {"type": "text_changed", "widget": "filter", "value": "par"}}
{"type": "OverlayEvent", "id": "hud", "event": {"type": "select", "widget": "jobs", "index": 2}}
```

  The plugin owns state and pushes a new scene; the host owns pixels and
  input. Interactive vocabulary in v1 (O3): **button, text-input,
  list-select**.

## Mode stack & built-in boundary

par-term's focus consumers form a stack, settled here so later features
slot in consistently:

```
3  modal modes (pane-hint select, later: others)   — capture keys until resolved
2  focused plugin overlay                           — semantic events only
1  terminal                                         — normal keyboard input
```

- **Modal modes trump overlays.** The built-in pane-hint selection mode
  (letter badges on circle backgrounds, one centered in each pane, type the
  letter to focus that pane — built natively, not as a plugin; its own
  board card) renders its badges **above every plugin overlay**, and while
  it is active it captures keys regardless of overlay focus. A modal mode
  cannot be blocked, covered, or intercepted by plugin surfaces.
- Plugin overlays sit above terminal content but below modal-mode chrome.
- Plugins cannot draw during a modal mode's chrome, mimic its visuals, or
  receive its key stream.

## Permissions

Everything in the existing security model carries over unchanged:
subprocess boundary, entry confinement (and, later, image-path
confinement), land-disabled, explicit installs, kind-pure command
dispatch, manifest-declared event subscriptions. New axes:

- **`overlay` manifest capability** — required for the kind at all;
  `interactive` additionally required for focusable overlays.
- **Content clamps** — one overlay per plugin, size cap (half window per
  axis), update-rate cap, scene-size cap.
- **No raw input capture** — semantic events for the overlay's own widgets
  only; no global hotkeys, no key stream, no terminal-content access.
- A compromised overlay plugin can at worst draw pixels inside its clamped
  rect and hear clicks on its own buttons.

## Phasing (O2)

One design, two implementation phases:

1. **Phase 1 — display-only**: `SetOverlay` / `ClearOverlay`, anchors +
   free rects, non-interactive rendering, clearing on disable. Small diff:
   rendering only, no focus work.
2. **Phase 2 — interactive**: manifest `interactive` capability, focus
   stack, semantic event delivery, button / text-input / list widgets.

## Out of scope (follow-up cards, not this design)

- Images in scenes (path-confined when added)
- User-draggable overlays and plugin-chosen z-order
- More than one overlay per plugin
- Raw-key or global-hotkey access (never — constitutional)
- The built-in pane-hint selection mode itself (separate card; referenced
  here only for the mode-stack contract)

## Decisions (owner, 2026-09-24 — all recommendations accepted)

1. **O1**: egui layer substrate, not wgpu-drawn.
2. **O2**: one design, two phases — display-only first.
3. **O3**: interactive v1 = button, text-input, list-select.
4. **O4**: never auto-steal focus; click or explicit summon only; Escape
   returns to terminal.
5. **O5**: images deferred; entry-confinement applies when added.
6. **O6**: anchored presets **and** free rect in window fractions.
7. **(Owner input, same session)** The pane-hint letter-selection mode is
   built into par-term natively, not as a plugin; its existence is the
   source of the mode-stack rules above.
