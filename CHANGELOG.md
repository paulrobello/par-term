# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Bug Fixes
- **Static tmux-heavy tabs could tank FPS even without par-term splits** — the pane-render path is used whenever the tab's pane manager exists, which includes the normal single-pane case. That path rebuilt and cloned pane cell buffers on every frame, so tabs with large static screen contents could render much slower than simpler tabs even when the PTY was idle. Pane cell snapshots are now cached across frames by terminal generation and scroll offset, with copy-on-write only when focused-pane decorations actually mutate the cells.
- **Animated frames walked every tab title on every render** — `update_animations()` refreshed all tab/pane titles every frame, touching terminal state for every open tab and making idle-tab count reduce FPS. Title refresh is now throttled instead of running at render cadence.
- **Geometric shape characters (◼ ◻ ■ □ ▪ ▫ ◾ ◽ ▬ ▮) rendered vertically squished** — the pane render path only handled box-drawing, half-blocks, and block elements (U+2580–U+259F); Geometric Shapes (U+25A0–U+25FF) fell through to the font path and landed on the glyph-snap branch, which preserved the font's short baseline-relative metrics. Filled variants now render as pixel-perfect rectangles via `get_geometric_shape_rect`, and outline variants go through the same center+scale-to-fill treatment previously applied to `Symbol` chars (ballot boxes, dingbats).

---

## [0.30.9] - 2026-04-18

### Bug Fixes
- **Ctrl+Alt+letter chords collapsed to plain Ctrl+letter without enhanced modifier reporting** — `Ctrl+Alt+P` and `Ctrl+P` both reached inner TUIs as `0x10` because Alt/Option was discarded before the C0 control byte was returned. par-term now preserves the Alt modifier using the configured Option-key mode; with the default `esc` mode, `Ctrl+Alt+P` emits `\x1b\x10`, which apps can distinguish from plain `Ctrl+P`.
- **Hardcoded primary-modifier handlers shadowed registered shortcuts with extra modifiers** — chords like `Ctrl+Alt+R` could incorrectly fire simpler `Ctrl+R` handlers because `primary_modifier()` matched even when additional modifiers were held. `primary_modifier()` and `primary_modifier_with_shift()` now require exclusive modifier sets; the duplicate hardcoded `CmdOrCtrl+R` command-history check has been removed.

---

## [0.30.8] - 2026-04-16

### Bug Fixes
- **Shift+Enter still broken in kitty-keyboard TUIs under non-gateway tmux** — the previous fix relied on per-keypress `sysinfo` process-tree scanning to detect tmux, which was unreliable (failed silently, falling through to raw LF). tmux converts LF (0x0a) into Ctrl+J (its C0→Ctrl remapping in `tty-keys.c`), then re-encodes as `\x1b[106;5u` for `MODE_KEYS_EXTENDED_2` panes — the inner app never sees Shift+Enter. New approach: use alternate screen buffer state as the primary signal. TUI apps (and tmux wrapping TUIs) always enter alternate screen; when active, emit `\x1b[13;2u` so tmux's `extended-keys on` parser can re-encode for the pane's negotiated protocol. Process-tree detection kept as fallback. Shell context (no alternate screen) preserves the iTerm2 `\n` convention.
- **Unicode symbol characters (ballot boxes, dingbats) rendered vertically squished** — `BlockCharType::Symbol` characters (U+2600–U+26FF, U+2700–U+27BF) used baseline-relative font metrics that produce glyphs much shorter than the terminal cell height. Now centered in the cell and scaled to fill the cell height while maintaining aspect ratio.

---

## [0.30.7] - 2026-04-15

### Bug Fixes
- **Shift+Enter lost as soft-newline in kitty-keyboard TUIs running under tmux** — apps like the pi agent detect `$TMUX` and negotiate the kitty-keyboard protocol with tmux, after which a raw `\n` is no longer interpreted as Shift+Enter (only `\x1b[13;2u` is). par-term was emitting LF in every scenario following the iTerm2 convention, so Shift+Enter silently no-op'd inside tmux. Two-part fix: (1) in gateway mode (`tmux -CC`), route Shift+Enter via `send-keys -t %N -H 0a` so the literal LF bypasses tmux's per-pane `modifyOtherKeys` re-encoding (the old `C-j` path was being rewritten to `\x1b[27;5;106~` for mode-2 apps); (2) in subprocess tmux, detect a `tmux*` process under the active tab's shell via sysinfo and emit `\x1b[13;2u` instead of `\n`, letting tmux's `extended-keys on` parser re-encode for whatever keyboard protocol the inner app has negotiated. Outside tmux the iTerm2 `\n` convention is preserved so Claude Code and other non-kitty TUIs keep working.

### Build
- **MSRV bumped from 1.91 to 1.94** across all workspace crates (matches latest stable Rust). CI and release workflows updated to use 1.94.1.

---

## [0.30.6] - 2026-04-11

### Bug Fixes
- **Shift+digit/symbol sends unshifted char to crossterm apps outside tmux** — Claude Code and other crossterm-based TUIs received `1` instead of `!`, `[` instead of `{`, etc. when run in a normal tab (tmux masked the bug by re-encoding). par-term was emitting `modifyOtherKeys` sequences like `CSI 27;2;49~` for any Shift-modified printable, and crossterm cannot reverse-map a base codepoint to the shifted character without keyboard-layout tables. Fixed by matching iTerm2's `iTermModifyOtherKeysMapper` reference exactly: skip `modifyOtherKeys` encoding for any Shift-only combo (regardless of mode or character class) and let winit's layout-resolved shifted character pass through. Ctrl+digit and Ctrl+Shift+digit still encode via `modifyOtherKeys` as before.

---

## [0.30.5] - 2026-04-11

### Features
- **Move Tab to New Window** — tab context menu gains "Move Tab to New Window" and "Move Tab to Window ▸" (submenu listing every other par-term window). Moving a tab transfers its live PTY, scrollback, split panes, session logger, prettifier state, profile history, and custom title/color/icon without killing the shell. Tmux gateway and display tabs are disabled; solo-tab source windows auto-close after a merge into another window. A new keybinding action `MoveTabToNewWindow` pops the active tab out to a new window.

### Bug Fixes
- **Tab click sometimes required a second click to switch** — when the FPS gate dropped a `RedrawRequested`, any events already sitting in `egui_winit`'s `raw_input` accumulator (tab click press+release) stalled until an unrelated wake, so the click appeared to be ignored. Added `pending_egui_repaint` tracking in `should_render_frame()` and extended the `about_to_wait` self-heal to re-arm a frame at the earliest eligible time so the gap closes on its own.

### Dependencies
- Bumped `par-term-emu-core-rust` from 0.41.0 to 0.41.1.

---

## [0.30.4] - 2026-04-03

### Bug Fixes
- **Stale inline graphics persist after tmux split/clear** — when tmux redraws cells over Sixel/iTerm2/Kitty graphics without sending ED 2, images persisted indefinitely. Added three-layer invalidation: scroll detection, 500ms time-based grace period, and per-frame dirty-row threshold (>50% of graphic rows dirty).
- **Tab click leaks mouse press to tmux** — clicking a tab sometimes caused text selection/highlight in tmux panes. Fixed by always updating stored mouse position on cursor move, and marking tab-bar presses as consumed so the matching release is also blocked.
- **No UTF-8 locale when launched from Finder/Dock** — PTY environment had no LANG/LC_ALL/LC_CTYPE when launched outside a terminal, causing tmux and starship to fall back to ASCII. Now inherits locale vars from parent and defaults LANG to `en_US.UTF-8` when none are set.
- **Modifier keys stop working sporadically in alt-screen apps** — key handler used `try_write()` for read-only terminal mode queries; under render-thread write-lock contention, this failed and fell back to mode 0. Switched to `try_read()` for concurrent reader access.
- **Tmux box-drawing characters render as ASCII** — `build_shell_env()` did not inherit LANG/LC_ALL/LC_CTYPE from the parent process, so tmux fell back to ACS line-drawing. Fixed by inheriting locale env vars before merging user config overrides.

### Performance
- **FPS degradation in long tmux sessions** — two causes: (1) per-frame full cell Vec clone even on cache-hit frames, and (2) O(n) mark/history iteration growing linearly with session time. Fixed by restoring cache-hit guard and only iterating new entries beyond last synced position.

---

## [0.30.3] - 2026-04-01

### Bug Fixes
- **Modifier keys ignored for special keys outside tmux** — Shift/Ctrl/Alt+Arrow, Home, End, PageUp/Down, Insert, Delete, and F1-F12 sent unmodified escape sequences. Alt-screen apps (vim, htop) could not distinguish modified from plain keys. Now emits xterm-standard modifier-parameterized sequences.
- **URL underline position wrong in split panes and with scrollbar** — URL detection used renderer grid dimensions instead of actual pane terminal dimensions, causing row misalignment and underlines at wrong positions. Fixed by capturing terminal grid dims in the cell snapshot.
- **URL underline drifts when scrolling** — on lock-contention frames, stale URLs used old scroll offset while the renderer used the new one. Fixed by storing and using the detection-time scroll offset consistently.
- **URL underline lags behind content in alt-screen editors** — pane could acquire fresher cells than URL detection saw. Fixed by always populating pane cell cache from `extract_tab_cells`.
- **Content wraps incorrectly on split pane focus change** — clicking a pane with a scrollbar caused one frame of wrong text wrapping. Fixed by invalidating cached cells when dimensions don't match after scrollbar-induced resize.
- **Scrollbar disappears when split pane loses focus** — scrollbar inset was only applied to the focused pane, causing layout reflow on click. Fixed: scrollbar inset now applies to all panes in split mode; scrollbar shows for any pane with scrollback.
- **Per-pane scrollbar positioning wrong on HiDPI** — double-counted global insets already baked into viewport bounds. Fixed by deriving insets purely from viewport bounds.
- **Scrollbar width not rescaled on DPI change** — moving window between displays left scrollbar at old DPI width. Fixed by rescaling in `set_scale_factor`.

### Performance
- **Eliminate blocking lock in URL detection** — reuse cells from `extract_tab_cells` instead of re-acquiring `pty_session.lock()` + `terminal.lock()`. Eliminates FPS drops (60→12) with tmux 6+ panes. OSC 8 hyperlinks now fetched via non-blocking `try_get_all_hyperlinks()`.

---

## [0.30.2] - 2026-03-29

### Performance
- **Eliminate blocking mutex contention in render loop** — replaced blocking `lock()` calls in `get_cells_with_scrollback()` with non-blocking `try_lock()` and cache fallback. Fixes severe FPS drops (60→5) with animated shaders when tmux has many active panes.
- **Skip redundant cell generation for focused pane** — focused pane cells are now cached after the first call per frame, eliminating a duplicate cell generation + lock acquisition.
- **Upload only used portion of pane instance buffers** — GPU buffer uploads now send only the populated `[..index]` slice instead of the full window-grid-sized array, reducing per-pane staging bandwidth.

---

## [0.30.1] - 2026-03-26

### Bug Fixes
- **macOS/Linux: Shift+letter broken inside apps using modifyOtherKeys mode 2** — applications built with crossterm (e.g. Claude Code) set modifyOtherKeys mode 2, which caused par-term to encode `Shift+a` as `CSI 27;2;97~`. Crossterm receives this as `KeyEvent { code: Char('a'), modifiers: SHIFT }` but does not apply the SHIFT modifier to uppercase the character, producing lowercase `a`. Fix: shift-only alphabetic key combinations are now exempted from mode-2 encoding (matching the existing mode-1 exemption), allowing the logical-key path to send `'A'` directly.
- **Text selection highlight does not follow content when scrolling** — selection coordinates were stored as viewport-relative rows with no record of the scroll offset at capture time. After scrolling, `is_cell_selected` still tested the original viewport rows, freezing the highlight at those visual positions while the selected content moved. Fix: `Selection` now records `scroll_offset` at capture time; the renderer adjusts rows by the delta between the stored and current offsets before highlighting. Applies to all selection modes (normal, line, rectangular) in both single-pane and split-pane layouts.

---

## [0.30.0] - 2026-03-26

### Bug Fixes
- **Windows: Shift/Ctrl/Alt stop working after a notification or popup briefly steals focus** — on Windows, `WM_NCACTIVATE(false)` fires when any notification balloon or popup window becomes active, causing winit to emit `ModifiersChanged(empty)` and zero out all modifier state. Because keyboard focus is never actually lost, no `WM_SETFOCUS` fires to restore the state, leaving Shift/Ctrl/Alt permanently broken until the key is re-pressed. Fix: `InputHandler` now synthesizes modifier-state updates directly from `KeyboardInput` events for physical modifier keys (ShiftLeft/Right, ControlLeft/Right, AltLeft/Right, SuperLeft/Right). This is a no-op in the normal path (winit guarantees `ModifiersChanged` fires before `KeyboardInput`) and only corrects state when `ModifiersChanged` delivery is stale or missing.
- **tmux text selection: highlight persists and clipboard not populated on release** — when clicking between tmux panes, trackpad tap jitter could cause a press→drag→release sequence to be forwarded to tmux (which interpreted it as an empty selection), while simultaneously starting a local selection that was never finished. Root cause: `try_send_mouse_event` uses `try_write()` which can miss the lock during press (PTY reader holds it), causing the press to be handled locally (starting a selection); the release then succeeds via the alt-screen path and is consumed by tracking without completing the local selection. Fix: after tracking consumes a release, check for a pending local selection and call `handle_left_mouse_release()` to copy the text and clear the highlight.

### Security
- **ACP sensitive-path blocklist extended** — `is_sensitive_path()` now blocks `~/.aws/`, `~/.docker/`, `~/.netrc`, `~/.config/gh/`, and `~/.config/gcloud/` in addition to the existing `~/.ssh/`, `~/.gnupg/`, `/etc/` entries, closing a credential-exfiltration vector for connected AI agents with `auto_approve` enabled.
- **`NotebookEdit` reclassified as write operation** — previously listed as a read-only tool in the ACP permission auto-approval logic, allowing agents to silently modify Jupyter notebooks. Now routes through the write-path escalation and requires user approval.
- **ACP agent TOML override warning** — when a user-config-dir agent TOML overrides a built-in embedded agent identity, a warning is now logged and displayed, making the substitution visible.
- **Session logger credential redaction expanded** — added patterns for `GITHUB_TOKEN=`, `HEROKU_API_KEY=`, `npm_token=`, `pypi_token=`, `gitlab_token=`, `circleci_token=`, and `Bearer <token>`. Plain-text log files now include a header warning documenting known redaction limitations.
- **`resolve_shell_path()` validates `$SHELL` against allowlist** — the function now checks the shell basename against a known-shells allowlist before use, falling back to `/bin/sh` with a warning for unknown values.
- **`make clean` removes project-root log files** — `*.log` files at the repo root are now deleted by the `clean` target.

### Architecture
- **Resolved `render_pipeline` `#[path]` module redirect** — `render_pipeline` was declared via `#[path = "../render_pipeline/mod.rs"]` inside `window_state`, making it a logical child while physically a sibling. It is now declared directly in `src/app/mod.rs`, fixing silent `super::` path resolution and unblocking future `WindowState` decomposition.
- **`FontRenderingConfig` extracted from `Config`** — `font_antialias`, `font_hinting`, `font_thin_strokes`, and `minimum_contrast` are now grouped under `#[serde(flatten)] pub font_rendering: FontRenderingConfig`. Fully backward compatible with existing YAML configs.
- **`WindowConfig` extracted from `Config`** — `window_opacity`, `window_always_on_top`, `window_decorations`, `blur_enabled`, `blur_radius`, `window_padding`, `hide_window_padding_on_split`, and `snap_window_to_grid` now live under `#[serde(flatten)] pub window: WindowConfig`. Fully backward compatible with existing YAML configs.
- **Glyph font-fallback loop deduplicated** — `resolve_glyph_with_fallback()` extracted as a shared `CellRenderer` method; `text_instance_builder.rs` and `pane_render/mod.rs` both call it, eliminating the duplicated ~60-line font-fallback loop.
- **`pane_render/mod.rs` reduced below 800-line target** — powerline fringe logic extracted to `powerline.rs` (116 lines) and block-character rendering extracted to `block_char_render.rs` (222 lines). File reduced from 1,062 → 792 lines.

### Code Quality
- **`check_trigger_actions` refactored** — the 630-line God method now delegates to focused private helpers: `dispatch_trigger_action`, `handle_run_command_action`, `handle_send_text_action`, `handle_split_pane_action`, `handle_mark_line_action`.
- **Cursor-contrast logic deduplicated** — `compute_cursor_text_color()` extracted as a shared `pub(crate)` free function in `instance_buffers.rs`; both render paths now call it.
- **`show_action_edit_form` decomposed** — per-action-type form rendering extracted into eight private helper functions (~60 lines each), replacing a single 413-line inline match block.
- **`Vec<char>` hot-path allocation eliminated** — per-cell `grapheme.chars().collect::<Vec<char>>()` in the render loop replaced with direct iterator calls (`chars().next()`, `chars().nth(1)`).
- **`RowCacheEntry` phantom struct replaced** — `pub(crate) struct RowCacheEntry {}` replaced with `pub(crate) type RowCacheEntry = bool`.
- **`ActionBase` helper added to `CustomActionConfig`** — `base()` / `apply_base()` methods reduce `set_keybinding`, `set_prefix_char`, `set_keybinding_enabled` from 18-line match blocks to 4-line calls; `into_copy()` reduced from ~70 lines to 10.
- **Repeat action bounded** — `MAX_SAFE_REPEAT_COUNT = 100` guard added in `snippet_actions.rs` to prevent config-based DoS from unbounded `thread::sleep` loops.
- **Orphaned `test_cr.rs` and `test_grid.rs` deleted** — standalone files at repo root that were not part of any crate.

### Documentation
- **`CONTRIBUTING.md` factual corrections** — fixed stale module paths (`src/terminal/` → `par-term-terminal/src/`), wrong sub-crate count (13 → 14, added `par-term-prettifier` to Layer 2 table), wrong `input_events` path (file → directory), wrong incremental build time (30–40s → 1–2s), and wrong `dev-release` profile specs (opt-level 3 / thin LTO → opt-level 2 / no LTO).
- **README config default corrected** — `tab_bar_mode` example corrected from `when_multiple` to `always` (the actual default since v0.20.0).
- **`docs/MIGRATION.md` created** — documents breaking behavior changes across v0.20.0, v0.25.0, and v0.27.0 including renamed config fields, security-gated trigger execution, and prettifier external-commands default-deny.
- **`docs/README.md` navigation improvements** — added `Contributing` link to Architecture & Development table; added `Getting Started` as first row in the Getting Started table.
- **README CI badge added** — GitHub Actions CI status badge now shown in the badge row.
- **`docs/DOCUMENTATION_STYLE_GUIDE.md` updated** — added explicit "Actual Layout: Flat docs/ Directory" section documenting the conscious deviation from the prescribed subdirectory structure.

---

## [0.29.2] - 2026-03-25

### Fixed
- **Clicking a tab while the app is unfocused now reliably selects that tab** — focus-clicks on the tab bar were forwarded to egui, but egui's `clicked_by()` detection could miss them if pointer state was stale from when the window lost focus. The native event handler now also hit-tests the cached tab rects directly and stores a pending switch that fires in post-render if egui doesn't detect the click itself.
- **Cmd+Shift+/- characters no longer leak to the terminal** — font size increase/decrease shortcuts now also handle the shifted variants (`Cmd+Shift+=` producing `+`, `Cmd+Shift+-` producing `_`) so the characters are consumed instead of being forwarded to the PTY.
- **Ctrl+L and Ctrl+Shift+K now target the focused pane in split-pane mode** — clear screen and clear scrollback were always operating on the tab's root terminal; they now route through the focused pane's terminal, matching normal keyboard input routing.
- **`--shader` CLI flag now overrides configured shader and background image** — `create_window()` was reloading config from disk after `App::new()` patched it with the CLI shader, silently discarding the override. The patch is now re-applied after each config reload so `--shader <name>` and `--screenshot` work correctly for gallery generation.

### Changed
- **Gallery screenshots added for `jellyfish.glsl` and `rain-glass.glsl`** — two background shaders were missing from `gh-pages/gallery/`.
- **Shader manifest regenerated** — `shaders/manifest.json` updated to include SHA256 hashes for all 75 shader bundle files.
- **Documentation sync** — corrected stale values across multiple docs: `blur_radius` default (`20` → `8` in WINDOW_MANAGEMENT.md), `tab_html_titles` default (`true` → `false` in TABS.md), settings section label in ACCESSIBILITY.md, badge display values in PRETTIFIER.md, shader debug file paths in TROUBLESHOOTING.md, and missing child-process environment variables in ENVIRONMENT_VARIABLES.md. README shader count updated to 52+.

---

## [0.29.1] - 2026-03-20

### Fixed
- **URL hover cursor no longer gets stuck as a pointer** — moving the mouse from a URL link into the tab bar or opening the profile drawer now correctly restores the cursor and title when leaving a URL.
- **Paste routes to the focused pane in native split panes** — keyboard and middle-click paste now targets the focused pane's PTY instead of always using the primary pane.
- **URL detection underlines no longer persist or appear at wrong positions in split panes** — the cache-miss check now uses the focused pane's terminal generation, so content changes correctly trigger URL re-detection.
- **Middle-click paste now focuses the clicked pane in split panes** — middle-clicking a non-focused pane now switches keyboard focus before dispatching the paste.
- **Clicking a tab while the app is in the background now selects that tab (macOS)** — `acceptsFirstMouse` is now enabled so the activation click is forwarded to the app.
- **Updated dependencies** — bumped `clap`, `objc2`, `rodio`, and `tar` to latest versions.

---

## [0.29.0] - 2026-03-17

### Added
- **Per-pane title tracking** — each pane now stores its own last-known title (`title` + `has_default_title` fields on `Pane`). `Tab::update_title()` iterates all panes each frame and updates each pane's title from its own terminal's OSC sequences and shell-integration CWD, then derives the tab bar title from the focused pane. Switching focus between split panes now instantly reflects the correct title without waiting for the next terminal output. Local hostname and home-directory lookups are hoisted once per frame (not once per pane) to avoid redundant syscalls in split-pane configurations.
- **Hide tmux control-mode gateway tab** — new config option `tmux_hide_gateway_tab` hides the `tmux -CC` gateway tab from the tab bar while tmux window tabs are active. The gateway tab is automatically restored when the session ends. Configurable in **Settings → Advanced → Tmux**.
- **Tmux session persistence across restarts** — the active tmux control-mode session name is now saved and restored with window session state, automatically reconnecting to the tmux session on app restart.

### Fixed
- **Stale mouse state when switching pane focus** — clicking to switch focus between split panes could leave `button_pressed = true` on the old pane, causing spurious text selection on the next click back. The old pane's mouse state is now explicitly cleared during focus transfer.
- **Tmux control-mode crash on key press** — fixed a panic when pressing keys in tmux control mode before the session was fully initialized.
- **Tmux control-mode display-tab, TUI content, and mouse routing** — fixed several tmux control-mode issues: display-tab close handling, TUI content rendering in tmux panes, and mouse event routing to the correct tmux pane.

---

## [0.28.0] - 2026-03-14

### Added
- **Duplicate Tab keybinding** — `duplicate_tab` is now a named keybinding action, allowing users to assign any hotkey to clone the current tab via **Settings → Keyboard → Actions**. The action was previously only accessible via the tab context menu and macOS menu bar.
- **New tab position** — new config option `new_tab_position` controls where new tabs are inserted in the tab bar. `end` (default) preserves existing behavior; `after_active` inserts the new tab immediately to the right of the currently active tab. Applies consistently to all user-initiated new-tab actions (keyboard shortcut, "+" button, profile picker, custom `NewTab` actions). Session undo and arrangement restore are unaffected — they always restore tabs to their original positions. Configurable in **Settings → Window → Tab Bar**; searchable via "new tab position" / "after active".
- **Per-tab tmux auto-connect via profiles and arrangements** — profiles can now automatically connect to a named tmux session when opened, and arrangements capture/restore that session on save/restore:
  - **Profile fields** — `tmux_session_name` (string, empty = disabled) and `tmux_connection_mode` (`control_mode` / `normal`). Uses create-or-attach semantics (`tmux new-session -A -s <name>`), so the session is created if absent or reattached if it already exists.
  - **Control Mode** (default) — connects via `tmux -CC` for full par-term integration: pane sync, window tabs per tmux window, and input routing.
  - **Normal Mode** — writes a plain `tmux new-session -A -s <name>` command to the PTY; tmux runs in the terminal with no par-term integration.
  - **Arrangement capture/restore** — saving an arrangement records the active control-mode session name per window; restoring reconnects each window to its saved session automatically (failures are logged as warnings, not errors).
  - **Settings UI** — a collapsible **Tmux Auto-Connect** section in the profile editor provides a session name text field and radio buttons for Control Mode vs Normal. Discoverable via settings search (`tmux`, `tmux session`, `auto-connect`).
  - Auto-connect is skipped silently when `tmux_enabled = false` or when the window is already connected to a tmux gateway.
- **Remote tab title format** — two new config fields control how the tab title is displayed when shell integration detects a remote host (via SSH):
  - `remote_tab_title_format` — choose between `user_at_host` (e.g. `alice@server`, default), `host` (hostname only), or `host_and_cwd` (e.g. `server:~/projects`). The `host_and_cwd` format abbreviates the remote user's home directory to `~` using the remote username, not the local `$HOME`.
  - `remote_tab_title_osc_priority` — when `true` (default), an explicit OSC title sequence (`\033]0;...`) takes priority over the remote format; when `false`, the format always wins.
  - Both options are exposed in **Settings → Window → Tab Bar** with a combo box and checkbox below the existing "Tab title mode" control. All six new options are discoverable via settings search (`remote tab title`, `ssh title`, `osc priority`, etc.).
- **Workflow action types** — three new custom action types enable multi-step automation without leaving the terminal:
  - **`sequence`** — runs a list of actions in order. Each step has an optional `delay_ms` and an `on_failure` policy (`abort` / `stop` / `continue`). Sequences can nest inside other sequences; circular references are detected at execution time and show an error toast.
  - **`condition`** — evaluates a check and branches to a different action based on the result. Five check kinds: `exit_code`, `output_contains`, `env_var`, `dir_matches` (glob), `git_branch` (glob). Standalone use dispatches `on_true_id` / `on_false_id`; as a sequence step, the result drives the step's `on_failure` behavior.
  - **`repeat`** — runs a single action up to N times with an optional delay between iterations. Supports `stop_on_success` and `stop_on_failure` for early exit.
- **`capture_output` for `shell_command` actions** — set `capture_output: true` to capture the command's stdout+stderr (capped at 64 KB) and exit code. Subsequent `condition` checks against `exit_code` or `output_contains` read from this captured context. The Settings UI exposes a checkbox to enable this per-action.
- **Clone button for custom actions** — each custom action row in Settings → Actions now has a "Clone" button that duplicates the action, appends `-copy` to its title, assigns a fresh ID, and inserts the copy immediately below the original. The keybinding and prefix char are cleared on the clone to avoid immediate conflicts.

### Fixed
- **Clicking between native split panes no longer triggers text highlighting** — switching pane focus set `button_pressed = true` on the old pane before `focus_pane_at()` transferred focus, leaving the old pane with a stale flag that was never cleared (mouse release only clears the currently-focused pane). When the user clicked back to the old pane, the early-return in the pane-focus path skipped `click_pixel_position` setup, but the stale `button_pressed = true` combined with the pane's previous `click_pixel_position` caused the next mouse-move to satisfy the drag-selection threshold, spuriously highlighting text. The fix clears `button_pressed` on the old pane via `pane_manager.get_pane_mut()` inside the focus-switch block.
- **Tab click no longer leaves a stuck selection highlight** — if the user dragged from the terminal into the tab bar and released the mouse there, `handle_left_mouse_release()` was never called (the tab bar early-return fired first), leaving `is_selecting = true` and a visible text-selection highlight that persisted until the next terminal click. The guard that already cleared `button_pressed` before any early returns now also clears `is_selecting`.
- **Local tab titles no longer show only the hostname** — `shell_integration_hostname()` returns the local machine's hostname for any terminal with shell integration active, not just SSH sessions. The remote-host branch was therefore always taken, applying the remote title format (hostname only) to local tabs. Fixed by comparing the reported OSC 7 hostname against `hostname::get()` and only treating the tab as remote when the two differ. If the local hostname cannot be determined the tab is conservatively treated as local.
- **Braille spinner characters render in the tab bar** — egui's default fonts and the bundled Nerd Font Symbols do not cover the Braille Patterns Unicode block (U+2800–U+28FF), causing Claude Code's thinking indicator (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏) to render as □. A platform-specific system font that covers the full Braille block is now loaded as a fallback (Apple Braille.ttf on macOS; DejaVu Sans Mono / GNU FreeMono on Linux; Segoe UI Symbol on Windows).
- **Text selection now works in native split panes** — clicking within the already-focused pane no longer silently blocked selection anchoring. The pane-focus handler previously returned early for every click in multi-pane mode (including clicks within the already-focused pane), preventing the selection anchor from ever being stored.
- **Selection highlight row alignment in native split panes** — drag-selection rows were up to half a cell off because the renderer's vertical centering offset (`center_offset_y`) was not subtracted in `pixel_to_pane_cell`. The coordinate mapping now mirrors the same centering formula used by `gather_pane_render_data`.
- **Split-pane divider highlight no longer gets stuck** — the hover highlight on native pane dividers could freeze permanently after a drag-resize if the mouse button was released outside the terminal area (e.g. in the tab bar, profile drawer, or context menu). Root cause: those early-return paths in the mouse-button handler skipped the `button_pressed = false` update, leaving `dragging_divider` set with `button_pressed` still `true`. All subsequent mouse moves then hit the divider-dragging early-return in `handle_mouse_move`, bypassing hover detection entirely. Fixed with two changes: (1) `button_pressed` is now cleared to `false` at the top of `handle_mouse_button` on left-button release before any early returns; (2) `handle_mouse_move` detects when `dragging_divider` is set but `button_pressed` is false, clears the stuck drag state, and falls through to hover detection so the highlight clears immediately.
- **Shell integration installer uses `$HOME`** — the source and PATH lines written to `.bashrc` / `.zshrc` / `config.fish` during shell integration install now use `$HOME/` instead of the literal home-directory path, making the entries portable across user renames and shared dotfile repositories.
- **Scrollback flash in tmux when mouse tracking lock is contended** — when a mouse scroll event arrived while the terminal write lock was held by the PTY reader, par-term failed to detect that tmux had mouse tracking enabled and incorrectly handled the scroll locally, briefly scrolling par-term's own scrollback buffer (showing old tmux output) instead of forwarding the event to tmux. The fix skips the scroll event entirely on lock contention so it is re-evaluated on the next tick. Also added a per-pane cell cache so lock misses during rendering reuse the last successfully gathered cells instead of rendering an empty pane, and fixed the `extract_tab_cells` lock-failure fallback which hardcoded `is_alt_screen: false`, causing prettifier state thrashing.
- **Session storage tests compile with tmux session field** — added missing `tmux_session_name` field to test `SessionWindow` initializers.

---

## [0.27.0] - 2026-03-12

### Security
- **Trigger `i_accept_the_risk` guard** — triggers with `prompt_before_run: false` now require an explicit `i_accept_the_risk: true` field; execution is blocked with an audit warning if absent. Every no-prompt execution is logged at warn level.
- **Shader installer requires checksum** — installing shaders from a GitHub release without a `.sha256` asset now returns a hard error instead of proceeding with a warning.
- **Custom SHA-256 replaced with `sha2` crate** — removed the hand-rolled SHA-256 implementation in the shader installer; uses the `sha2` workspace dependency.
- **Prettifier external commands default-deny** — `ExternalCommandRenderer` now refuses execution when `allowed_commands` is empty (the default); users must explicitly list permitted commands.
- **Clipboard paste control-char warning** — adds `warn_paste_control_chars` config option (default `true`) that logs a warning when clipboard content contains VT escape sequences.
- **`O_NOFOLLOW` for debug log file** — the debug log open path on Unix now uses `O_NOFOLLOW` to close a TOCTOU symlink-race window.
- **`allow_all_env_vars` startup warning** — a prominent warning is emitted at startup when `allow_all_env_vars: true` is detected in config, recommending a local-only override file.
- **Session logging warning** — a one-time notice is printed when session logging starts, showing the log path and advising that sensitive data may be captured.

### Added
- **`split_pane` custom action** — a new action type that splits the active pane and optionally runs a command in the new pane. Supports two command modes:
  - **Shell mode** (`command_is_direct: false`, default): the command is sent as text to the shell with a trailing newline; the shell stays running when the command finishes.
  - **Direct mode** (`command_is_direct: true`): the new pane's PTY runs the command directly as its process; the pane closes automatically when the command exits. Best for interactive tools like `htop`, `vim`, or `watch`.
- **`new_tab` custom action** — custom actions can now open a new tab and optionally send a command to that tab's shell after launch. The settings UI exposes this as a dedicated **New Tab** action type with a multiline command editor.
- **Custom-action prefix mode** — custom actions can now be triggered with a tmux-style two-stroke binding: configure a global `custom_action_prefix_key`, assign a single-character `prefix_char` to each action, then press the prefix key, release it, and press that character. The settings UI exposes both fields, supports recording the prefix combo, warns about duplicate prefix chars and prefix-key conflicts, keeps the prefix toast visible while the mode is armed, and lets you cancel prefix mode with `Esc`.
- **`split_percent` for `split_pane` actions** — both custom actions and trigger `split_pane` now accept a `split_percent` field (10–90, default `66`). This controls how much of the current pane the *existing* pane retains after the split; the new pane receives the remainder. Keyboard-shortcut splits (`Ctrl+\` etc.) are unaffected and continue to split 50/50. The settings UI displays the percent in the type indicator (`[Split-vert-66]`) and exposes a drag-control in the editor. Config example: `split_percent: 66`.
- **`split_pane` trigger action** — triggers can now open a new horizontal or vertical pane and optionally run a command in it when a regex pattern matches terminal output. Supports `send_text` and `initial_command` sub-types for the post-split command, and a `target` field (`active` or `source`) for future per-pane source tracking.
- **`prompt_before_run` confirmation dialog** — dangerous trigger actions (`RunCommand`, `SendText`, `SplitPane`) now show an interactive modal dialog before executing. The dialog offers three choices: **Allow Once** (run this one time), **Always Allow** (auto-approve for the rest of the session), and **Deny** (discard). Setting `prompt_before_run: false` bypasses the dialog; the rate-limiter and command denylist still apply.
- **`typecheck` Makefile target** — `make typecheck` runs `cargo check --workspace`; also added to `make checkall`.
- **Sub-crate READMEs** — created README.md for 11 previously undocumented sub-crates: `par-term-acp`, `par-term-fonts`, `par-term-input`, `par-term-keybindings`, `par-term-prettifier`, `par-term-render`, `par-term-scripting`, `par-term-settings-ui`, `par-term-terminal`, `par-term-tmux`, `par-term-update`.
- **`ATLAS_SIZE` constant** — replaced 16 scattered `2048.0` magic literals in the render pipeline with `pub(crate) const ATLAS_SIZE: f32 = 2048.0;`.
- **151 docstrings** added to `par-term-config/src/defaults/` functions and other undocumented public API items.
- **`//!` crate-level doc comment** added to `par-term-input/src/lib.rs`.
- Updated `par-term-emu-core-rust` dependency to v0.41.0 (adds `TriggerAction::SplitPane` / `ActionResult::SplitPane`).

### Changed
- **`require_user_action` renamed to `prompt_before_run`** on trigger definitions. The old name is accepted as a YAML alias — existing config files continue to work without modification.
- **Settings window opens 10% wider** by default, giving the custom-actions editor and list more room before truncation.
- **Rust toolchain pinned to 1.91.0** — `release.yml` `RUST_VERSION` updated from `1.85.0` to `1.91.0` to match CI; `rust-toolchain.toml` channel pinned from `"stable"` to `"1.91.0"`.
- **`rust-version = "1.91"` added to all 14 sub-crate `Cargo.toml` files** — aligns MSRV declarations across the workspace.
- **Removed redundant `resolver = "2"`** from root `Cargo.toml` (Edition 2024 defaults to resolver v2).
- **`Tab::Drop` no longer sleeps 50 ms** — removed the blocking `std::thread::sleep(50ms)` on tab close; `abort()` is non-blocking.
- **Render-path `.unwrap()` replaced with `.expect()`** — the 3 `unwrap()` calls in `par-term-render/src/renderer/rendering.rs` now carry descriptive invariant messages.
- **Keyboard shortcuts doc corrected** — Linux/Windows column for Next/Prev tab and Move tab left/right now shows `Ctrl+Shift` (was `Cmd+Shift`).
- **README updated to v0.27.0 and Rust 1.91+**.

### Fixed
- "Skip This Version" in the update dialog now persists across restarts — the `save_config` flag from the render pass was not being acted on, so `skipped_version` was never written to disk.
- URL and file path detection now strips trailing sentence punctuation (`.`, `!`, `?`) so that paths at the end of sentences (e.g., "the file is at ~/thefile.txt.") no longer include the trailing period in the highlight or click-to-open target.
- Custom-action prefix follow-up keys are now fully captured while prefix mode is armed, so bound prefix chars and `Esc` no longer leak through to the terminal.
- The custom-actions list now renders in its own full-width container below the prefix-key row, keeping item text left-aligned and preserving the `Edit` / `Delete` buttons on narrower settings windows.
- Custom action keybinding conflict checker no longer reports a false conflict when re-editing an action that already has a saved keybinding.
- Keybinding conflict warning in the action editor is now shown below the input row instead of inline, preventing the Record button from being pushed off-screen.
- `PageUp`/`PageDown` are now forwarded to terminal applications (e.g., `joe`, `less`, `vim`) as `\x1b[5~`/`\x1b[6~`; scrollback navigation now requires `Shift+PageUp`/`Shift+PageDown`, consistent with `Shift+Home`/`Shift+End`.
- Middle-click paste in tmux now focuses the clicked pane before pasting — a synthetic left-click press/release is sent at the cursor position when mouse tracking is active, matching iTerm2 behaviour.
- File drops now target the pane under the cursor — in split-pane and tmux modes the dropped file path is sent to the pane at the drop position instead of always going to the focused pane. In tmux gateway mode the text is routed through `send-keys` to the correct tmux pane.
- Clippy `field_reassign_with_default` violation in `par-term-config` prettifier test — replaced with struct initializer form.
- Removed dead `segment_texts()` helper from `par-term-prettifier` markdown inline tests.
- Removed 3 dead `keywords()` functions from settings-ui badge, progress-bar, and arrangements tabs.

### Performance
- **`dev-release` profile optimized** — `opt-level 2`, no LTO, `codegen-units = 16`, `incremental = true`, `strip = false` for ~1-2s incremental rebuilds at ~90-95% of full release performance.

---

## [0.26.0] - 2026-03-11

### Security
- Block `file://` URLs in dynamic profile fetcher to prevent local-file-read via SSRF.
- ACP `auto_approve` mode now always enforces `is_safe_write_path` for write-class tools.
- Shader downloads verify SHA256 checksum when a `.sha256` asset is present in the release.
- Added 50 MB response body size limit to shader downloads.
- macOS private API calls (`CGSSetWindowBackgroundBlurRadius`, SkyLight SLS) now gated behind an OS version check (≥ 13).
- ACP file/directory tools (`read_file_with_range`, `list_directory_entries`, `find_files_recursive`) block sensitive paths (`~/.ssh/`, `~/.gnupg/`, `/etc/`).
- ACP agent validates `$SHELL` is an absolute path to a known shell binary before use.
- Session data files written with `0o600` permissions; session log directories set to `0o700`.
- Config file persistence enforces `0o600` permissions to protect secrets in `env_vars`.

### Added
- `[workspace.dependencies]` table in root `Cargo.toml` centralizing 38 shared dependency versions across all 15 crates.
- `deny.toml` for `cargo-deny` license/vulnerability/ban auditing with CI integration.
- `assets/Info.plist.template` replacing inline echo chains in Makefile bundle target.
- `rust-version = "1.91"` MSRV declared in workspace `Cargo.toml`.
- `get_or_rasterize_glyph()` shared helper in `atlas.rs`, deduplicating glyph cache logic from 3 call sites.
- `SearchHighlightParams` struct replacing a 9-parameter function signature.
- `fill_visible_separator_marks()` scratch-buffer API for separator mark computation.
- Scratch buffers on `Renderer`, `CellRenderer`, and `WindowState` for per-frame allocations (divider instances, row cells, prettifier block IDs).
- Named types replacing `#[allow(clippy::type_complexity)]` in 5 locations: `ScriptPassResult`, `PrettifierRef`, `PrettifierGraphicRef`, `TerminalContext`.
- Sub-crate README files for `par-term-config`, `par-term-ssh`, `par-term-mcp`.
- `cargo install par-term` installation instructions added to README and GETTING_STARTED.
- Table of Contents added to README.md and QUICK_START_FONTS.md.

### Changed
- Split pane divider drag hit width default reduced from 8 px to 5 px.
- Reduced dark-theme tab color preset brightness by 25% for better visual comfort.
- Updated `par-term-emu-core-rust` dependency to v0.40.0.
- CI toolchain pinned to SHA-verified `dtolnay/rust-toolchain` action at `toolchain: 1.91.0`.
- CI `cargo test` now uses `--workspace` to exercise all sub-crate tests.
- `make checkall` now runs `fmt-check` instead of `fmt` to avoid mutating source during checks.
- Dev tool binaries (`test_cr.rs`, `test_grid.rs`) moved from workspace root to `src/bin/`.
- Removed `wgpu-types` feature from `par-term-prettifier`'s dependency on `par-term-config`.
- Removed 6 unused GPU utility functions from `gpu_utils.rs`; retained only `create_sampler_with_filter`.
- Removed dead `_shaping_options` variable from `pane_render/mod.rs` and `instance_buffers.rs`.
- Replaced `unreachable!()` panic with `log::warn!` fallback in `bg_instance_builder.rs`.
- GPU device poll errors now emit `log::warn!` instead of being silently discarded.

### Fixed
- tmux mouse highlight no longer gets stuck spanning all panes — when mouse tracking (e.g. tmux) consumes a press, the local selection state is now cleared so stale highlights cannot persist indefinitely.
- tmux drag-selection no longer snaps to word boundaries mid-drag — the spurious second mouse-press from the image-guard code is now suppressed when mouse tracking is already active.
- Clicking between tmux panes no longer wipes the clipboard — trackpad micro-movements are now suppressed within the same 8 px dead zone used for local text selection.
- Middle-click paste now takes priority over mouse tracking and alt-screen mode, matching iTerm2 behaviour.
- Eliminated the dark gap between powerline separator glyphs and adjacent colored segments in the tmux status bar when using background-image mode.
- Fixed regression where custom shader background was hidden by opaque default-bg cell quads; new `fill_default_bg_cells` flag controls default-bg rendering independently of `skip_solid_background`.
- `compute_visible_separator_marks` doc comment corrected to match actual implementation.
- Stale file paths in CLAUDE.md and CONTRIBUTING.md corrected to current crate locations.
- `CONFIG_REFERENCE.md` defaults updated: `window_padding` 4.0→1.0, `minimum_contrast` 1.0→0.0.
- README install instructions updated from `cargo build --release` to `make build` / `make run`.
- Expanded `docs/README.md` index and CLAUDE.md Docs Reference table to cover all current docs.

---

## [0.25.0] - 2026-03-07

### Security
- Added amber warning banner in Settings → Automation when any trigger has `require_user_action: false` (SEC-002).
- File and URL opening uses direct `process::Command` spawn (no login shell) when the editor template contains only `{file}/{line}/{col}` placeholders, eliminating shell-escape injection (SEC-003).
- ACP permission validation serializes the canonicalize-and-compare phase with a process-wide mutex, closing a TOCTOU race between concurrent checks (SEC-004).
- ACP agent subprocess spawns without a shell interpreter when the resolved command contains no shell metacharacters (SEC-005).
- Session log redaction expanded with 45 additional patterns covering API keys, AWS credentials, PEM headers, cloud/vault tokens, database passwords, and 2FA/TOTP prompts (SEC-006).
- Migrated from `serde_yml` to `serde_yaml_ng` to resolve upstream vulnerabilities.
- Enforced command allowlists for `ExternalCommandRenderer`.
- Blocked HTTP profile URLs by default; added MitM risk warnings.
- Strengthened update checker with domain allowlists and binary content validation.
- Improved permissions for session logs and MCP IPC files; added password redaction warnings for session logging.
- Prevented accidental commit of local API tokens via `.gitignore`.
- Added path traversal prevention for config paths and shader names.
- Hardened tmux command escaping to prevent truncation via null bytes.

### Added
- New `jellyfish.glsl` background shader: animated procedural jellyfish with caustic light shimmer, bioluminescent particles, and neon blue/purple palette.
- Configurable chat font size for the Assistant panel (10–24 pt slider, default 14 pt, live-reloads).
- Replace button on saved arrangement rows — captures current layout and overwrites in-place with inline confirmation.
- MP3 audio file support for alert sounds (`rodio` mp3 feature enabled); settings hint text updated to reflect WAV/MP3/OGG/FLAC.
- File picker ("Browse…") button next to each alert sound path field, with audio file type filter.
- `snap_window_to_grid` config option (default: `true`) — snaps window to exact terminal cell boundaries on resize, disabled automatically in split-pane mode.
- `link_highlight_color_enabled` config option (default: `true`) — when disabled, URLs/file paths are underlined without changing text colour.
- Configurable visual bell flash color (`notification_visual_bell_color`, default: white) with color picker in Settings → Notifications → Bell.
- `ScriptCommand` handlers for `WriteText`, `Notify`, `SetBadge`, `SetVariable`, `RunCommand`, and `ChangeConfig` with permission opt-ins and rate limiting.
- Per-pane selection state isolation: each pane owns its own selection, click tracking, and drag state.
- `UIElement` trait with GAT-based context parameter (`type Ctx<'a>`) on `TabBarUI` and `StatusBarUI`, enabling unit testing without a live GPU context.
- `RenderLoopState` sub-struct extracted from `WindowState`.
- `make install-shell-integration` Makefile target to copy shell integration scripts (bash, zsh, fish) to `~/.config/par-term/`.
- Expanded Nerd Font icon presets: "UI Actions" (16 icons) and "Navigation" (16 icons) categories, plus 4 more icons in "Status & Alerts".
- `docs/ENTERPRISE_DEPLOYMENT.md` guide covering bulk installation, MDM/Jamf packaging, and multi-user deployment.
- `docs/ENVIRONMENT_VARIABLES.md` and `docs/API.md` references.
- Three-mutex policy documented in `src/lib.rs` and `docs/MUTEX_PATTERNS.md`; try-lock failure telemetry added.
- `src/ui_constants.rs` centralizing UI layout dimensions.
- Customizable `timeout_secs` for snippet shell commands.
- 73 keybinding integration tests and 97 copy mode state machine tests.

### Changed
- Consolidated rendering: removed dormant single-pane orchestrator (`render_orchestrator.rs`, `CellRenderer::render()`, `render_sixel_graphics()`); extracted duplicated 3-phase draw sequence into `emit_three_phase_draw_calls()`; net ~450 lines removed.
- Assistant panel: suppressed "Suggested command" box when Terminal Access is enabled (command already auto-executes from the agent's code block).
- Assistant panel system guidance now clearly distinguishes fenced code-block execution (user's PTY) from the Bash tool (subprocess), with a prominent `RUNNING COMMANDS:` header.
- Pane padding: no padding in single-pane mode; split mode adds automatic base padding equal to half the divider width. Default `pane_padding` changed from `4.0` → `1.0` px; default `window_padding` changed from `0.0` → `1.0` px.
- Removed unused cursor shader parameter controls (Trail duration, Glow radius, Glow intensity) from Settings UI.
- `pause_refresh_on_blur` now defaults to `true` (reduced-FPS mode enabled when window loses focus).
- `tab_inactive_outline_only` now defaults to `true` (inactive tabs render outline-only by default).
- Dark Background, Light Background, Pastel, and High Contrast themes: all colors corrected to match iTerm2's sRGB values (converted from NSCalibratedRGBColorSpace).
- Default pane background opacity changed from 0.85 → 1.0; default font size increased from 12 → 13.
- `minimum_contrast` slider capped at 0.99; saved value of 1.0 auto-migrated to 0.0 (disabled) on load.
- Minimum contrast refactored from WCAG ratio scale (1.0–21.0) to iTerm2-compatible perceived brightness scale (0.0–1.0).
- Scrollbar settings (width, colors, autohide, command markers) moved from Terminal tab to Window tab in Settings.
- `WindowState` and `Config` decomposition continued: `RenderLoopState`, `WindowConfig`, `FontConfig`, `ScrollbackConfig`, `ThemeConfig`, `NotificationConfig` sub-structs extracted; 12 named sub-structs now in place.
- `pane_render.rs` split into `pane_render/` submodule with `cursor_overlays.rs` and `separators.rs`.
- `TerminalManager` reorganized into `progress.rs`, `terminal_config.rs`, `tmux_control.rs`, `triggers.rs`, `observers.rs`.
- `shader_metadata.rs` split into `shader_metadata/parsing.rs` and `shader_metadata/cache.rs`.
- `renderer/shaders.rs` split into `shaders/background.rs`, `shaders/cursor.rs`, `shaders/shared.rs`.
- Mutex policy documentation consolidated to `src/lib.rs`; re-export patterns standardized to consistent `pub mod` style.
- Integration tests now document the PTY device requirement and how to run with `--include-ignored`.
- Centralized config saves with a 100ms debounce; prettifier disabled by default; automatic CI triggers enabled for main and PRs.

### Fixed

#### Prettifier & Shaders
- Prettifier cell substitution now applies to pane cells (always-active path) instead of invisible `FrameRenderData.cells`; pipeline `terminal_width` tracks actual terminal dimensions; feed stride uses real cell array stride.
- Updated shader manifest: bumped to 0.24.0, added `rain-glass.glsl`, refreshed 7 stale SHA256 hashes.
- Fixed `generate_manifest.py` to exclude hidden directories and write a trailing newline.
- Custom background shaders with `custom_shader_full_content: true` now work correctly — pane content is rendered to the shader's intermediate texture before the shader runs, so CRT/bloom/dither/retro-terminal shaders can properly distort terminal content.
- Cursor shaders now render correctly in the pane path (`render_split_panes`); all content targets the cursor shader's intermediate texture when active.
- Custom background shaders (e.g., rain) no longer render as fully transparent in split-pane mode; shader render pass now uses final-mode opacity.
- Fixed translucent right-half background on macOS: all background modes now render a full-screen opaque quad via `bg_image_pipeline`; also fixed pane-viewport fill quad using pixel coordinates instead of NDC.

#### Assistant Panel / ACP
- Terminal Access auto-drive now sends an auto-context notification after each auto-executed command so the agent can see the exit code and continue multi-step tasks.
- Agent now receives a `[Terminal access enabled]` context block on each prompt when Terminal Access is enabled, fixing cases where the agent incorrectly reported having no terminal access.
- Added tooltip to the Terminal Access checkbox.
- Fixed "too many open files" error when spawning agents with large shader directories — switched from kqueue (O_EVTONLY fd per file) to FSEvents (path-based, no per-file fds).
- MCP `config_update` tool now writes IPC files to the correct directory (`~/.config/par-term/` instead of `~/Library/Application Support/par-term/`).

#### Split Pane
- Scrollbar no longer leaks from original pane to a newly-split pane.
- URL/file-path highlights and underlines no longer appear in the wrong pane; `detect_urls()` now reads from the focused pane's terminal; mouse hover and Cmd/Ctrl+Click use pane-local coordinates.
- Spurious visual bell flash after splitting panes fixed — bell count now read from the correct pane's terminal.
- Scrollbar width no longer scales with window size during resize; cache key now includes window dimensions.
- Terminal text no longer renders behind the scrollbar; focused pane column count reduced by scrollbar width when visible.
- Scrollbar GPU uniform cache now includes viewport bounds, preventing stale geometry after splits.
- Theme changes now apply to all split pane terminals, not just the primary pane.
- Restoring a session with split panes no longer causes exit on first keypress.
- Cell grid now centered within each pane, distributing remainder pixels evenly on all sides.
- Per-pane selection reads from the focused pane's terminal buffer.
- Fixed drag-selection clipboard copy using `blocking_write()` to eliminate `try_write()` race.
- Fixed clicking between tmux panes overwriting clipboard via accidental micro-selections.
- Double-click and triple-click selection no longer fails due to `try_write()` contention.

#### Cursor Rendering
- Block cursor text color setting now has visible effect (initial color changed to red `[255,0,0]`); also fixed `current_col` not incrementing on glyph resolution failure.
- Block cursor text color now applies in split-pane mode (`build_pane_instance_buffers`).
- Beam/underline cursor no longer hidden under text in pane renderer; 3-phase rendering (`cell bgs → text → cursor overlays`) added to `render_pane_to_view`.
- Hollow cursor now appears in pane renderer when window loses focus; hollow borders use alpha=1.0 independent of blink phase.
- Hollow cursor also fixed in the single-pane path: 3-phase rendering added to all three render methods; fixed hollow cursor border alpha and `is_block` guard.
- Cursor guide and cursor shadow now render in the pane path (`build_pane_instance_buffers`); hollow cursor border width reduced from 2 px to 1 px.
- Cursor remains hidden while scrolling into scrollback (`scroll_offset > 0`).

#### Rendering
- ▄/▀ half-block gradient banding fixed: half-block characters now rendered entirely via text pipeline (two quads per cell), eliminating cross-pipeline seams.
- Non-uniform text brightness fixed: `cell_width` now rounded to integer pixels at initialization, font-change, and cols/rows calculation, ensuring every glyph renders at scale 1.0.
- Fixed macOS silent exit on Cmd+, before clicking in the window: replaced `PredefinedMenuItem::quit` with a custom graceful-shutdown item; menu installed before GPU initialization.
- Semantic file-path highlighting no longer bleeds across tmux pane separators (regex stops at box-drawing characters U+2500–U+257F).
- Tab bar (left position) new-tab buttons now render at the top instead of the bottom.

#### URL / Link Detection
- URL/file hover cursor no longer flickers on every render frame; hover/cursor state now owned exclusively by `mouse_move`.
- URL/file-path highlighting now correctly applies foreground color and underline decoration in the pane render path.

#### Scrollbar
- Scrollbar cross-tab contamination fixed: `marks_override_scrollbar` only forces scrollbar visible when `scrollback_len > 0`; GPU cache key includes marks count.
- `clear` command now removes command markers from the scrollbar immediately via `ScreenCleared` event.
- Scrollbar thumb/track color changes in Settings now take effect immediately (cache reset on appearance change).

#### Shell Integration & History
- Shell integration now correctly detects running commands: OSC 133;C handler parses command text from params[2].
- Close-tab confirmation now correctly shows with the tab close button: replaced `try_write` with `blocking_read` in running-job checks.
- Close-tab/pane confirmation now routes through the full cleanup path (session undo, tab bar resize, alert sounds).
- Bash shell integration exit code capture fixed: uses `${__bp_last_ret_value:-$?}` to avoid `__bp_interactive_mode` clobbering `$?`.
- `ScrollbackMetadata::apply_event` now applies D-marker exit code correctly; `CommandHistory::update_exit_code_if_unknown` updates from any differing `Some` value.
- Command history and paste-special UI icons now use Nerd Font PUA codepoints instead of missing Unicode shapes.

#### Session Restore
- `auto_restore_arrangement` now takes priority over `restore_session` when both are enabled.
- Fixed alternating launch bug: single-pane tabs now save `pane_layout = None`; `restore_pane_layout()` guard added against old session files.

#### Search
- Search highlights (Cmd+F) now appear correctly: highlights applied to focused pane cells in `gpu_submit.rs` after `gather_pane_render_data`.
- Search navigation (▲/▼) and close (×) buttons now render correctly using Font Awesome icon codepoints.

#### Other
- Command complete alert sound now plays when a command finishes (`play_alert_sound(AlertEvent::CommandComplete)` was never called).
- PTY child processes no longer inherit `TMUX`, `TMUX_PANE`, `STY`, or `WINDOW` from the parent terminal.
- Custom shaders section in Settings → Integrations now works correctly (shader callbacks wired from main crate to settings UI).
- Badge overlay now accounts for tab bar, status bar, scrollbar, and AI inspector panel when positioning.
- Status bar separator character in Settings UI now renders correctly (TextEdit switched to monospace font).
- Wired `process_sync_actions` in TmuxSync for session, layout, output, and flow-control notifications.
- Fixed highlight flickering in `detect_urls` by preserving stale lists on lock misses.
- Resolved `window_opacity` state corruption during `render_to_texture`.
- Improved left/right modifier remapping logic.
- Resolved various panic-prone `.expect()` calls; added response size limits for update checker and ACP file reads.
- Fixed orphaned trigger processes; improved tmux control mode cleanup on session end.
- Fixed panics in command truncation with multi-byte UTF-8 characters.
- Annotated all `unsafe` blocks with `// SAFETY:` justifications.

### Refactored
- Full codebase audit (28 findings): eliminated all 23 `#[allow(clippy::too_many_arguments)]` suppressions via parameter builder structs; extracted `GlobalShaderConfig`, `AiInspectorConfig`, `StatusBarConfig` sub-structs; split 12 files exceeding 800 lines; removed all dead-code suppressions; consolidated duplicate `shell_detection` and `profile_modal_ui` implementations. 1,065 tests pass.
- Extracted `src/prettifier/` (93 files, 22,778 lines) and `src/ansi_colors.rs` into `par-term-prettifier` workspace sub-crate (R-03).
- Promoted `src/app/window_state/render_pipeline/` to `src/app/render_pipeline/` using `#[path]`; zero caller changes (R-14).
- Eliminated all 19 flat `.rs` files from `src/app/` root; moved to `window_state/`, `pane/`, `copy_mode/`, `tmux_handler/` (R-02).
- Decomposed 890-line `submit_gpu_frame()` into `update_gpu_renderer_state()`, `render_egui_frame()`, `scroll_offset_from_tab()` (R-31).
- Added 7 semantic color accessors to `ThemeColors`; migrated 250+ raw `palette[N]` indices across json/yaml/toml/xml parsers (R-34).
- Split large files: `markdown/tests.rs` → 5 sub-files (R-36); `cli.rs` → `cli/mod.rs` + `cli/install.rs` (R-37); `settings_actions.rs` 795 → 233 lines via `config_propagation.rs` (R-39); `file_transfers.rs` 780 lines → 3 modules (R-42); `copy_mode/mod.rs` 637 lines → 5 focused modules (R-46); `TabBarUI` extracted to `tab_bar_ui/state.rs` + `horizontal.rs`, `mod.rs` 714 → 361 lines (R-47).
- Extracted test files out-of-line for `xml.rs` (R-40), `regex_detector.rs` (R-41), `config_bridge.rs` → `rule_loader.rs` (R-43), `tab/mod.rs` → `pane_accessors.rs` (R-45).
- Extracted `ConfigurableDetector` subtrait; `RendererRegistry` dispatches through `as_configurable_mut()` (R-51).
- Centralized `make_block`/`make_block_with_command` test factories; removed 24 duplicate local definitions (R-30).
- Added `with_active_tab()`, `with_window()`, `request_redraw()` helpers on `WindowState` (60+ call sites); `try_with_terminal()` / `try_with_terminal_mut()` helpers on `Tab` (7 call sites).
- Created `src/platform/` consolidating platform-specific notification delivery and modifier key detection, reducing `#[cfg]` blocks across 8 files.
- Created `TerminalAccess`, `UIElement`, and `EventHandler` traits in `src/traits.rs`.
- Added per-pane state accessors on `Tab` routing through `PaneManager`; updated ~30 call sites.
- Migrated terminal access from `Mutex` to `RwLock` for better read concurrency.
- Added 128 new tests for coordinate translation, pane bounds, and file splits.

### Performance
- Eliminated per-frame GPU buffer allocations for pane backgrounds using a uniform buffer cache.
- Implemented scratch `Vec` reuse in `CellRenderer`.
- Added regex caching for triggers; replaced per-frame `StyledLine` clones with borrows.
- Integrated native filesystem watchers for config hot-reload.

### Documentation
- Added `docs/ENTERPRISE_DEPLOYMENT.md`, `docs/ENVIRONMENT_VARIABLES.md`, `docs/API.md`, `docs/MUTEX_PATTERNS.md`.
- Updated `CONTRIBUTING.md`, `docs/CONCURRENCY.md`, `docs/STATE_LIFECYCLE.md`, `docs/ARCHITECTURE.md` with deep technical overviews.
- Simplified `README.md` with a quick start guide.
- Added per-module documentation for re-exports, locking rules, and architectural patterns; documented 3-tier shader resolution chain and legacy `Tab` field migration plans.

---

## [0.24.0] - 2026-02-27

### Fixed
- **Box-Drawing Line Thickness**: Snapped box-drawing pixel rectangles to integer boundaries for consistent line thickness.
- **Prettifier Improvements**: Fixed source-to-rendered line mapping, synced cell dimensions for inline graphics, and implemented Claude Code integration enhancements.
- **Security & Reliability**: Sanitized paste control characters, restricted MCP IPC file permissions, and redacted passwords from session logs.
- **System**: Implemented graceful shutdown sequence and restricted config variable substitution to an allowlist.

### Changed
- **Internal Architecture**: Decomposed `window_state.rs` into focused sub-modules and extracted render coordination functions.

---

## [0.23.0] - 2026-02-25

### Added
- **Content Prettifier**: New system to detect and render structured content (Markdown, JSON, etc.) with syntax highlighting and format-specific enhancements.

### Changed
- **Font Hinting**: Enabled by default for improved text sharpness.
- **Dependencies**: Updated workspace dependencies to latest versions.

### Fixed
- **Settings Search**: Fixed and updated search keywords across all settings tabs.
- **Split Pane Mode**: Fixed inline graphics, scrollback, and scrollbar rendering in split-pane layouts.
- **Window Arrangements**: Resolved DPI-related positioning and sizing issues on multi-monitor setups.
- **Rendering**: Fixed character artifacts in glyph atlas and improved symbol rendering from emoji fonts.
- **Usability**: Improved text selection in mouse-tracking apps and fixed trackpad micro-selection jitter.

---
## [0.22.0] - 2026-02-22

### Added
- **Assistant Panel**: Added code block rendering, message queueing/cancellation, and multi-line chat input.
- **ACP Integration**: Support for custom ACP agents (including Ollama) and better context restoration across reconnects.
- **Debugging**: New `par-term-acp-harness` for reproducing Assistant Panel sessions and `terminal_screenshot` MCP tool.
- **Aesthetics**: New `glass-sphere-bounce.glsl` shader and sharpened tab bar borders.

### Changed
- **Dependencies**: Updated `par-term-emu-core-rust` and rebranded Claude ACP bridge package.
- **Security**: Split screenshot permissions from YOLO mode.

### Fixed
- **Performance**: Resolved input and shader lag by refining idle-throttling logic.
- **ACP Handshaking**: Fixed connection failures in app bundles and nested session blocking.
- **UI/UX**: Resolved chat input visibility issues, UTF-8 command truncation panics, and Escape key behavior.

---
## [0.21.0] - 2026-02-20

### Added
- **Customization**: Replaced emoji presets with ~120 Nerd Font icons and added support for per-tab custom icons and manual renaming.
- **Tab Behavior**: New `tab_title_mode` for finer control over automatic title updates.

### Changed
- **Power Efficiency**: Major reduction in idle CPU usage (~103% to ~18-25%) via adaptive polling and conditional dirty tracking.
- **UI Responsiveness**: Decoupled idle wakeup cadence from FPS and throttled inactive tab refresh.

### Fixed
- **Multi-Window Layouts**: Fixed tab property restoration for arrangements with multiple windows.
- **Responsiveness**: Resolved input lag during heavy output by switching to `try_lock()` in the render path.
- **Rendering**: Fixed tab bar corner thickness, scrollbar overlap, and vertically squashed Unicode symbols.

---
## [0.20.0] - 2026-02-20

### Added
- **Updates**: Hourly update check frequency and a new clickable status bar widget for available updates.
- **UI/UX**: Dropdown new-tab menu, real-time pane background previews, and a file transfer progress overlay.
- **Shaders**: New `rain-glass.glsl` background shader and an outline-only mode for inactive tabs.

### Changed
- **Defaults**: Disabled window padding by default and set `tab_bar_mode` to `always`.

### Fixed
- **File Transfers**: Fixed uploads hanging over SSH and implemented background threads for PTY writes.
- **Split Panes**: Corrected mouse event routing and divider resize logic in split-pane mode.
- **Rendering**: Resolved inline image display issues for large files and fixed live window padding updates.

---
## [0.19.0] - 2026-02-19

### Added
- **Link Highlighting**: Configurable link highlight colors, underlining support, and stipple underline style.
- **Settings**: Auto-focus for settings search input.

### Fixed
- **Shutdown**: Implemented fast window shutdown by moving I/O to background threads.
- **Symbols**: Fixed media control character rendering as colored emoji.
- **Distribution**: Reduced crate package size by excluding non-essential files.

---
## [0.18.0] - 2026-02-18

### Added
- **Quick Settings**: Added BG and Cursor Shader toggles to the quick settings strip.
- **Focus Tracking**: Forward CSI focus-in/out sequences to PTYs for applications like tmux.

### Fixed
- **Rendering**: Fixed dingbat/symbol characters rendering as colored emoji instead of monochrome.
- **Input**: Suppressed focus clicks to prevent accidental clipboard loss in mouse-aware apps.
- **Shell Detection**: Improved shell detection with multi-strategy fallback.
- **Settings**: Fixed empty icons in the settings sidebar and resolved version display issues.

### Refactored
- Collapsed `src/config/` re-export layer (~4,800 lines of duplicates removed).
- Extracted SSH, keybinding, scripting, update, input, MCP, and tmux subsystems into dedicated workspace crates.

---
## [0.17.1] - 2026-02-18

### Changed
- Updated workspace dependencies including `zip`, `mdns-sd`, and `ureq`.

### Fixed
- **macOS**: Resolved self-update quarantine issues by stripping Gatekeeper attributes.
- **CI**: Fixed workspace subcrate publishing order.

---
## [0.17.0] - 2026-02-17

### Added
- **Assistant Panel**: DevTools-style panel for terminal inspection and ACP agent integration.
- **Shader Assistant**: Context-triggered shader expertise for agents.
- **File Transfers**: Native UI for iTerm2 OSC 1337 transfers.
- **Per-Pane Backgrounds**: Independent background images for each split pane.
- **Scripting**: New Python-based scripting manager for reacting to terminal events.
- **Team Features**: Dynamic profile loading from remote URLs.
- **Aesthetics**: Auto dark mode and automatic tab styling based on system theme.

### Changed
- Refactored core modules (fonts, terminal, settings, rendering) into dedicated workspace crates.
- Renamed "AI Inspector" to "Assistant".

### Fixed
- Resolved Shift+Tab interception issues.
- Implemented instant window shutdown on macOS.

---
## [0.16.0] - 2026-02-13

### Added
- **Status Bar**: Configurable bar with widgets for system monitoring and session info.
- **Remote Integration**: Support for installing shell integration via SSH.
- **Native Menus**: Platform-appropriate settings access from application menus.
- **SSH Host Management**: Integrated SSH config parsing and Quick Connect dialog.
- **Profile Improvements**: Profile selection on new-tab button and per-profile shell overrides.

---
## [0.15.0] - 2026-02-12

### Added
- **Auto-Switching**: Automatically switch profiles based on current working directory patterns.
- **UI/UX**: Nerd Font icon picker for profiles and support for tab style variants.
- **Audio**: Configurable alert sounds for terminal events.
- **History**: Fuzzy search overlay for command history.
- **Session Management**: Session undo (reopen closed tabs) and automatic session restoration on startup.
- **Layout**: Support for bottom and left tab bar positions.

### Improved
- Moved profile management directly into the Settings window.

### Fixed
- Resolved HiDPI/DPI scaling issues across all UI components.
- Fixed keyboard shortcut routing in egui overlays.

---
## [0.14.0] - 2026-02-11

### Added
- **Self-Update**: In-place update system detecting installation method (Homebrew, cargo, bundle, etc.).
- **Command Separators**: Optional horizontal lines between shell commands using OSC 133 marks.
- **Config Variables**: Environment variable substitution in `config.yaml` using `${VAR}` syntax.
- **Tab Reordering**: Drag-and-drop support for reordering tabs in the tab bar.
- **Window Arrangements**: Save and restore named window layouts with monitor-aware positioning.
- **Settings Persistence**: Persistent expand/collapse states for settings window sections.

### Changed
- Increased default `font_size` to 12.0.

### Fixed
- Improved update notifications and resolved duplicate arrangement name issues.

---
## [0.13.0] - 2026-02-10

### Added
- **Copy Mode**: Keyboard-driven text selection and navigation (Vi-style).
- **Unicode Normalization**: Support for NFC (default), NFD, NFKC, and NFKD forms.
- **Snippets & Actions**: Completed custom variables UI, key sequence simulation, and import/export.

### Fixed
- Resolved emoji rendering issues, tmux pane resize via mouse drag, and link highlighting offsets.

---
## [0.12.0] - 2026-02-10

### Added
- **Snippets & Actions**: New system for text automation and custom macros.
- **Progress Bars**: Thin overlay bars supporting OSC 9;4 and OSC 934 protocols.
- **Paste Improvements**: Configurable paste delay and new newline-control transformations.
- **Pane Enhancements**: GPU-rendered title bars and customizable divider styles.
- **Integration**: OSC 1337 RemoteHost support and current command display in window title.

### Changed
- Major cross-platform keybinding overhaul and modernized terminfo.

### Fixed
- Resolved pane focus indicator settings, background opacity issues, and Linux Ctrl+C behavior.

---
## [0.11.0] - 2026-02-06

### Added
- **Automation**: New "Automation" settings tab for managing regex triggers and coprocesses.
- **Triggers**: Match terminal output to fire actions (highlight, notify, play sound, send text, etc.).
- **Coprocesses**: Background processes that receive terminal output with restart policies.
- **Accessibility**: WCAG-based minimum contrast enforcement.
- **Semantic History**: Ctrl+click (Cmd+click) on file paths to open them in a configured editor.
- **Logging**: Configurable runtime log level control.

### Changed
- Unified logging bridge and improved coprocess PATH resolution.

### Fixed
- Resolved trigger mark deduplication and improved scrollbar command text capture.

---
## [0.10.0] - 2026-02-04

### Added
- **Confirm Close**: Confirmation dialog when closing tabs/panes with active jobs.
- **Exit Action**: Configurable behavior when a shell process exits (close, keep, restart).
- **Modifier Remapping**: Independent remapping for left/right Ctrl, Alt, and Super keys.
- **Physical Keys**: Option to match keybindings by physical position (scan code).
- **Keyboard Protocols**: Support for XTerm `modifyOtherKeys` extension.
- **Performance**: iTerm2-style flicker reduction and manual "Maximize Throughput" mode.
- **Customization**: GPU power preference and per-profile badge configuration.

### Fixed
- Resolved arrow key issues in `less` and other pagers using DECCKM mode.

---
## [0.9.0] - 2026-02-04

### Added
- **Profiles Tab**: New tab in Settings for profile management and drawer visibility toggle.
- **tmux Formatting**: Customizable tmux status bar content via format strings.
- **Welcome Dialog**: Added a link to the changelog in the onboarding popup.

### Fixed
- Resolved segfaults on exit, Windows ARM64 build failures, and HTTPS request panics.
- Improved Windows taskbar icon handling and file watching.

---
## [0.8.0] - 2026-02-03

### Added
- **Startup Directory**: Control over initial working directory (home, previous, or custom).
- **Badge System**: Semi-transparent text overlays with dynamic session variables.
- **Tab Enhancements**: Support for tab stretching and HTML markup in titles.
- **UI/UX**: Tooltips for scrollbar marks and "Reset to Defaults" button in Settings.

### Changed
- Updated core library and enabled tab stretching by default.

### Fixed
- Resolved Windows console window visibility and bash shell integration exit codes.

---
## [0.7.0] - 2026-02-02

### Added
- **Integrations**: Unified installation system for shell integration and shader bundles.
- **Settings**: Added missing UI controls for various configuration options.
- **tmux**: Native status bar display and improved multi-client sync in control mode.
- **Session Logging**: Automatic recording of terminal output in text, HTML, or asciicast formats.
- **Profile System**: Full CRUD for named profiles with a collapsible drawer.
- **Window Management**: New window types (fullscreen, edge-anchored) and target monitor selection.
- **Unicode**: Configurable Unicode version and ambiguous width settings.
- **Paste Special**: Command palette for transforming clipboard content before pasting.
- **Notifications**: Desktop alerts for session exit, activity, and silence.
- **Mouse**: Advanced mouse features including Option+Click cursor movement and focus-follows-mouse.
- **Selection**: Smart selection rules and auto-quoting for dropped files.
- **Search**: Incremental search through scrollback buffer with match highlighting.
- **Font**: Rendering options for anti-aliasing, hinting, and thin strokes.

### Fixed
- Resolved tmux pane display issues, Shift+Enter behavior, and multi-window focus routing.
- Improved DPI scaling across all UI components and fixed various rendering overlaps.

---
## [0.6.0] - 2026-01-29

### Added

- **Shader Gallery**: Visual gallery with screenshots of all 49+ included shaders
  - Hosted on GitHub Pages at https://paulrobello.github.io/par-term/
  - Auto-deploys on changes to gh-pages folder
- **CLI Options**: New command-line flags for automation and scripting
  - `--screenshot <path>`: Take screenshot and save to file
  - `--shader <name>`: Override background shader
  - `--exit-after <seconds>`: Exit after specified duration
  - `--command <cmd>`: Run command instead of default shell
- **Configurable Keybindings**: Customize all keyboard shortcuts
  - Edit `~/.config/par-term/keybindings.yaml`
  - Support for modifier keys (Ctrl, Alt, Shift, Super)
- **Shader Distribution System**: Easy shader installation
  - `par-term install-shaders` CLI command
  - Downloads shaders from latest GitHub release
  - Options: `-y` (no prompt), `--force` (overwrite existing)

### Fixed

- **Option+Click Cursor Movement**: Use arrow key sequences instead of absolute cursor positioning
  - Shells interpret arrow keys correctly for cursor movement within command line
  - Queries terminal's actual cursor position to calculate movement delta
- **Option+Click Selection Conflict**: Prevent text selection when Option+click moves cursor
  - Button press state now set after special click handlers return
  - Rectangular selection changed to Option+Cmd (matching iTerm2)
- **Custom Shader Background Handling**: Preserve solid color background when custom shader is disabled
- **Full Content Mode Compositing**: Shader output used directly without re-compositing terminal content on top

### Documentation

- Synced COMPOSITOR.md and CUSTOM_SHADERS.md with current implementation
- Updated README with CLI shader installer instructions

---

## [0.5.0] - 2026-01-29

### Added

#### Settings & Configuration
- **Standalone Settings Window**: Moved settings UI from overlay to dedicated window
  - `F12` or `Cmd+,` (macOS) / `Ctrl+,` (Linux/Windows) to open
  - Automatically brought to front when terminal gains focus
  - View and edit settings while terminal content remains visible
- **Per-Shader Configuration System**: 3-tier configuration for background and cursor shaders
  - Shader metadata defaults embedded in GLSL files (`/*! par-term shader metadata ... */`)
  - Per-shader user overrides in `shader_configs` section of config.yaml
  - Global config fallback for unspecified values
  - "Save Defaults to Shader" button to write settings back to shader files
  - Per-shader UI controls for animation_speed, brightness, text_opacity, texture channels
- **Shader Hot Reload**: Automatic shader reloading when files are modified on disk
  - Configurable via `shader_hot_reload` (default: false) and `shader_hot_reload_delay` (default: 100ms)
  - Desktop notifications on reload success/failure
  - Visual bell on compilation errors when enabled
- **Power Saving Options**: Reduce resource usage when window is unfocused
  - `pause_shaders_on_blur` (default: true): Pause shader animations when unfocused
  - `pause_refresh_on_blur` (default: false): Reduce refresh rate when unfocused
  - `unfocused_fps` (default: 30): Target FPS when window is unfocused
- **Cursor Lock Options**: Prevent applications from overriding cursor preferences
  - `lock_cursor_visibility`: Prevent apps from hiding cursor via DECTCEM
  - `lock_cursor_style`: Prevent apps from changing cursor style via DECSCUSR
  - `lock_cursor_blink`: Prevent apps from enabling cursor blink when user has it disabled
- **Background Mode Options**: Choose between theme default, solid color, or background image
  - `background_mode`: "default", "color", or "image"
  - `background_color`: Custom solid color with color picker in UI
  - Solid color passed to shaders via `iBackgroundColor` uniform
- **Resize Overlay**: Centered overlay during window resize showing dimensions
  - Displays both character (cols×rows) and pixel dimensions
  - Auto-hides 1 second after resize stops
- **Grid-Based Window Sizing**: Calculate initial window size from cols×rows
  - No visible resize on startup (like iTerm2)
  - "Use Current Size" button in settings to save current dimensions

#### Terminal Features
- **Bracketed Paste Mode Support**: Proper paste handling for shells that support it
  - Wraps pasted content with `ESC[200~`/`ESC[201~` sequences
  - Prevents accidental command execution when pasting text with newlines
  - Works with bash 4.4+, zsh, fish, and other modern shells
- **DECSCUSR Cursor Shape Support**: Dynamic cursor changes via escape sequences
  - Applications can change cursor style (block/underline/bar) and blink state
  - Respects user's `lock_cursor_style` and `lock_cursor_blink` settings
- **Multi-Character Grapheme Cluster Rendering**: Proper handling of complex Unicode
  - Flag emoji (🇺🇸) using regional indicator pairs
  - ZWJ sequences (👨‍👩‍👧‍👦) for family/profession emoji
  - Skin tone modifiers (👋🏽)
  - Combining characters (diacritics)
  - Requires par-term-emu-core-rust v0.22.0
- **Box Drawing Geometric Rendering**: Pixel-perfect TUI borders and block characters
  - Light/heavy horizontal and vertical lines (─ ━ │ ┃)
  - All corners, T-junctions, and crosses (┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼ etc.)
  - Double lines and corners (═ ║ ╔ ╗ ╚ ╝ etc.)
  - Rounded corners (╭ ╮ ╯ ╰)
  - Solid, partial, and quadrant block elements (█ ▄ ▀ ▐ ▌ etc.)
  - Eliminates gaps between adjacent cells

#### Tab Bar Enhancements
- **Tab Bar Color Configuration**: 11 new options for full color customization
  - Background, active/inactive/hover tab colors
  - Text colors, indicator colors, close button colors
  - Settings UI panel for live color editing
- **Per-Tab Custom Colors**: Right-click context menu to set individual tab colors
  - Color presets row with custom color picker
  - Color indicator dot on inactive tabs with custom colors
- **Tab Layout Improvements**:
  - Equal-width tabs that spread across available space
  - Horizontal scrolling with arrow buttons when tabs exceed minimum width
  - Configurable `tab_min_width` (default: 120px, range: 120-512px)
  - Tab borders with configurable width and color
  - Toggle for tab close button visibility
- **Inactive Tab Dimming**: Visual distinction for active tab
  - `dim_inactive_tabs` (default: true)
  - `inactive_tab_opacity` (default: 0.6)

#### Shader System
- **Cubemap Support**: Load 6-face cubemap textures for environment reflections
  - Auto-discovery of cubemap folders in settings UI dropdown
  - Standard naming convention: px/nx/py/ny/pz/nz
- **iTimeKeyPress Uniform**: Track when last key was pressed for typing effects
  - Enables screen pulses, typing animations, keystroke visualizations
  - Included keypress_pulse.glsl demo shader
- **use_background_as_channel0**: Option to use app's background image as iChannel0
  - Allows shaders to incorporate configured background image into effects
- **New Background Shaders**:
  - `rain.glsl`: Rain on glass post-processing effect
  - `singularity.glsl`: Whirling blackhole with red/blue accretion disk
  - `universe-within.glsl`: Mystical neural network with pulsing nodes
  - `convergence.glsl`: Swirling voronoi patterns with lightning bolt
  - `gyroid.glsl`: Raymarched gyroid tunnel with colorful lighting
  - `dodecagon-pattern.glsl`: BRDF metallic tile pattern
  - `arcane-portal.glsl`: Animated portal with swirling energy
  - `bumped_sinusoidal_warp.glsl`: Warped texture effect
- **Cursor Shader Overrides**: Per-shader settings for cursor effects
  - animation_speed, hides_cursor, disable_in_alt_screen

#### Window Transparency
- **Proper Window Transparency Support**: Correct alpha handling across platforms
  - Appropriate alpha mode selection based on surface capabilities
  - macOS window blur support via CGS private API
  - `transparency_affects_only_default_background` (default: true)
  - `keep_text_opaque` option to maintain text clarity
  - RLE background rendering to eliminate seams between cells

#### macOS Improvements
- **macOS Clipboard Shortcuts**: `Cmd+C` and `Cmd+V` support
- **Keyboard Shortcuts in Shader Editors**: Fixed `Cmd+A/C/V/X` in text editors

### Changed
- **Core Library Update**: Bumped `par-term-emu-core-rust` to v0.22.0 for grapheme cluster support
- **Default VSync Mode**: Changed to FIFO (most compatible across platforms)
- **Default Unfocused FPS**: Changed from 10 to 30 for better background responsiveness
- **Default Blur Radius**: Changed to 8 for better visual effect
- **Build Target**: `make build` now uses release mode; added `make build-debug` for debug builds
- **Shader Optimizations**:
  - Removed iChannel4 terminal blending dependencies from background shaders
  - Replaced pow(x, n) with multiplications
  - Precomputed constants and reduced loop iterations

### Fixed
- **Text Clarity with Shaders**: Use nearest filtering instead of linear for terminal texture
- **Shader Transparency Chaining**: Preserve transparency when both background and cursor shaders enabled
- **Double Opacity Bug**: Fixed background getting darker when cursor shader enabled with opacity < 100%
- **DPI Scaling**: Properly recalculate font metrics when moving between displays with different DPIs
- **Background Image Loading**: Fixed tilde expansion and uniform buffer layout
- **Cursor Settings**: Cursor style and blink changes now apply to running terminals
- **FPS Throttling**: Properly throttle when window unfocused with pause_refresh_on_blur
- **Selection Bug**: Modifier keys (Ctrl/Alt/Cmd) alone no longer clear text selection
- **Tab Bar Click-Through**: Tab close button clicks no longer leak to terminal
- **Alt Screen Rendering**: Fixed black screen when cursor shader disabled for alt screen apps
- **Animation Resume**: Respect user's animation settings when resuming from blur
- **Box Drawing Lines**: Adjusted thickness for cell aspect ratio consistency

### Refactored
- **Large File Extraction**: Decomposed monolithic files into focused modules
  - `config/` module directory with types.rs, defaults.rs
  - `font_manager/` with types.rs, loader.rs, fallbacks.rs
  - `settings_ui/` with shader_editor.rs, cursor_shader_editor.rs, shader_dialogs.rs
  - `custom_shader_renderer/` with pipeline.rs, cursor.rs
  - `cell_renderer/` with pipeline.rs
  - `window_state/` with tab_ops.rs, scroll_ops.rs, keyboard_handlers.rs
  - `mouse_events/` with text_selection.rs, url_hover.rs
  - `app/handler/` with notifications.rs
- **DRY Helpers**: RendererInitParams, ConfigChanges structs for cleaner code
- **GPU Utilities**: New gpu_utils.rs module with reusable sampler and texture helpers

### Documentation
- Added `docs/SHADERS.md` with complete list of 49 included shaders by category
- Updated `docs/CUSTOM_SHADERS.md` with all uniforms and configuration options
- Added code organization guidelines to CLAUDE.md

---

## [0.4.0] - 2026-01-23

### Added
- **Multi-Tab Support**: Multiple terminal tabs per window with independent PTY sessions
  - `Cmd/Ctrl+T` to create a new tab
  - `Cmd/Ctrl+W` to close tab (or window if single tab)
  - `Cmd/Ctrl+Shift+[/]` or `Ctrl+Tab/Shift+Tab` to switch tabs
  - `Cmd/Ctrl+1-9` for direct tab access
  - `Cmd/Ctrl+Shift+Left/Right` to reorder tabs
  - Tab duplication with inherited working directory
  - Visual tab bar with close buttons, activity indicators, and bell icons
  - Configurable tab bar visibility (always, when_multiple, never)
- **Multi-Window Support**: Spawn multiple independent terminal windows
  - `Cmd/Ctrl+N` to open a new terminal window
  - Each window runs its own shell process with separate scrollback and state
  - Application exits when the last window is closed
- **Native Menu Bar**: Cross-platform native menu support using the `muda` crate
  - macOS: Global application menu bar with standard macOS conventions
  - Windows/Linux: Per-window menu bar with GTK integration on Linux
  - Full keyboard accelerators for all menu items
  - Menu structure: File, Edit, View, Tab, Window (macOS), Help
- **Shader Texture Channels**: Shadertoy-compatible iChannel1-4 texture support
  - Load custom textures for use in GLSL shaders
  - Configure via `custom_shader_channel1` through `custom_shader_channel4` settings
  - Supports PNG, JPEG, and other common image formats
- **Shader Brightness Control**: New `custom_shader_brightness` setting
  - Dims shader background to improve text readability (0.05 = very dark, 1.0 = full)
- **Cursor Shader Improvements**: Enhanced cursor shader system
  - New `cursor_shader_hides_cursor` option to fully replace cursor rendering
  - Allows cursor shaders to completely control cursor appearance
- **Custom Shaders Collection**: 40+ included GLSL shaders in `shaders/` directory
  - Background effects: starfield, galaxy, underwater, CRT, bloom, clouds, happy_fractal, bumped_sinusoidal_warp
  - Cursor effects: glow, sweep, trail, warp, blaze, ripple, pacman, orbit

### Changed
- **Architecture Refactor**: Decomposed monolithic `AppState` into modular components
  - `TabManager`: Coordinates multiple tabs within each window
  - `WindowManager`: Coordinates multiple windows and handles menu events
  - `WindowState`: Per-window state (terminal, renderer, input, UI)
  - Event routing by `WindowId` and tab index for proper multi-window/tab support

### Documentation
- Added `docs/CUSTOM_SHADERS.md` - Comprehensive guide for installing and creating shaders
- Updated `docs/ARCHITECTURE.md` - Added TabManager and texture system details
- Updated README with multi-tab keyboard shortcuts and configuration

---

## [0.3.0] - 2026-01-21

### Added
- **Ghostty-Compatible Cursor Shaders**: Full support for cursor-based shader animations
  - `iCurrentCursor`, `iPreviousCursor` uniforms for cursor position and size
  - `iCurrentCursorColor` uniform for cursor color
  - `iTimeCursorChange` uniform for cursor movement timing
  - Built-in cursor shaders: sweep, warp, glow, blaze, trail, ripple, boom
- **Configurable Cursor Color**: New cursor color setting exposed to shaders
- **Cursor Style Toggle**: `Cmd+,` (macOS) / `Ctrl+,` to cycle through Block, Beam, and Underline styles
- **Geometric Cursor Rendering**: Proper visual rendering for all cursor styles

### Fixed
- **Login Shell Support**: Fixed issues with login shell initialization and environment loading

### Changed
- **Shader Editor Improvements**: Background and cursor shader editors now show filename in window header

---

## [0.2.0] - 2026-01-20

### Added
- **Intelligent Redraw Loop (Power Efficiency)**: Significantly reduced CPU/GPU usage by switching from continuous polling to event-driven rendering
  - Switched from `ControlFlow::Poll` to `ControlFlow::Wait`
  - Implemented smart wake-up logic for cursor blinking, smooth scrolling, and custom shader animations
  - Redraws are now requested only when content actually changes or animations are active
- **parking_lot Mutex Migration**: Migrated from `std::sync::Mutex` to `parking_lot::Mutex` for improved performance and robustness
  - Eliminated Mutex poisoning risk, preventing crash loops if a thread panics while holding a lock

### Fixed
- **Dropped User Input**: Fixed a critical logic error where key presses, paste operations, and middle-click paste could be silently discarded if the terminal lock was contested (e.g., during rendering). Replaced `try_lock()` with `.lock().await` for all critical input paths.
- **Audio Bell Panic**: Fixed a crash on startup on systems without audio devices. `AudioBell` now fails gracefully and returns a disabled state instead of panicking.

### Changed
- **Core Library Update**: Bumped `par-term-emu-core-rust` to v0.21.0 to leverage safe environment variable APIs and non-poisoning mutexes.

## [0.1.0] - 2025-11-24

### Fixed - Critical (2025-11-24)
- **macOS crash on startup (NSInvalidArgumentException)**: Fixed crash when calling `setDisplaySyncEnabled:` on wrong layer type
  - Added proper type checking using `objc2::runtime::AnyClass::name()` to verify layer is `CAMetalLayer`
  - Fixed class name retrieval to correctly detect layer type
  - Moved Metal layer configuration to AFTER renderer/surface creation (src/app.rs:264-270)
  - Application now starts successfully without crashing
  - Root cause: Was trying to call Metal-specific methods before wgpu created the Metal surface
  - Files: `src/macos_metal.rs:48-75`, `src/app.rs:264-270`

### Added - Configuration (2025-11-24)
- **max_fps configuration option** - Control target frame rate (matches WezTerm's naming)
  - Renamed `refresh_rate` to `max_fps` for clarity (backward compatible via alias)
  - Default: 60 FPS
  - Controls how frequently terminal requests screen redraws
  - Documentation includes macOS VSync throttling caveat
  - Files: `src/config.rs:98-104`, `src/app.rs:334`, `examples/config-complete.yaml:165-170`

### Known Limitations - Performance (2025-11-24)
- **macOS FPS throttling remains at ~22-25 FPS** despite CAMetalLayer configuration
  - Successfully configures `displaySyncEnabled = false` on CAMetalLayer
  - Verified setting is applied (logs confirm `displaySyncEnabled = false`)
  - However, FPS remains throttled at ~22-25 FPS with 40-53ms frame times
  - Root cause: Issue appears to be in wgpu's rendering pipeline, not just CAMetalLayer settings
  - wgpu may have additional VSync or frame pacing logic that can't be disabled via CAMetalLayer alone
  - Alternative approaches (WezTerm's native Cocoa, iTerm2's CVDisplayLink) bypass wgpu entirely
  - **Status**: Functional but FPS-limited. May require wgpu upstream changes or alternative rendering approach
  - Files: `src/macos_metal.rs` (new), `src/app.rs:264-270`, `src/cell_renderer.rs:107`, `src/lib.rs:13`, `src/main.rs:11`
  - Dependencies: Added `objc2`, `objc2-app-kit`, `objc2-foundation`, `objc2-quartz-core`, `raw-window-handle` for macOS

### Planned Features
- Clipboard history integration (pending core library API)
- Tmux control protocol support
- Color accessibility controls (contrast, brightness)
- Dynamic font hot-reloading
- Font subsetting for large CJK fonts
- Split pane support (horizontal/vertical)

---

## [0.2.1] - 2025-11-23 - Emoji Sequence Preservation

### Changed - Core Library Compatibility
- **Updated to par-term-emu-core-rust v0.10.0**
  - Cell struct now uses `grapheme: String` instead of `character: char` for full emoji sequence preservation
  - Supports variation selectors (⚠️ vs ⚠), skin tone modifiers (👋🏽), ZWJ sequences (👨‍👩‍👧‍👦), regional indicators (🇺🇸)
  - Cell no longer implements `Copy` trait, now `Clone` only (breaking change in rendering code)
  - Text shaping now receives complete grapheme clusters for proper emoji rendering
  - All character operations updated to extract base character from grapheme when needed
  - Changed from `copy_from_slice` to `clone_from_slice` for cell array operations

### Fixed - Emoji Rendering
- **Emoji sequences are now preserved** during text shaping instead of being broken into individual characters
- **Variation selector font selection**: Emoji with FE0F variation selector now force emoji font selection (fixes ⚠️ ❤️ rendering in color)
- **Texture filtering artifacts**: Changed from linear to nearest filtering to eliminate edge artifacts and bleeding between glyphs
- **Flag placeholder boxes**: Regional indicators no longer cache fallback boxes, only rendered via text shaping
- **Flag scaling**: Removed 1.5x scaling for flags, now same size as other emoji for visual consistency
- **Emoji modifier caching**: Variation selectors, skin tone modifiers, ZWJ, and regional indicators now skip individual glyph caching

---

## [0.2.0] - 2025-11-23 - Font Features & Hyperlinks

### Added - Font Features

#### Multiple Font Families
- **Styled font support**: Configure separate fonts for bold, italic, and bold-italic text
  - `font_family_bold`: Use professionally designed bold fonts instead of synthetic bold
  - `font_family_italic`: Use proper italic/oblique fonts
  - `font_family_bold_italic`: Use dedicated bold-italic variants
  - Smart fallback to primary font if styled fonts not configured
  - Font indexing system: 0=primary, 1=bold, 2=italic, 3=bold-italic, 4+=range fonts

#### Custom Font Ranges
- **Unicode range mapping**: Map specific fonts to Unicode character ranges
  - Configure fonts for specific codepoint ranges (e.g., 0x4E00-0x9FFF for CJK)
  - Perfect for CJK scripts (Chinese, Japanese, Korean)
  - Custom emoji fonts (Apple Color Emoji, Noto Color Emoji)
  - Mathematical symbols with specialized math fonts
  - Box drawing characters with monospace fonts
  - Font priority system: styled fonts → range fonts → fallback fonts → primary font
  - `FontRange` config structure with start/end codepoints

#### Optimized Glyph Caching
- **Compound cache keys**: Separate cache entries for each style combination
  - `GlyphCacheKey(character, bold, italic)` enables proper styled font rendering
  - Changed from `HashMap<char, GlyphInfo>` to `HashMap<GlyphCacheKey, GlyphInfo>`
  - Maintains O(1) lookup performance
  - Supports thousands of unique glyph combinations efficiently

### Added - Hyperlink Features

#### OSC 8 Hyperlink Support
- **Full OSC 8 protocol support**: Terminal hyperlinks work alongside regex detection
  - Added `hyperlink_id: Option<u32>` field to `Cell` struct
  - Cell conversion extracts `hyperlink_id` from terminal cell flags
  - `get_all_hyperlinks()`: Returns all hyperlinks from terminal
  - `get_hyperlink_url(id)`: Returns URL for specific hyperlink ID
  - `detect_osc8_hyperlinks()`: Extracts OSC 8 hyperlinks from cells
  - Combined detection: OSC 8 hyperlinks + regex URLs rendered together

### Added - Documentation

#### User Documentation
- **QUICK_START_FONTS.md**: 5-minute setup guide with step-by-step instructions
- **examples/README.md**: Comprehensive guide with Unicode reference table
- **examples/config-styled-fonts.yaml**: Bold/italic font configuration example
- **examples/config-font-ranges.yaml**: Unicode range mapping examples
- **examples/config-complete.yaml**: Complete feature showcase
- **test_fonts.sh**: Comprehensive test script with 12 test cases

#### Technical Documentation
- **IMPLEMENTATION_SUMMARY.md**: Complete technical reference
- **RELEASE_CHECKLIST.md**: Production readiness verification

### Changed

#### Core Structures
- **Cell struct**: Added `hyperlink_id: Option<u32>` field
- **FontManager**: Extended to manage styled fonts and range-specific fonts
- **GlyphCacheKey**: New compound key type for cache lookups
- **Config struct**: Added font configuration fields

#### Rendering Pipeline
- **CellRenderer**: Updated to use compound glyph cache keys
- **URL Detection**: Enhanced to combine OSC 8 and regex detection
- **Terminal Integration**: Added hyperlink accessor methods

### Fixed
- **Clippy warnings**: Fixed collapsible if statement
- **Formatting**: All code formatted with rustfmt
- **Font traits**: Added Clone/Debug implementations for FontData

### Performance
- Maintains O(1) glyph cache lookups
- Fonts loaded once, Arc-shared across glyphs
- Negligible overhead for range checks

### Testing
- All 33 tests pass (6 PTY tests ignored as expected)
- Zero compiler warnings
- Clippy clean
- Format verified

---

## [0.1.1] - Scrollbar & Clipboard Features

### Added
- **Visual Scrollbar**: GPU-accelerated scrollbar with custom WGSL shader
  - Auto-hide behavior when no scrollback content available
  - Smooth position tracking and visual feedback
  - Configurable scrollback size (default: 10,000 lines)
- **Scroll Navigation**: Multiple ways to navigate terminal history
  - Mouse wheel scrolling support
  - `PageUp`/`PageDown` for page-by-page navigation
  - `Shift+Home` to jump to top of scrollback
  - `Shift+End` to jump to bottom (current content)
- **Scrollback Rendering**: Properly displays history when scrolled up
  - Shows actual scrollback content instead of current content when scrolled
  - Combines scrollback buffer with current visible content
  - Calculates correct window of lines to display based on scroll position
- **Clipboard Integration**: Full cross-platform clipboard support
  - `Ctrl+V` to paste from clipboard
  - Middle-click paste (configurable via config)
  - Automatic line ending conversion for terminal compatibility
- **Text Selection**: Mouse-based text selection with clipboard integration
  - Click and drag to select text
  - Automatic copy to clipboard on mouse release
  - Support for single-line and multi-line selection
  - Works across scrollback buffer and current content
- **PTY Integration**: Real pseudo-terminal support
  - Automatic shell spawning on startup
  - Cross-platform shell detection (Unix/Windows)
  - PTY resize synchronization with window
  - Real-time terminal output updates at 60fps
- **Shell Exit Handling**: Graceful shutdown on shell exit
  - Exit detection with status message
  - "[Process completed - press any key to exit]" prompt
- **Styled Content Extraction**: Foundation for ANSI color rendering
  - Per-character color and attribute extraction
  - Support for bold, italic, underline attributes
- **Comprehensive Testing**: 23 tests covering core functionality

### Changed
- Improved terminal rendering to use real PTY content
- Enhanced error handling throughout the codebase
- Optimized redraw loop to 60fps

### Fixed
- Code formatting and linting issues
- Test assertions for grid padding behavior
- Module visibility for public API

---

## [0.1.0] - Initial Release

### Added
- Basic terminal window creation
- GPU-accelerated text rendering using wgpu and glyphon
- Cross-platform window management via winit
- Configuration file support (YAML)
- Font size and family configuration
- Window resizing with proper PTY synchronization
- VT sequence support via par-term-emu-core-rust
- Complete keyboard input handling
  - Special keys (arrows, function keys)
  - Modifier keys (Ctrl, Alt, Shift)
  - Control character sequences
- Cross-platform support (macOS, Linux, Windows)

---

## Notes

### Versioning
- **Major version (X.0.0)**: Breaking changes
- **Minor version (0.X.0)**: New features, backward compatible
- **Patch version (0.0.X)**: Bug fixes, minor improvements

### Links
- [GitHub Repository](https://github.com/paulrobello/par-term)
- [Core Library](https://github.com/paulrobello/par-term-emu-core-rust)

### References
- [OSC 8 Hyperlinks Spec](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
- [Unicode Character Ranges](https://en.wikipedia.org/wiki/Unicode_block)

---

[Unreleased]: https://github.com/paulrobello/par-term/compare/v0.30.0...HEAD
[0.30.0]: https://github.com/paulrobello/par-term/compare/v0.29.2...v0.30.0
[0.29.2]: https://github.com/paulrobello/par-term/compare/v0.29.1...v0.29.2
[0.29.1]: https://github.com/paulrobello/par-term/compare/v0.29.0...v0.29.1
[0.29.0]: https://github.com/paulrobello/par-term/compare/v0.28.0...v0.29.0
[0.28.0]: https://github.com/paulrobello/par-term/compare/v0.27.0...v0.28.0
[0.27.0]: https://github.com/paulrobello/par-term/compare/v0.26.0...v0.27.0
[0.26.0]: https://github.com/paulrobello/par-term/compare/v0.25.0...v0.26.0
[0.25.0]: https://github.com/paulrobello/par-term/compare/v0.24.0...v0.25.0
[0.24.0]: https://github.com/paulrobello/par-term/compare/v0.23.0...v0.24.0
[0.23.0]: https://github.com/paulrobello/par-term/compare/v0.22.0...v0.23.0
[0.22.0]: https://github.com/paulrobello/par-term/compare/v0.21.0...v0.22.0
[0.21.0]: https://github.com/paulrobello/par-term/compare/v0.20.0...v0.21.0
[0.20.0]: https://github.com/paulrobello/par-term/compare/v0.19.0...v0.20.0
[0.19.0]: https://github.com/paulrobello/par-term/compare/v0.18.0...v0.19.0
[0.18.0]: https://github.com/paulrobello/par-term/compare/v0.17.1...v0.18.0
[0.17.1]: https://github.com/paulrobello/par-term/compare/v0.17.0...v0.17.1
[0.17.0]: https://github.com/paulrobello/par-term/compare/v0.16.0...v0.17.0
[0.16.0]: https://github.com/paulrobello/par-term/compare/v0.15.0...v0.16.0
[0.15.0]: https://github.com/paulrobello/par-term/compare/v0.14.0...v0.15.0
[0.14.0]: https://github.com/paulrobello/par-term/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/paulrobello/par-term/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/paulrobello/par-term/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/paulrobello/par-term/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/paulrobello/par-term/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/paulrobello/par-term/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/paulrobello/par-term/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/paulrobello/par-term/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/paulrobello/par-term/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/paulrobello/par-term/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/paulrobello/par-term/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/paulrobello/par-term/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/paulrobello/par-term/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/paulrobello/par-term/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/paulrobello/par-term/releases/tag/v0.1.0
