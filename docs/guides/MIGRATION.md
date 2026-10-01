# Migration Guide

Upgrade notes for par-term covering breaking configuration changes, renamed fields, and behavior shifts between significant version groups. Check the relevant section before upgrading from an older release.

## Table of Contents

- [Unreleased — Leader Key](#unreleased--leader-key)
- [Unreleased — Profile Shortcuts and the Open Profiles Launcher](#unreleased--profile-shortcuts-and-the-open-profiles-launcher)
- [Unreleased — Session Picker, Window Actions, and Detach](#unreleased--session-picker-window-actions-and-detach)
- [Unreleased — Pane Dimming, Splits, and Restart](#unreleased--pane-dimming-splits-and-restart)
- [Unreleased — Default Shortcuts Aligned with iTerm2](#unreleased--default-shortcuts-aligned-with-iterm2)
- [Unreleased — Close-Safety Defaults Aligned with iTerm2](#unreleased--close-safety-defaults-aligned-with-iterm2)
- [Unreleased — Renamed Labels, Stable Tab Numbers, and Tab Behavior](#unreleased--renamed-labels-stable-tab-numbers-and-tab-behavior)
- [v0.45.0 — Core 0.48 and Kitty Placement Geometry (Library Consumers)](#v0450--core-048-and-kitty-placement-geometry-library-consumers)
- [v0.43.0 — MSRV 1.98 and wgpu 30 for Library Consumers](#v0430--msrv-198-and-wgpu-30-for-library-consumers)
- [v0.39.0 — MSRV 1.97 and the `mermaid` Feature Removed](#v0390--msrv-197-and-the-mermaid-feature-removed)
- [v0.38.0 — Upgrading Requires a Manual Download](#v0380--upgrading-requires-a-manual-download)
- [v0.38.0 — Preference Import Requires HTTPS](#v0380--preference-import-requires-https)
- [v0.38.0 — Profile Commands Require Confirmation](#v0380--profile-commands-require-confirmation)
- [v0.38.0 — `Cmd/Ctrl+Shift+P` Moves to the Profile Drawer](#v0380--cmdctrlshiftp-moves-to-the-profile-drawer)
- [v0.38.0 — `XDG_CONFIG_HOME` Is Now Honoured](#v0380--xdg_config_home-is-now-honoured)
- [v0.38.0 — macOS Config Directory Consolidation](#v0380--macos-config-directory-consolidation)
- [v0.31.0 — Content Prettifier Removed](#v0310--content-prettifier-removed)
- [v0.27.0 — Trigger Field Renamed](#v0270--trigger-field-renamed)
- [v0.27.0 — Security-Gated Trigger Execution](#v0270--security-gated-trigger-execution)
- [v0.26.0 — ACP auto_approve Enforces Safe Write Paths](#v0260--acp-auto_approve-enforces-safe-write-paths)
- [v0.25.0 — HTTP Profile URLs Blocked by Default](#v0250--http-profile-urls-blocked-by-default)
- [v0.25.0 — Minimum Contrast Scale Change](#v0250--minimum-contrast-scale-change)
- [v0.25.0 — Pane Padding Defaults](#v0250--pane-padding-defaults)
- [v0.20.0 — Default Changes](#v0200--default-changes)
- [Related Documentation](#related-documentation)

---

## Unreleased — Leader Key

- **`Cmd + B` (macOS) and `Ctrl + Shift + B` (Linux/Windows) are the leader key.** The leader is checked before keybindings, so a binding of yours on that chord stops running. Rebind it, or set `leader_key` to another chord (empty turns the leader off). See [Leader Key](../features/LEADER_KEY.md).
- **The background-shader toggle moved to `Ctrl + Alt + B` on Linux and Windows.** A saved config that binds `toggle_background_shader` to `Ctrl + Shift + B` is moved to `Ctrl + Alt + B` on load when that chord is free; otherwise it is left alone and the leader takes the key. macOS keeps `Cmd + Shift + B`.
- **The tmux prefix arms the leader in a tmux gateway tab.** The keys tmux's prefix table knows still go to tmux. The ones that open par-term overlays (`:`, `?`, `w`, `s`, `q`, `,`) now open par-term's palette, help, tree picker, session picker, pane letters, and tab rename instead of tmux's prompt or choose-tree. Tab navigation (`n`, `p`, `l`, `1`–`9`) switches par-term's tab, as the tab chords do, instead of tmux's current window, which par-term did not follow. In a local tab beside a gateway, the tmux prefix now reaches the shell.

## Unreleased — Profile Shortcuts and the Open Profiles Launcher

- **Profile shortcuts move into your keybindings.** On first launch, each profile's **Keyboard Shortcut** (`keyboard_shortcut` in `profiles.yaml`) becomes a keybinding to `open_profile:<id>` with the same chord, saved to `config.yaml`, and the profile field is cleared. Nothing to do unless the log reports a shortcut that was not migrated: one that does not parse, or whose chord another binding already holds, stays on the profile and does nothing. Bind it in Settings › Input › Keybindings instead. Child profiles no longer inherit a parent's shortcut.
- **The tab bar's "New Tab" profile window is gone.** The chevron next to `+`, and the new-tab shortcut with `new_tab_shortcut_shows_profiles: true`, open the Open Profiles launcher instead. The window's **Assistant Panel** entry is not in the launcher; use the assistant keybinding or the View menu.
- **`toggle_profile_drawer` opens Open Profiles.** The id is unchanged, so existing bindings keep working, but the chord now opens the launcher popup. The docked drawer is `toggle_profiles_panel`, or its button on the window edge.
- **Automatic profile switching runs on every tab and can be undone.** A background tab that connects to a matching host switches too; each switch shows a toast with **Undo**. Pin a tab (tab context menu) to keep it from switching.
>
---

## Unreleased — Session Picker, Window Actions, and Detach

- **`close_window` closes the whole window.** A keybinding bound to `close_window` used to close the active tab when the window had several and the window only on its last tab. It now closes the window with all its tabs, asking first when the window holds more than one tab. The previous behavior is `close_tab_or_window`; rebind to that id to keep it.
- **`detach` is the detach action.** `mux-detach` keeps working as an alias; configs that use it need no change.

---

## Unreleased — Pane Dimming, Splits, and Restart

- **Inactive panes darken instead of fading.** With `dim_inactive_panes: true`, unfocused panes now scale their colors toward black and keep text solid (`inactive_pane_dim_mode: darken`, the new default). `inactive_pane_opacity` sets the brightness in that mode. The previous look, where the whole pane including its text turned transparent over a background image or shader, is `inactive_pane_dim_mode: fade`.
- **Splits of a profile tab run the profile's program.** Splitting a tab opened from a profile with an SSH host, command, or shell starts that program in the new pane instead of the default shell. Set `split_inherits_profile: false` to keep the old behavior.
- **Restart Pane reruns the pane's program.** A profile tab's first pane and a split started with a command restart that program, not the default shell. The `shell_exit_action` restart options follow the same rule.

---

## Unreleased — Default Shortcuts Aligned with iTerm2

The default keybindings follow iTerm2 on macOS, and the same letters through the `Ctrl+Shift` / `Ctrl+Alt` family on Linux and Windows. See [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) for the full table.

**Existing configs keep their chords.** When a config loads, a default is added only if its action has no binding *and* its chord is not already bound to another action. A chord your saved config already binds keeps its old action. You get a new default only where the chord and the action were both free. To take a new default, delete (or rebind) the saved row that holds its chord in **Settings ▸ Keys ▸ Key Bindings** or in the `keybindings:` list of `config.yaml`; the default is merged in on the next load.

**Renamed actions.** `split_horizontal` is now `split_down` and `split_vertical` is now `split_right`, named for where the new pane goes. Saved bindings are migrated in place on load with the chord unchanged, and the old ids still dispatch if they appear anywhere else.

**Moved chords, macOS:**

| Action | Before | Now |
|---|---|---|
| Split right / split down | `Cmd+Shift+D` / `Cmd+D` | `Cmd+D` / `Cmd+Shift+D` |
| Close | `Cmd+W` closed the tab | `Cmd+W` closes the pane, then the tab, then the window |
| Close pane | `Cmd+Shift+W` | `Cmd+W` |
| Close tab | smart close on `Cmd+W` | `Cmd+Opt+W` |
| Resize pane | `Cmd+Opt+Shift+Arrow` | `Cmd+Ctrl+Arrow` |
| Swap pane | `Cmd+Ctrl+Opt+Arrow` | `Cmd+Opt+Shift+Arrow` |
| Move tab | `Cmd+Shift+Left/Right` | `Cmd+Opt+Shift+[ / ]` (arrows kept) |
| Reopen closed tab | `Cmd+Z` | `Cmd+Shift+T` (`Cmd+Z` kept) |
| Throughput mode | `Cmd+Shift+T` | command palette |
| Command history | `Cmd+R` | `Cmd+Shift+;` (`Cmd+R` goes to the shell) |
| tmux session picker | `Cmd+Opt+T` | `Cmd+Ctrl+S` |
| Command palette | none | `Cmd+Shift+P` |
| Profile drawer | `Cmd+Shift+P` | `Cmd+O` |
| Command marks | `Cmd+Up/Down` | `Cmd+Shift+Up/Down` (`Cmd+Up/Down` kept) |
| Fullscreen | `F11` | `F11` or `Cmd+Ctrl+F` |

**Moved chords, Linux and Windows:**

| Action | Before | Now |
|---|---|---|
| Split down | `Ctrl+Shift+D` | `Ctrl+Shift+O` (`Ctrl+Shift+D` kept) |
| Close pane | `Ctrl+Shift+X` | `Ctrl+Shift+W` (`Ctrl+Shift+X` kept), cascading to tab and window |
| Close tab | `Ctrl+Shift+W` | `Ctrl+Alt+W` |
| Swap pane | `Alt+Shift+Arrow` | unbound |
| Command palette | none | `Ctrl+Shift+P` |
| Profile drawer | `Ctrl+Shift+P` | unbound (Profiles menu, palette) |
| tmux session picker | `Ctrl+Alt+T` | `Ctrl+Alt+T` or `Ctrl+Alt+S` |

---

## Unreleased — Close-Safety Defaults Aligned with iTerm2

Window-close behavior changes for configs that do not set these keys (saved configs that set them keep their values):

- `prompt_on_quit` now defaults to `true` (was `false`).
- New `confirm_close_multiple_tabs` (default `true`): closing a window that holds **more than one** tab asks for confirmation. With it on, `prompt_on_quit` no longer fires for single-tab windows — a single-tab close is silent and undoable.
- `session_undo_preserve_shell` now defaults to `true` (was `false`): for 5 s after a tab closes (`session_undo_timeout_secs`), Reopen Closed Tab restores the tab **with its running processes** instead of spawning a fresh shell.

To restore the old behavior, set:

```yaml
prompt_on_quit: false
confirm_close_multiple_tabs: false
session_undo_preserve_shell: false
```

`confirm_close_running_jobs` (still `false`) and attached par-mux tab close dialogs are unchanged.

---

## Unreleased — Renamed Labels, Stable Tab Numbers, and Tab Behavior

"Session" now means only a par-mux or tmux session in every label and doc. No config key or action id changed; only the words did:

| Old label | New label |
|-----------|-----------|
| Restore previous session on startup | Restore windows on launch |
| Confirm before quitting with open sessions | Confirm before quitting with open tabs |
| Undo close tab timeout | Reopen closed tab for |
| Preserve shell session on close | Keep the shell running while a closed tab can be reopened |
| Session Logging (Settings section), Toggle Session Logging (action) | Output Recording, Toggle Output Recording |
| Session Ended: (desktop notification title) | Shell Exited: |
| There are N active sessions / All sessions will be terminated (quit dialog) | There are N open tabs / Their shells and running programs will be closed |

Tab behavior changes:

- Default **Tab N** titles keep their number. Closing or moving another tab no longer renumbers them, and a new tab takes the lowest free number.
- Keyboard **move tab left/right** stops at the ends instead of wrapping around.
- The `close_tab` action closes the window when it runs on the last tab, the same as the menu's Close Tab.
- New window numbers (`show_window_number`) take the lowest free number, so two windows never show the same number after one closes.

---

## v0.45.0 — Core 0.48 and Kitty Placement Geometry (Library Consumers)

This release affects only projects that depend on par-term's crates as libraries. If you use the released binaries, there is nothing to do — no configuration fields changed.

**`par-term-emu-core-rust` moves 0.46 → 0.48.** The core now parses and exposes full Kitty placement geometry: an order-independent `x`/`y`/`w`/`h` source crop, `X`/`Y` destination pixel offsets, and a `c`/`r` cell footprint where an omitted axis is derived from the image's aspect ratio per axis. Semantics you may observe as a consumer:

- Retransmitting an existing image id deletes its previous placements first (replace, not stack).
- Delete resolution is order-independent within a single command (key order inside one APC, including `d=` target components), while separate commands take effect in stream order.
- A crop that resolves to zero area yields a zero-size placement instead of falling back to the full image.
- APC commands are processed in stream order, with interleaved cursor moves honored at each APC completion.

**`par-term-render` moves 0.10 → 0.11** to honor that geometry at draw time: UV-crop mapping, cell-footprint sizing, destination pixel offsets, and signed-pixel scroll clipping. Virtual placements (`U=1`) render unchanged.

---

## v0.43.0 — MSRV 1.98 and wgpu 30 for Library Consumers

Both changes affect only projects that depend on par-term's crates as libraries. If you use the released binaries, there is nothing to do.

**Minimum supported Rust version is now 1.98** (was 1.97). As with the v0.39.0 bump, cargo's MSRV-aware resolver will select an older version of these crates rather than failing your build — you silently stop receiving updates instead. Run `rustup update` to stay current.

**`par-term-render` 0.10 sits on wgpu 30.** The crate's public API exposes wgpu types (`Surface`, `Queue`, `TextureFormat`, …), and wgpu 29 and 30 cannot be linked into one binary, so a project using `par-term-render` must depend on wgpu 30 itself:

```toml
# before
par-term-render = "0.9"
wgpu = "29"

# after
par-term-render = "0.10"
wgpu = "30"
```

No par-term APIs were renamed — only the underlying wgpu types moved. Custom WGSL/GLSL shaders needed no changes in par-term itself, and the same held for its test suite.

---

## v0.39.0 — MSRV 1.97 and the `mermaid` Feature Removed

Both changes affect only projects that depend on par-term's crates as libraries. If you use the released binaries, there is nothing to do.

**Minimum supported Rust version is now 1.97** (was 1.95). Cargo's MSRV-aware resolver will select an older version of these crates rather than failing your build, so an outdated toolchain degrades quietly instead of breaking — but you will silently stop receiving updates. Run `rustup update` to stay current.

**The `mermaid` cargo feature no longer exists.** Remove it from any `features = [...]` list or `--features` flag; leaving it in place is a hard error from cargo, not a warning.

```toml
# before
par-term = { version = "0.38", features = ["mermaid"] }

# after
par-term = { version = "0.39" }
```

Nothing is lost. The feature's two dependencies had been unreferenced since v0.31.0 removed the content prettifier that used them, so enabling it only added `resvg`'s SVG rasterization stack to your build — 18 crates — without changing behaviour.

---

## v0.38.0 — Upgrading Requires a Manual Download

**Check for Updates will not install 0.38.0.** Download it from the [releases page](https://github.com/paulrobello/par-term/releases) instead, this once.

**Why:** two independent gates both refuse. Releases before 0.38.0 published no per-binary `.sha256`, so the checksum gate that has always guarded self-update hard-failed on every one of them. And a 0.37.1 or earlier build has no release-signing public key compiled in, so it cannot verify 0.38.0's new `.minisig` signatures regardless of what the release publishes.

**After this upgrade:** self-update works normally. 0.38.0 is the first release to publish both the per-binary checksums and the signatures, and it is the first build that carries the key needed to verify them.

> **Note:** rotating the release signing key in future would have the same one-time effect — a build pins the key it shipped with, so it cannot verify a release signed by a newer one.

---

## v0.38.0 — Preference Import Requires HTTPS

**Settings ▸ Advanced ▸ Import/Export Preferences** now rejects any import URL that is not `https://`. There is no HTTP opt-in on this path, and `file:`, `ftp:` and `data:` URLs are rejected as well.

**Affected users:** anyone whose preference-import URL uses `http://`. The Fetch buttons stay enabled and the status line reports why the URL was rejected.

**Migration:** serve the configuration over HTTPS, or download it and use **Import from File** instead.

Note that this path also used to abort par-term outright on any `https://` URL — the HTTP agent selected a TLS provider that is not compiled in, and the library panics rather than returning an error in that case. That is fixed in the same release, so HTTPS import works for the first time here.

---

## v0.38.0 — Profile Commands Require Confirmation

A profile that specifies a **Command** now shows a confirmation dialog before that command runs, on every auto-switch path — working directory, tmux session name, and remote hostname.

**Why:** profile matching is driven by remote-controlled input. The hostname comes from an OSC 7 sequence the remote shell emits, it is checked on every event-loop iteration, and a `*` pattern always matches. The trigger subsystem already gates the same capability behind an allowlist, a denylist, a rate limit, a concurrency cap, an audit log, and confirmation on by default; profile auto-switch had none of it.

**Affected users:** anyone relying on a profile command running unattended on auto-switch.

**Migration:** none available — there is deliberately no opt-out. Confirm the prompt when it appears. A profile fetched from a remote URL additionally cannot pre-approve its own command under any setting.

---

## v0.38.0 — `Cmd/Ctrl+Shift+P` Moves to the Profile Drawer

`Cmd+Shift+P` (macOS) / `Ctrl+Shift+P` (Windows and Linux) now toggles the profile drawer on all three platforms. **Manage Profiles…** keeps its entry in the Profiles menu but no longer carries an accelerator.

**Why:** the settings table always advertised this chord for the profile drawer, but the Profiles menu registered it for **Manage Profiles…**, and the native menu bar consumes a chord before it reaches the application — so on macOS and Windows the advertised binding opened the manager instead. On Linux, which had no native menu bar, the chord already toggled the drawer, so the defect was inverted per platform and could not be fixed by moving the drawer to a different chord.

**Affected users:** macOS and Windows users who used this chord to open **Manage Profiles…**.

**Migration:** reach **Manage Profiles…** from the Profiles menu, from **Settings ▸ Profiles**, or from the profile drawer's **Manage** button. If you prefer the old behavior, bind `toggle_profile_drawer` to a different chord in your keybindings and the menu accelerator will not conflict.

---

## v0.38.0 — `XDG_CONFIG_HOME` Is Now Honoured

par-term previously hardcoded `~/.config/par-term/` on Linux and macOS and never read `XDG_CONFIG_HOME`, even though the documentation claimed otherwise. It now resolves its config directory to `$XDG_CONFIG_HOME/par-term/`, falling back to `~/.config/par-term/` when the variable is unset, empty, or not an absolute path.

**Affected users:** only those who set `XDG_CONFIG_HOME` to something other than `~/.config` — typically dotfile-managed Linux setups. Previously par-term silently ignored the variable and read `~/.config/par-term/config.yaml`; now it reads the XDG location. Everyone else sees no change.

**No manual steps required.** On the first launch after upgrading, par-term moves an existing `~/.config/par-term/` into the new location, without overwriting anything already present there, and logs a summary. If an entry cannot be moved (for example a cross-device rename) it is left in place with a warning.

Only `XDG_CONFIG_HOME` is read. `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME` and `XDG_RUNTIME_DIR` are not consulted — par-term keeps all per-user data under the one config directory. Windows is unaffected and continues to use `%APPDATA%\par-term`.

---

## v0.38.0 — macOS Config Directory Consolidation

On macOS, par-term now keeps all of its per-user data under `~/.config/par-term/` — the same directory as `config.yaml` — instead of the previous mixed layout where some files landed in `~/Library/Application Support/par-term/`.

Affected items: `profiles.yaml`, `command_history.yaml`, `arrangements.yaml`, `last_session.yaml`, the `cache/dynamic_profiles/` directory, the `sounds/` directory, and the `agents/` directory.

**No manual steps required.** On the first launch after upgrading, par-term automatically moves any legacy entries into `~/.config/par-term/` (without overwriting files already present) and removes the now-empty legacy directory; a summary is written to the debug log. Subsequent launches do nothing. If an entry fails to move (for example a cross-device rename), it is left in place with a warning — move it by hand if you need it. Linux and Windows are unaffected; the two locations already coincided there.

---

## v0.31.0 — Content Prettifier Removed

The content prettifier feature has been removed. The `par-term-prettifier` workspace crate and all related runtime wiring have been deleted, including the settings UI tab, config/profile fields, trigger action (`type: prettify`), keybinding action (`toggle_prettifier`), and render-path substitutions.

The prettifier was an optional subsystem that reformatted terminal output (JSON, Markdown, tables) using built-in formatters or user-configured external commands. It was removed to reduce maintenance surface area; equivalent formatting can be achieved through shell aliases, pipe-through formatters (e.g., `jq`, `bat`), or the trigger system.

**Migration steps:**

1. Remove `enable_prettifier`, `content_prettifier`, and any per-profile prettifier overrides from `config.yaml`.
2. Remove `toggle_prettifier` from keybindings.
3. Remove any triggers with `type: prettify`.
4. If you relied on external prettifier commands, consider adding them as trigger `RunCommand` actions instead.

---

## v0.27.0 — Trigger Field Renamed

The `require_user_action` field on trigger definitions was renamed to `prompt_before_run`.

```yaml
# Before v0.27.0
triggers:
  - name: "my trigger"
    require_user_action: false

# v0.27.0 and later
triggers:
  - name: "my trigger"
    prompt_before_run: false
    i_accept_the_risk: true   # required when prompt_before_run is false
```

The old field name is accepted as a YAML alias — existing config files continue to load without modification. However, the Settings UI only shows `prompt_before_run`. Update your config to avoid confusion.

---

## v0.27.0 — Security-Gated Trigger Execution

Triggers with `prompt_before_run: false` now **require** an explicit `i_accept_the_risk: true` field. Without it, execution is blocked and an audit warning is emitted.

If your existing config has `require_user_action: false` (or the new `prompt_before_run: false`) on any trigger, add `i_accept_the_risk: true` to that trigger to restore automatic execution:

```yaml
triggers:
  - name: "auto-run trigger"
    prompt_before_run: false
    i_accept_the_risk: true
    pattern: "some pattern"
    action: ...
```

A warning banner appears in Settings → Automation → Triggers when any trigger has this configuration.

---

## v0.26.0 — ACP auto_approve Enforces Safe Write Paths

The ACP agent's automatic approval mode for file-write tools now always validates that the target path passes `is_safe_write_path`. The target must fall within the user's home directory or an explicitly declared safe root. Writes to system paths are blocked even in `auto_approve` mode.

---

## v0.25.0 — HTTP Profile URLs Blocked by Default

Profiles fetched from remote URLs must use HTTPS. HTTP URLs are rejected at fetch time with a warning, unless the global `allow_http_profiles` setting (default `false`) explicitly opts in to plaintext. Update any profile `url` fields to use `https://`.

---

## v0.25.0 — Minimum Contrast Scale Change

`minimum_contrast` changed from the WCAG ratio scale (1.0–21.0) to an iTerm2-compatible perceived-brightness scale (0.0–1.0).

| Old value | Meaning | New equivalent |
|-----------|---------|----------------|
| `1.0` | Disabled | `0.0` (disabled; auto-migrated on load) |
| `4.5` | WCAG AA | approximately `0.3` |
| `7.0` | WCAG AAA | approximately `0.5` |

A saved value of `1.0` is automatically migrated to `0.0` (disabled) on load. All other values are not auto-migrated — review your setting after upgrading.

The slider in Settings → Appearance → Theme is capped at `0.99`; values of `1.0` are treated as disabled.

---

## v0.25.0 — Pane Padding Defaults

Default padding values changed:

| Field | Old default | New default |
|-------|-------------|-------------|
| `pane_padding` | `4.0` px | `1.0` px |
| `window_padding` | `0.0` px | `1.0` px |

Split-pane mode now automatically adds base padding equal to half the divider width, so `pane_padding` of `0.0` is no longer needed to remove the inter-pane gap.

---

## v0.20.0 — Default Changes

**`tab_bar_mode` default changed from `when_multiple` to `always`.**

If you were relying on the tab bar auto-hiding when only one tab was open, add this to your config explicitly:

```yaml
tab_bar_mode: "when_multiple"
```

**`window_padding` default changed to `0.0`.**

If you preferred the previous padded look, restore it:

```yaml
window_padding: 4.0
```

---

## Related Documentation

- [Config Reference](../CONFIG_REFERENCE.md) — complete field reference with types and defaults
- [Automation](../features/AUTOMATION.md) — trigger configuration and `prompt_before_run` / `i_accept_the_risk` usage
- [Assistant Panel](../ASSISTANT_PANEL.md) — ACP agent configuration and `auto_approve` permissions
- [Changelog](../../CHANGELOG.md) — full release history
