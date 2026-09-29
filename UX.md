# par-term UX Plan: Windows, Tabs, Panes, and par-mux

Research date: 2026-09-28, against `main` at `f3f8fd00`. This is a research and planning document. Nothing in it is implemented yet.

## How to read this document

The plan answers two questions the owner asked: how do we make par-term **easier to use** for pane and par-mux power users, and how do we make it **easier to reason about**. The order follows that goal:

1. [Executive summary](#1-executive-summary)
2. [Mental model and terminology](#2-mental-model-and-terminology) (the "reason about it" half)
3. [Proposed keymap as a system](#3-proposed-keymap-as-a-system), including a par-term leader key
4. [New actions](#4-new-actions)
5. [par-mux safety fixes](#5-par-mux-safety-fixes)
6. [Unifying local and par-mux behavior](#6-unifying-local-and-par-mux-behavior)
7. [Visibility and discoverability](#7-visibility-and-discoverability)
8. [Pane behavior changes](#8-pane-behavior-changes)
9. [Tab and window behavior changes](#9-tab-and-window-behavior-changes)
10. [Decisions for the owner](#10-decisions-for-the-owner)
11. [Phased roadmap with acceptance criteria](#11-phased-roadmap-with-acceptance-criteria)
- [Appendix A: bugs found during research](#appendix-a-bugs-found-during-research)
- [Appendix B: documentation drift](#appendix-b-documentation-drift)
- [Appendix C: current keybinding inventory](#appendix-c-current-keybinding-inventory)
- [Appendix D: upstream daemon gaps](#appendix-d-upstream-daemon-gaps-par-term-emu-core-rust)
- [Appendix E: items to confirm at runtime](#appendix-e-items-to-confirm-at-runtime)

**Reference codes** are stable for discussion: `T` terminology, `K` keymap, `L` leader key table, `A` new actions, `M` par-mux safety, `U` local/mux unification, `V` visibility and discoverability, `PN` pane behavior, `TW` tab/window behavior, `D` decisions, `P` phases, `B` bugs, `DOC` doc drift, `UP` upstream daemon work, `RT` runtime checks.

**Evidence tags.** `[verified]` means the claim was read in source (file:line given). `[inferred]` means the code path strongly implies it but nobody ran the app; each inferred item also appears in Appendix E. The six claims the plan leans on hardest were re-read by hand before writing: B1, B2, B3, B4, B5, M1.

---

## 1. Executive summary

par-term has most of the raw capability a pane power user wants, but it is spread across four keyboard dispatch layers, three menus, a command palette with no default chord, and a Settings grid that can't show or edit about half of it. par-mux mode layers a second model on top (daemon sessions, windows, panes) that reuses tmux plumbing and tmux wording, and several of its paths lose work without warning.

The five highest-value changes, in order:

1. **Stop par-mux from destroying work silently (M1–M4).** Closing the last tab of an attached window sends `kill-window`, and the daemon then deletes the whole session (`src/app/tab_ops/lifecycle.rs:291-302`; `par-term-emu-core-rust/src/mux/tree.rs:1050-1073`). There is no running-job check, Cmd+Z restores an unrelated tab, and closing a tab's last pane leaves an invisible daemon window.
2. **One vocabulary (T1–T6).** "Session" currently means five things. Fix the words and most of the "hard to reason about" complaint goes with it.
3. **A par-term leader key that works in every mode (K4, L-table).** tmux prefix plumbing exists but only works for the tmux gateway, not par-mux (`src/app/tmux_handler/gateway_input.rs:469-563`, gated by `is_tmux_connected()` at `gateway.rs:212-217`). A native leader with a which-key overlay makes every window/tab/pane/session operation reachable and discoverable from one place.
4. **Fill the missing pane and session actions (A1–A20).** No pane zoom, no next/last pane, no equalize, no last-used tab, no window switcher, no rename-tab action, no par-mux session picker or new-session command.
5. **Make state visible (V1–V12).** Session name, mux vs local tab, broadcast, zoom, agents needing attention, and dead panes have no persistent indicator today.

The plan is split into six phases (Section 11). Phase P0 is small, safe bug fixes; P1 is par-mux safety; P2 is the keymap and leader; P3 new pane actions; P4 session management; P5 polish.

---

## 2. Mental model and terminology

### 2.1 The problem

| Word | What it means today | Where |
|---|---|---|
| Session | A par-mux daemon session | profile field `mux_session_name`, MUX.md |
| Session | A tmux session | tmux picker "tmux Sessions" |
| Session | A tab (the quit dialog says "N active sessions") | `src/quit_confirmation_ui.rs:91-93` |
| Session | The saved set of windows ("session restore") | SESSION_MANAGEMENT.md |
| Session | The closed-tab undo stack ("session undo") | `session_undo_*` config keys |
| Session | The terminal's status-bar variables (`session.*`) | STATUS_BAR.md:161-185 |
| Window | An OS window | everywhere |
| Window | A par-mux / tmux daemon window, which par-term shows as a **tab** | MUX.md, daemon protocol |
| Window | All windows ("Window Arrangement") | ARRANGEMENTS.md |
| Close Window | Closes **one tab** (smart close) | `src/app/menu_actions.rs:27-36` |
| Horizontal split | Panes stacked top/bottom in code, but "left/right" in Getting Started | `src/pane/types/common.rs:20-25` vs `docs/guides/GETTING_STARTED.md:239-240` |

A user who attaches to par-mux sees a tab bar that the daemon calls windows, a window that the daemon calls a client, and toasts that say "tmux" (`src/app/tmux_handler/session.rs:223`, `flow_control.rs:38-97`).

### 2.2 Proposed vocabulary

Adopt one word per concept in every user-visible string, doc, and action label. Code identifiers can migrate later.

- **T1. Window** = an OS window. Nothing else.
- **T2. Tab** = a tab in a window's tab bar. In par-mux mode a tab **is** a daemon window; the UI always says "tab" and the docs carry one bridging table (T6).
- **T3. Pane** = a split region inside a tab.
- **T4. Session** = **only** a par-mux or tmux session (a named, persistent collection of tabs that outlives par-term). Rename the other uses:
  - "Session restore" → **"Restore windows on launch"** (config key keeps its name, label changes).
  - "Session undo" → **"Reopen closed tab"** settings.
  - Quit dialog "N active sessions" → **"N tabs in M windows"**, and state which will be detached rather than killed (M9).
  - Status bar `session.*` variables → keep as aliases, add `pane.*` and document them as "the focused pane".
  - "Session logging" → **"Output recording"** (label only).
- **T5. Attached vs local.** A tab is either **attached** (backed by a par-mux session, survives quit) or **local** (dies with par-term). One adjective, used in tooltips, badges, the tab context menu, and toasts.
- **T6. Bridging table** in MUX.md and in the session picker header:

  | par-term says | par-mux / tmux says |
  |---|---|
  | Window (attached) | Client |
  | Tab | Window |
  | Pane | Pane |
  | Session | Session |

- **T7. Split direction names** describe **where the new pane goes**, never the divider: **Split Right** and **Split Down** (plus Split Left / Split Up, A5). `split_vertical` and `split_horizontal` stay as deprecated aliases so existing configs load. This matches snippets/triggers, which already describe placement (`par-term-config/src/automation.rs:156-167`).
- **T8.** Replace all tmux wording in par-mux paths: "tmux: Session ended" → "par-mux: daemon connection lost"; "tmux: Output paused" → "Output paused"; notification title "tmux Error" → "par-mux error" when the transport is par-mux; default tab title "tmux @N" → the daemon window name or cwd (M11).

### 2.3 One-sentence model to put at the top of MUX.md and the F1 help panel

> A **window** holds **tabs**; a tab holds **panes**. A window can be **attached** to one par-mux **session**, in which case its tabs live in the daemon and survive quitting par-term. **Detach** leaves them running; **Kill** ends them.

---

## 3. Proposed keymap as a system

### 3.1 Principles

- **K1. One modifier family per scope, same shape on every platform.** Let `Mod` = Cmd on macOS and Ctrl+Shift on Linux/Windows; `PaneMod` = Cmd+Opt on macOS and Ctrl+Alt on Linux/Windows.
  - Window and tab: `Mod+key`.
  - Pane: `PaneMod+key`.
  - **Shift moves things.** `PaneMod+Arrow` focuses, `PaneMod+Shift+Arrow` resizes (kept for muscle memory). Tab move is already `Mod+Shift+Left/Right`.
  - Rare or destructive operations live on the leader (K4) and the palette, not on four-modifier chords.
- **K2. One source of truth for chords.** Today there are four dispatch sources: native menu accelerators, modal modes, the registry, and hardcoded `KEY_LAYERS` (`src/app/input_events/key_handler/claims/mod.rs:262-297`, `key_handler/mod.rs:57-106`). Hardcoded chords cannot be freed or passed through to the terminal. Move every hardcoded chord into the registry as a default, have native menus read their accelerators from the registry, and add an explicit `unbind` / `passthrough` value so users can give a chord back to the shell (fixes B19–B23 class issues). This is the prerequisite for everything else in Section 3.
- **K3. Never steal keys a shell or TUI needs by default.** Candidates to move off hardcoded status (make them registry defaults, so users can clear them): Linux `Ctrl+_` and `Ctrl+=` font size (breaks readline undo, `utility.rs:108-109`), `Alt+1..9` (readline `digit-argument`, irssi/weechat), `Ctrl+Tab` on macOS, `Ctrl+,`. Plain `Ctrl+L` interception (`utility.rs:79-85`) should respect the keyboard encoding mode.
- **K4. A par-term leader key** that works identically for local tabs, par-mux, and tmux gateway windows. Pressing the leader arms a key table (L-table below) and shows a which-key overlay after ~400 ms listing the available next keys with their current bindings. The existing tmux prefix becomes an alias for the leader when a tmux gateway is connected, and its table (`par-term-tmux/src/prefix.rs:161-224`) is merged so tmux muscle memory carries over.
- **K5. Avoid Cmd+Ctrl chords as defaults** until the Settings recorder can record them (B18). Avoid `Ctrl+Alt+Arrow`-only defaults on Linux where GNOME uses them for workspaces; provide the leader path as the guaranteed fallback.
- **K6. Every chord shown anywhere is the live chord.** Palette, context menus, tab badges, toasts, F1 help, and Settings hints all read the registry, not static tables (B8, B9, B22).

### 3.2 Leader key

- **K7. Default leader:** macOS `Cmd+B` (free today; Cmd chords never reach the shell, and it echoes tmux `C-b`). Linux/Windows `Ctrl+Shift+B`, which requires moving `toggle_background_shader` to `Ctrl+Alt+B` there (D3). Configurable as `leader_key`; `leader_timeout_ms` default 2000; `leader_overlay_delay_ms` default 400.
- **K8.** Leader then the leader chord again sends the literal chord to the pane.
- **K9.** Keys marked *repeat* stay armed (tmux `-r` behavior) until a non-repeat key, Escape, or the timeout.
- **K9a.** Vim-style `h j k l` / `H J K L` for focus and swap are opt-in via `leader_vim_keys: true`, because the default table keeps tmux's meanings for `l` (last tab) and `L` (last session). Enabling it moves last tab to `Tab` and last session to `S`. Every letter in the default table below is unique.

**L-table (proposed).** Keys mirror tmux wherever tmux has a convention.

| Key | Action | Notes |
|---|---|---|
| `c` | New tab | par-mux: `new-window -c <cwd>` |
| `n` / `p` | Next / previous tab | |
| `l` | Last-used tab (A10) | tmux `last-window` |
| `0`–`9` | Go to tab N | |
| `,` | Rename tab (A12) | tmux `rename-window` |
| `&` | Close tab | par-mux: shows M1 dialog when last tab |
| `w` | Tree picker: windows → tabs → panes (+ sessions when attached) (A15) | tmux `choose-tree` |
| `s` | Session picker (A16) | par-mux + tmux in one list |
| `$` | Rename session | needs UP1 |
| `(` / `)` | Previous / next session | par-mux: re-attach this window |
| `L` | Last session | |
| `d` | Detach (M-series) | par-mux / tmux only |
| `%` or `\|` | Split Right | |
| `"` or `-` | Split Down | |
| Arrows | Focus pane in direction | *repeat* |
| `Shift`+Arrows | Swap with pane in direction | *repeat* |
| `o` | Next pane (A7) | *repeat* |
| `;` | Last-focused pane (A8) | |
| `q` | Show pane letters (existing `select_pane_hint`) | tmux `display-panes` |
| `z` | Toggle pane zoom (A1) | |
| `E` | Equalize panes (A3) | tmux `select-layout -E` |
| `Space` | Cycle layout presets (A4) | |
| `r` | Resize mode: arrows resize, *repeat*, Escape exits (A6) | |
| `x` | Close pane | |
| `!` | Move pane to new tab (existing promote) | tmux `break-pane` |
| `@` | Merge tab into this one as a pane (existing demote) | tmux `join-pane` |
| `R` | Restart pane process (A9) | |
| `b` | Toggle broadcast input | |
| `a` | Jump to next agent needing attention (A17) | |
| `N` | New window | |
| `P` | Profile drawer | frees `Cmd+Shift+P` (K10) |
| `:` | Command palette | |
| `?` | Full key cheat sheet (live bindings) | |

### 3.3 Direct chords (changes and additions only)

`Mod` and `PaneMod` as defined in K1. Every "new" chord below was checked against the claim inventory in Appendix C; items marked RT need a runtime check.

**Migration rule (prerequisite for every row below).** Default changes do not reach existing users safely today: `merge_default_keybindings` re-adds any default whose action id is missing from the user's config (`par-term-config/src/config/keybindings_methods.rs:25-35`), and a chord bound twice resolves by later-wins or HashMap order (B20, B23). An upgrading user with `Cmd+D: split_horizontal` would silently gain a `split_right` default on the same chord. Before shipping K10–K22: (1) a new or moved default is added only if its chord is unclaimed in the user's config, (2) renamed actions (`split_horizontal` → `split_down`) are migrated in place rather than added alongside, and (3) the registry rejects duplicate chords at load with a visible warning.

K5 governs new chords; the existing macOS swap default (Cmd+Ctrl+Opt+Arrow) is kept for continuity but becomes recordable once B18 is fixed.

| Code | Action | macOS | Linux / Windows | Change |
|---|---|---|---|---|
| K10 | Command palette | Cmd+Shift+P | Ctrl+Shift+P | New default (VS Code convention). Profile drawer moves to leader `P` and keeps its menu item (D2). Also remove the hardcoded drawer duplicate (`src/app/input_events/key_handler/keyboard_handlers.rs:172-195`) or it keeps intercepting. |
| K11 | Split Right | Cmd+D | Ctrl+Shift+E | macOS flips to match iTerm2 (D1). Linux already matches Terminator. |
| K12 | Split Down | Cmd+Shift+D | Ctrl+Shift+O | Linux adds Terminator's `O`; keep Ctrl+Shift+D as an alias (D1). |
| K13 | Zoom pane | Cmd+Shift+Enter | Ctrl+Shift+Enter | New (iTerm2). RT1: confirm the hardcoded Shift+Enter handler (`key_handler/mod.rs:489-542`) does not swallow it. |
| K14 | Next / previous pane | Cmd+] / Cmd+[ | Ctrl+Alt+] / Ctrl+Alt+[ | New (iTerm2). Depends on B26 bracket fix. |
| K15 | Equalize panes | Cmd+Opt+= | Ctrl+Alt+= | New. Registry runs before the loose font-size layer, so it wins. |
| K16 | Swap pane | Cmd+Ctrl+Opt+Arrow | *unbound*; leader `Shift+Arrow` | Drop Linux `Alt+Shift+Arrow`: editors use it and Windows uses Alt+Shift for input-language switching. |
| K17 | Next agent needing attention | Cmd+Opt+A | Ctrl+Alt+A | New. RT2: check OS claims. |
| K18 | Session picker (par-mux + tmux) | Cmd+Opt+T | Ctrl+Alt+T, plus Ctrl+Alt+S | Reuses the existing tmux picker chord for the unified picker. Ubuntu uses Ctrl+Alt+T to open a terminal, so add the S alternative. |
| K19 | Go to last tab | Cmd+9 | Alt+9 | Change: 9 means last tab (browser convention). |
| K20 | New window | Cmd+N (menu) | Ctrl+Shift+N | Linux gets a real registry default; the in-app menu already shows this label but nothing handles it. |
| K21 | Next / previous window | Cmd+` / Cmd+Shift+` | leader only | New actions (A13). RT3: confirm winit delivers Cmd+`. |
| K22 | Reopen closed tab | Cmd+Z (keep) | Ctrl+Shift+Z (keep) | Optional D4: also Cmd+Shift+T (browser convention) by moving throughput mode off it. |

Everything else keeps its current default.

### 3.3a iTerm2 alignment (owner decision 2026-09-28: "wherever possible align with iTerm2")

Source of truth: iTerm2's `sources/MainMenu/Base.lproj/MainMenu.xib` and `sources/Settings/iTermPreferences.m` in `~/Repos/iTerm2`. This table **supersedes K11–K22 on macOS** where they differ. Linux/Windows keep the K1 family (Ctrl+Shift for window/tab, Ctrl+Alt for pane) applied to the same letters, since iTerm2 is macOS-only.

| Code | Action | iTerm2 (macOS) | par-term today | Change |
|---|---|---|---|---|
| I1 | Split side by side (Split Right) | Cmd+D | Cmd+Shift+D | Swap (D1) |
| I2 | Split stacked (Split Down) | Cmd+Shift+D | Cmd+D | Swap (D1) |
| I3 | Maximize active pane (zoom) | Cmd+Shift+Enter | none | New (A1) |
| I4 | Next / previous pane | Cmd+] / Cmd+[ | none | New (A7) |
| I5 | Select pane in direction | Cmd+Opt+Arrow | Cmd+Opt+Arrow | Already aligned |
| I6 | Move divider (resize) | Cmd+Ctrl+Arrow | Cmd+Opt+Shift+Arrow | Move to Cmd+Ctrl+Arrow. Cmd+Opt+Shift+Arrow becomes swap (I7). Requires B18 recorder fix. |
| I7 | Swap pane | none in iTerm2 | Cmd+Ctrl+Opt+Arrow | Move to Cmd+Opt+Shift+Arrow (frees the Rectangle-style chord) |
| I8 | Next / previous tab | Cmd+Shift+] / [ | same | Aligned |
| I9 | Move tab left / right | Cmd+Shift+Opt+[ / ] | Cmd+Shift+Left/Right | Change to iTerm2 chords; keep arrows as alias |
| I10 | Tab 1–8, Cmd+9 = last tab | Cmd+1..9 | Cmd+9 = 9th tab | Cmd+9 = last tab (K19) |
| I11 | Window by number | Cmd+Opt+1..9 | none | New (A13) |
| I12 | New tab / new window | Cmd+T / Cmd+N | same | Aligned |
| I13 | New tab / window with current profile | Cmd+Opt+Shift+T / Cmd+Opt+Shift+N | none | New (TW11); Cmd+Opt+T moves session picker (I24) |
| I14 | New tab next to current | Cmd+Opt+T | config `new_tab_position` | New action |
| I15 | Close (pane, else tab, else window) | Cmd+W | Cmd+W closes tab; Cmd+Shift+W closes pane | Cmd+W closes the focused **pane**, cascading to tab and window (iTerm2 semantics). `close_tab` becomes Cmd+Opt+W ("Close All Panes in Tab"). |
| I16 | Close window | Cmd+Shift+W | none (menu smart close) | Real close-window (A14) |
| I17 | Undo close | Cmd+Shift+T (D4) | Cmd+Z | Cmd+Shift+T; throughput mode moves to palette only. Keep Cmd+Z as alias. |
| I18 | Broadcast to all panes in tab | Cmd+Opt+I | Cmd+Opt+I | Aligned (and becomes per-tab, D8) |
| I19 | Broadcast to all panes in all tabs | Cmd+Shift+I | none | New mode; Cmd+Shift+I is free on macOS (assistant is Cmd+I) |
| I20 | Send input to current pane only | Cmd+Opt+Shift+I | none | New (turns broadcast off) |
| I21 | Toggle broadcast for current pane | Cmd+Ctrl+Opt+I | none | New (per-pane opt-out, V5) |
| I22 | Edit session / rename | Cmd+I (Edit Session) | Cmd+I = assistant panel | **Not aligned**: Cmd+I stays the assistant. Rename tab via leader `,` and double-click. |
| I23 | Detach (tmux) | Cmd+Ctrl+Shift+D | none | Detach from par-mux/tmux; leader `d` too |
| I24 | Session picker | none (iTerm2 uses tmux dashboard) | Cmd+Opt+T | Move to Cmd+Ctrl+S (leader `s` as the main path) so Cmd+Opt+T can be I14 |
| I25 | New tmux window / tab | Cmd+Ctrl+Shift+N / Cmd+Ctrl+Shift+T | none | In an attached window, Cmd+T already creates a daemon window; add these as explicit aliases |
| I26 | Find / find next / find previous | Cmd+F / Cmd+G / Cmd+Shift+G | Cmd+F only | Add Cmd+G / Cmd+Shift+G |
| I27 | Open Quickly (fuzzy switcher) | Cmd+Shift+O | none | Bind the tree picker (A15) here |
| I28 | Command history | Cmd+Shift+; | Cmd+R | Change to Cmd+Shift+; and give Cmd+R back to the shell (readline reverse-search) |
| I29 | Paste history | Cmd+Shift+H | Cmd+Shift+H | Aligned |
| I30 | Copy mode | Cmd+Shift+C | Cmd+Shift+C | Aligned |
| I31 | Clear buffer / clear scrollback | Cmd+K / Cmd+Shift+K | Cmd+Shift+K clears scrollback | Add Cmd+K clear buffer |
| I32 | Full screen | Cmd+Ctrl+F | F11 | Add Cmd+Ctrl+F (macOS F11 is Show Desktop); keep F11 |
| I33 | Previous / next mark | Cmd+Shift+Up / Down | Super+Up/Down | Change to Cmd+Shift+Up/Down |
| I34 | Save window arrangement | Cmd+Shift+S | none (Cmd+Shift+S = SSH quick connect) | Save arrangement takes Cmd+Shift+S (prompting for a name inline, TW13). SSH quick connect moves to Cmd+Ctrl+Shift+S plus the palette (Cmd+Ctrl+S is the session picker, I24; Cmd+Opt+S is iTerm2's "save current window as arrangement"). |
| I35 | Command palette | none in iTerm2 | none | Cmd+Shift+P (D2, K10). Profile drawer, which iTerm2 calls Open Profiles, moves to Cmd+O (I36). |
| I36 | Open profiles | Cmd+O | Cmd+Shift+P (drawer) | Drawer moves to Cmd+O |
| I37 | Restart pane process | menu item, no chord | none | A9, leader `R` |
| I38 | Move pane to tab / window / split | menu items, no chord | promote/demote, no chord | Rename to iTerm2 wording: "Move Pane to New Tab", "Move Pane to Window", "Move Pane to Split" |

Notes:
- **I15 is the most visible change.** It matches iTerm2 and tmux mental models (close the thing you are in). The last-attached-tab dialog (M1) and running-job confirmation (M2, D6) apply before the cascade.
- **Chord reclaims** (I17, I28, I34, I36) go through the migration rule above, so users who already rebound those chords keep their bindings.
- **Close prompts, iTerm2 defaults** (D6): iTerm2 ships `ConfirmClosingMultipleTabs = YES`, `PromptOnQuit = YES`, profile "prompt before closing" = never, undo-close timeout 5 s (`iTermPreferences.m:708-709`, `iTermProfilePreferences.m:1217-1218`). Aligned defaults are listed under D6.

### 3.4 Settings keybinding editor

- **K23.** One grid listing **every** action, including prefix actions (`mux-detach`, `restore_arrangement:<name>`, `snippet:`, `action:`), grouped by Window / Tab / Pane / Session / Agents / View.
- **K24.** A filter box that matches action names, labels, and chords.
- **K25.** Live conflict detection across registry, menu accelerators, the leader table, and known OS reservations, with normalized comparison (`Cmd+D` equals `CmdOrCtrl+D` on macOS). Today `check_keybinding_conflict` does exact string compares and is only called from the Actions and Snippets tabs (`par-term-settings-ui/src/settings_ui/sections.rs:124-152`).
- **K26.** Fix the recorder: record Cmd+Ctrl on macOS, Super elsewhere, and punctuation keys (B18, B19).
- **K27.** "Reset to default" and "Unbind (pass to terminal)" per row; unbinding persists across restarts (B20).
- **K28.** Show the leader table as its own editable section.

---

## 4. New actions

Every new action is: registered in `ACTION_HANDLERS`, listed in `AVAILABLE_ACTIONS` (so Settings and the palette show it), reachable from the leader, and mux-aware. "Daemon" says whether par-mux needs daemon work (see Appendix D).

| Code | Action id | What it does | Default | Daemon |
|---|---|---|---|---|
| A1 | `toggle_pane_zoom` | Focused pane fills the tab; others keep running. Indicator on tab and pane (V6). Any split/close/focus-move unzooms first. | K13, leader `z` | Needs UP3 for true zoom; client-side zoom works as an interim |
| A2 | `split_right`, `split_down` | Rename of existing split actions (T7) | K11, K12 | Supported |
| A3 | `equalize_panes` | Reset every split ratio in the tab so leaves get equal space | K15, leader `E`, double-click divider | Supported (`resize-pane` per pane) |
| A4 | `cycle_layout`, `layout:<name>` | Presets: even-horizontal, even-vertical, main-left, main-top, tiled | leader `Space` | Supported (resize-pane); rearranging needs swap |
| A5 | `split_left`, `split_up` | New pane placed before the focused one | leader only | Needs `split-window -b` (UP6) |
| A6 | `enter_resize_mode` | Modal: arrows resize by `pane_resize_step` (new config, default 5%), Shift+arrows by 1 cell, Escape/Enter exit | leader `r` | Supported |
| A7 | `next_pane`, `prev_pane` | Cycle panes in tree order | K14, leader `o` | Local focus + `select-pane` |
| A8 | `last_pane` | Toggle to previously focused pane | leader `;` | Same |
| A9 | `restart_pane` | Respawn the focused pane's process in place, same cwd and command | leader `R` | Needs UP5 |
| A10 | `last_tab` | Toggle to the previously active tab (MRU) | leader `l` | Local + `select-window` |
| A11 | `go_to_last_tab` | Rightmost tab | K19 | Local |
| A12 | `rename_tab` | Opens inline rename on the active tab (today only the context menu can) | leader `,`, double-click tab | Supported (`rename-window`) |
| A13 | `next_window`, `prev_window` | Cycle OS windows in creation order | K21 | n/a |
| A14 | `close_window` (real) | Close the whole window with all tabs. Current smart-close keeps its behavior under a new id `close_tab_or_window` | none (menu + palette) | Attached window: detach (M-series) |
| A15 | `toggle_tree_picker` | Fuzzy tree of windows → tabs → panes (+ attached session) with live previews of titles, cwd, agent state; Enter focuses, `x` closes, `r` renames | leader `w` | Supported (list-windows / list-panes) |
| A16 | `toggle_session_picker` | Unified par-mux + tmux session list: attach here, attach in new window, new session, rename, kill, detach | K18, leader `s` | list/new supported; rename/kill need UP1/UP2 |
| A17 | `focus_next_attention_agent` | Jump to next roster agent that is blocked, then done-unseen, cycling | K17, leader `a` | Supported (roster cache) |
| A18 | `move_tab_to_window:<n>` and `move_tab_to_window_picker` | Keyboard version of the context submenu | palette | Blocked for attached tabs (explain why) |
| A19 | `split_with_profile:<name>` and a picker | Split using a profile's command/SSH/cwd | palette | Supported (`split-window` + command) |
| A20 | `toggle_always_on_top` | Today config-only | palette | n/a |
| A21 | `close_other_tabs`, `close_tabs_to_right` | Browser-style bulk close, with one confirmation | tab context menu, palette | Attached: kill-window each, with M1 rules |
| A22 | `attach_mux_session:<name>`, `new_mux_session` | Attach without a profile | palette, session picker | Supported |
| A23 | `detach` | Public, documented id for the existing `mux-detach` (keep the old id as alias) | leader `d` | Supported |

---

## 5. par-mux safety fixes

These rank above all keymap polish because they lose work. All are `[verified]` unless tagged.

- **M1. Closing the last tab ends the session with no warning.** The tab close sends `kill-window -t @N` (`src/app/tab_ops/lifecycle.rs:291-302`), and the daemon's `kill_window` deletes a session whose window list becomes empty (`par-term-emu-core-rust/src/mux/tree.rs:1050-1073`). The last-*pane* path was deliberately made safe (`src/app/tab_ops/pane_ops.rs:185-192`); the last-*tab* path was not.
  **Fix:** closing the last attached tab opens a dialog: "This is the last tab of session *work*. **Detach** (keep it running) / **End session** / Cancel". Default button Detach. Remember-my-choice checkbox writes `mux_last_tab_close: ask|detach|kill`.
- **M2. No running-job confirmation for attached tabs or panes.** The job check reads `tab.terminal`, which in an attached tab is a hidden local login shell, not the daemon pane (`src/app/tab_ops/tab_helpers.rs:98-121`; `src/tab/mod.rs:250-256`). The close-pane path returns before confirmation (`pane_ops.rs:193-208`).
  **Fix:** ask the daemon for each pane's foreground command (`list-panes -F #{pane_current_command}` or equivalent; UP7 if unsupported) and run the same `jobs_to_ignore` logic.
- **M3. Cmd+Z after closing an attached tab restores an unrelated tab.** `kill-window` returns before any undo entry is pushed (`lifecycle.rs:297-302`), so reopen pops an older entry. After a last-pane close the tab reopens as local shells `[inferred]`.
  **Fix:** push a typed undo entry for attached tabs. If the daemon window was hidden (M4) the undo re-shows it; if it was killed, reopen shows "That tab was a par-mux window and was ended; it can't be reopened" instead of popping another entry.
- **M4. Closing a tab's last pane leaves an invisible daemon window.** The tab is dropped but the window stays in the daemon and in `tmux_pane_owners`; later layout pushes find a dead tab and stop (`src/app/tmux_handler/notifications/layout.rs:90-91, 199-201`). Its agents stay in the roster and selecting them fails with a log line only (`keybinding_actions.rs:663-669`). The "window survives" toast is immediately replaced by an undo prompt (`lifecycle.rs:376-400, 426-443`).
  **Fix:** make this state explicit as a **hidden tab**. The tree picker (A15) and session chip (V1) show "N hidden"; selecting one re-shows it; Cmd+Z re-shows it; roster rows show "(hidden)" and selecting them re-shows the tab. Tab close offers "Hide (keep running)" as the non-destructive alternative to Kill.
- **M5. Opening a par-mux profile silently does nothing when the window is already attached or a tmux gateway is active** (`src/app/tab_ops/profile_ops.rs:60-66`). The "already attached" toast MUX.md promises is only reachable mid-attach.
  **Fix:** offer "Open session *X* in a new window?" with one keypress.
- **M6. A failed par-mux split creates a stray local pane inside the attached tab** (`src/app/tmux_handler/notifications/mux.rs:236-240` → `pane_ops.rs:145-154`). **Fix:** fail closed with an error surface (V11).
- **M7. Keyboard focus and resize never reach the daemon.** `navigate_pane` and pane-hint select change local focus only (`pane_ops.rs:327-335`; `src/app/pane_hint_select.rs:116-124`); only mouse paths call `set_tmux_focused_pane_from_native`. Keyboard resize adjusts the local ratio only (`pane_ops.rs:361-383`); `sync_pane_resize_to_tmux` runs only on divider drags. par-term never sends `select-window` on tab switch.
  **Fix:** send `select-pane`, `select-window`, and `resize-pane` from the keyboard paths, and clear the roster's done-unseen mark on keyboard focus. Consequences today: other clients and CLI agents targeting "active" drift `[inferred]`, keyboard resizes likely snap back on the next `%layout-change` (RT4).
- **M8. Local-only operations run unguarded on attached tabs.** Duplicate tab, promote pane to tab, and demote tab to pane have no transport guard (`tab_helpers.rs:17-81`; `src/app/tab_ops/pane_transfer.rs:37, 154`). Promote/demote likely break the moved pane's daemon link `[inferred]` (RT5).
  **Fix:** Duplicate on an attached tab = `new-window -c <cwd>`. Promote/demote disabled for attached tabs with a tooltip until UP4 lands.
- **M9. The quit dialog says "All sessions will be terminated"** (`src/quit_confirmation_ui.rs:91-93`), false for par-mux. It also only fires on the title-bar close, not menu Quit (B32). **Fix:** "3 local tabs will close. Session *work* (5 tabs) will keep running."
- **M10. Reattach replays only the visible screen**, not scrollback (`par-term-mux/src/resync.rs:165-170`), although the daemon supports `capture-pane`. **Fix:** fetch scrollback up to `scrollback_lines` on attach, lazily per tab.
- **M11. Unstable tab names.** New attached tabs are titled "tmux @N" (`notifications/window.rs:47`); the daemon names new windows "0" (`par-term-emu-core-rust/src/mux/dispatch.rs:698`). **Fix:** send `new-window -n <name>` using the same title rules as local tabs; show the daemon name, not the tmux id.
- **M12. All par-mux errors are 2-second toasts**, including the multi-line stale-daemon instruction (`src/app/input_events/keybinding_helpers.rs:13-16`; `mux.rs:638`). **Fix:** persistent error state on the session chip (V1) with the full message and an action button (for example "Restart daemon").
- **M13. Roster jump goes to the wrong tab.** `focus_agent_roster_pane` passes a 0-based position over all tabs to `switch_to_tab_index`, which is 1-based over visible tabs (`src/app/tmux_handler/gateway.rs:302-310`; `src/tab/manager_nav.rs:65-77`). Selecting an agent in the first tab does nothing; any other tab lands one to the left, then focuses a pane id in the wrong tab. (Listed as B5 too.)
- **M15. par-mux attach is profile-only.** The only entry is a profile's `mux_session_name` (`par-term-config/src/profile_types/profile.rs:100-104`); there is no CLI flag (`src/cli/mod.rs:32-72`), no global auto-attach setting (tmux has `tmux_auto_attach`), and no palette or menu attach. **Fix:** `par-term --attach <session>`, A22, the session picker, and an optional `mux_auto_attach: <session>` config.
- **M16. Agent launch and crash triage land in an arbitrary pane.** In attached windows they `split-window -h` next to *some* mux pane, possibly in a background tab (`src/app/tmux_handler/notifications/mux.rs:427-492`; `agent_launch.rs:49-57`). **Fix:** split next to the focused pane of the active tab, or open a new tab when the user chooses.
- **M14. Daemon death, hung daemon, and max-tabs overflow are easy to miss.** Windows beyond `max_tabs` are dropped with a debug log (`notifications/window.rs:15-25`). **Fix:** persistent chip state (V1) and a toast naming the dropped count.

---

## 6. Unifying local and par-mux behavior

Target: **the same key does the same thing** whether a tab is local or attached. Where the daemon can't do it yet, the UI says so in place, never silently.

| Code | Operation | Local today | Attached today | Target (both) |
|---|---|---|---|---|
| U1 | Swap pane | Swaps whole subtrees in 3+ pane layouts (B1) | True two-pane swap in daemon | Leaf-level swap everywhere |
| U2 | Keyboard focus | Local | Local only, daemon not told (M7) | Local + `select-pane` |
| U3 | Keyboard resize | Grows/shrinks focused pane regardless of arrow axis (PN3) | Local only, likely reverted (M7) | Arrow moves the nearest divider in that direction; synced |
| U4 | Tab switch | Local | No `select-window` (M7) | Synced |
| U5 | Close last tab | Closes window | Ends session silently (M1) | Attached: dialog, default Detach |
| U6 | Close tab, undo | Undo stack | No undo, wrong entry popped (M3) | Typed undo; hidden tabs re-show |
| U7 | Close last pane of a tab | Closes tab | Invisible daemon window (M4) | Attached: tab becomes hidden, visible in picker |
| U8 | Running-job confirm | Optional | Checks hidden local shell (M2) | Checks the real pane |
| U9 | Duplicate tab | Clone cwd | Local tab inside attached window (M8) | `new-window -c` |
| U10 | Promote / demote | Works | Unguarded, likely breaks (M8) | Disabled with reason until UP4 |
| U11 | Reorder tabs | Local | Local only, resets on reattach `[inferred]` (RT6) | Persist via UP8; until then show "order is local" hint |
| U12 | Blank tab rename | Reverts to auto title | Daemon keeps old name | Sends an explicit unset/auto rename |
| U13 | Clear scrollback | Clears pane | Clears hidden local shell `[inferred]` (RT7) | Clears the mirror and sends `clear-history` |
| U14 | Split on transport error | n/a | Stray local pane (M6) | Error, no split |
| U15 | New tab past `max_tabs` | Log only | Log only; daemon windows dropped silently | Toast in both (B28 class) |
| U16 | Shell exit handling | Per `shell_exit_action` | Skipped (`shell_exit.rs:40, 140`) | Dead attached panes show the same dead-pane state (V9) and offer restart once UP5 exists |
| U17 | Prefix / leader | tmux gateway only | None | Leader everywhere (K4) |
| U18 | Mux tab indicator | n/a | Glyph drawn on **every** tab in an attached window, including local ones (`src/app/render_pipeline/egui_submit.rs:389`, `src/tab_bar_ui/tab_painter.rs:164-210`) | Per-tab attached badge (V2) |

---

## 7. Visibility and discoverability

- **V1. Session chip.** At the left of the tab bar (or top of the vertical bar): `⧉ work ▾` when attached, nothing when fully local. Shows health (connected, reconnecting, hung, stale daemon, error) and "N hidden". Click opens the session picker (A16). Also available as a status-bar widget, and as `session.mux_name` / `session.mux_state` variables. Today the name appears only in the OS title, which OSC titles can overwrite `[inferred]` (RT8; `gather_phases.rs:150-161`).
- **V2. Per-tab attached badge** decided by the tab, not the window (fixes U18). Tooltip: "Attached to par-mux session *work* — survives quit".
- **V3. Agent badges on tabs and pane title bars.** Blocked = amber dot, done-unseen = green dot, working = subtle spinner. Today agent state appears only in an off-by-default status widget and palette rows (`par-term-config/src/status_bar.rs:309-313`). Turn the roster widget on by default when attached.
- **V4. Fix the dead activity dot** (B2) so background tabs signal output.
- **V5. Broadcast indicator.** While broadcast is on: colored border on every receiving pane, badge on the tab, and a status-bar item. Scope the flag per tab rather than per window, add per-pane opt-out, and broadcast pastes too (today toast-only, window-wide, keystrokes only; `src/app/window_state/mod.rs:245`, `keybinding_actions.rs:295-312`).
- **V6. Zoom indicator** on tab ("⤢") and pane title.
- **V7. Pane numbers.** Leader `q` shows letters (existing hint mode). Add an option to show a small pane index in pane title bars, and expand hint letters past 26 (two-letter hints) since `max_panes` allows 32 or unlimited (`pane_hint_select.rs:20-23`).
- **V8. Which-key overlay** for the leader (K4) and a live `?` cheat sheet.
- **V9. Dead-pane state.** With `shell_exit_action: keep`, a dead pane gets a dimmed overlay: "Process exited (code N). Enter: restart · Cmd+Shift+W: close" (today no cue; `window_state_impl/shell_exit.rs:20-22`).
- **V10. Menus show pane and session operations.**
  - Native menu bar gains a **Pane** menu (Split Right/Down, Close, Zoom, Equalize, Next/Previous, Swap submenu, Move to New Tab, Rename, Broadcast) and a **Session** menu (Attach…, New Session…, Detach, Switch…, Rename, End) visible when par-mux is available.
  - The macOS **Window** menu lists open windows (gives standard window cycling).
  - Right-click in a pane body opens a pane context menu when the app isn't capturing the mouse (today there is none; `src/app/mouse_events/mouse_button.rs:244-264`).
  - All context menus show chords (K6) and drop the duplicate separator (B29).
- **V11. Persistent error surface** for par-mux (M12) and for silent refusals (max panes, max tabs, blocked moves).
- **V12. F1 help panel** gains Window / Tab / Pane / Session sections generated from live bindings, and drops the wrong rows (DOC7).
- **V13. Tab bar affordances.** Tooltip with full title on truncated tabs; auto-scroll the active tab into view on switch (today only arrow buttons and the wheel move it; `tab_bar_ui/horizontal.rs:85-94`); honor `tab_show_index` or remove it (B3); new-tab dropdown anchored to its button (`profile_menu.rs:39-42`).

---

## 8. Pane behavior changes

- **PN1. Swap exchanges only the two panes** (B1, U1). `PaneNode::swap_panes` swaps the `first` and `second` subtrees at the lowest split separating the two ids (`src/pane/types/pane_node.rs:275-293`), so in a 2×2 grid swapping top-left with top-right swaps whole columns. The unit test `swap_panes_works_across_splits_and_keeps_unknown_ids` (`src/pane/manager/mod.rs:733-776`) encodes this and its own message contradicts its assertion. Swap leaf contents instead and keep sizes in place.
- **PN2. Split direction and names.** T7 names, K11/K12 chords, icons in the demote direction picker (`src/app/render_pipeline/egui_dialogs.rs:78, 89`), and consistent docs.
- **PN3. Resize follows the arrow.** Today Right/Down grow and Left/Up shrink the innermost split containing the pane, regardless of its axis (`src/pane/manager/layout.rs:94-128`; `app/tab_ops/pane_ops.rs:364-373`), so Resize-Up can change width. Target: move the nearest divider on that side of the pane in the arrow's direction (tmux semantics). Step configurable (`pane_resize_step`).
- **PN4. Enforce `pane_min_size`** (shown in Settings, never read; B6) during drag, keyboard resize, split, and restore. Refuse a split that would violate it, with a toast.
- **PN5. Focus after close** goes to the previously focused pane, then the sibling, not the first pane in tree order (`src/pane/manager/focus.rs:22-33`).
- **PN6. Splits inherit the tab's profile** (command, SSH target, cwd, badge) with `split_inherits_profile: true` (D5). Today splits use global config (`src/pane/manager/creation.rs:177-181`), so splitting an SSH tab gives a local shell `[inferred]` (RT9).
- **PN7. Optional auto-balance** (`split_balance: none|siblings|all`) so repeated splits don't produce 50/25/12.5%.
- **PN8. Divider ergonomics.** Default `pane_divider_hit_width` 5 → 8; double-click a divider equalizes its split; keep hover color.
- **PN9. Dim inactive panes by darkening, not transparency.** `dim_inactive_panes` scales opacity, including text alpha (`par-term-render/src/cell_renderer/pane_render/text_render.rs:70-75`), which makes text unreadable over background images/shaders. Add `inactive_pane_dim_mode: darken|fade`, default darken.
- **PN10. Max-pane feedback.** Toast "Pane limit (16) reached — change in Settings › Panes" and remove the misleading log line "split not yet functional (renderer integration pending)" (`app/tab_ops/pane_ops.rs:125-130`).
- **PN11. Arrangements and Duplicate Tab keep pane layouts.** Arrangements store only single-pane titles (`src/arrangements/capture.rs:93-106`); duplicate copies only cwd/color/icon (`src/tab/manager.rs:466-505`). Store the tree the way session restore already does (`src/session/mod.rs:88-121`).
- **PN12. Restore divider sizing** uses `unwrap_or(1.0)` with no DPI scale (`src/tab/pane_ops.rs:376-377`) versus `unwrap_or(2.0) * dpi_scale` elsewhere (B14).
- **PN13. Remove or implement `pane_title_font`** (listed in CONFIG_REFERENCE, never read; B7).
- **PN14. Pane hover focus.** `focus_follows_mouse` only focuses the OS window on cursor enter (`src/app/handler/window_state_impl/handle_window_event.rs:501-507`); there is no hover-to-focus between panes. Add `pane_focus_follows_mouse` (off by default).
- **PN15. Composable macros.** Custom action sequences resolve `SequenceStep.action_id` against `config.actions` only (`workflow.rs:57-83`), so a user can't build "split right, then run X, then equalize". Let sequence steps call any built-in action id.

---

## 9. Tab and window behavior changes

- **TW1. Closing a background tab keeps you where you are.** The close button and context menu switch to the tab first, then close it, so focus lands on its neighbor (`src/app/handler/action_handlers/tab_bar.rs:45-55`; `src/tab/manager.rs:173-182`). Close by id without switching.
- **TW2. Quit saves every window.** Session save runs only in `close_window` when exactly one window remains (`src/app/window_manager/window_close.rs:18-23`), and Quit closes windows one at a time, so only the last window is saved `[verified code path]` (RT10 confirms the user-visible result). Capture all windows before the first close.
- **TW3. Reopened tabs keep their custom name and icon.** `ClosedTabInfo` lacks `user_named` and `custom_icon` (`src/app/tab_ops/mod.rs:22-33`; `tab_reopen.rs:119-124`).
- **TW4. One close vocabulary.** `close_tab` (action) is a no-op on the last tab (`keybinding_actions.rs:187-193`) while Cmd+W closes the window. Decide one rule: `close_tab` on the last tab closes the window (matches Cmd+W), and A14 adds a real close-window.
- **TW5. Safer defaults for power users** (D6): `confirm_close_running_jobs: true`, `session_undo_timeout_secs` 5 → 30.
- **TW6. Rename tab everywhere:** action (A12), double-click, leader `,`. Clicking outside the inline rename should cancel, not save (B31).
- **TW7. Mouse gestures:** middle-click closes a tab; double-click empty tab-bar space opens a new tab; drag a tab out of the bar to open it in a new window; drop onto another window's tab bar to move it (local tabs only).
- **TW8. Window list and switching:** A13, macOS Window menu listing, stable window numbers that don't collide after a close (`window_lifecycle.rs:53, 299` uses `windows.len()+1`), and a stable order in the Move submenu (today HashMap order, `window_close.rs:60`).
- **TW9. Keyboard tab move stops at the ends** (today wraps; `manager_nav.rs:83-93`), or make wrap a setting.
- **TW10. Default "Tab N" titles don't renumber** when other tabs close or move (`manager.rs:347-351`).
- **TW11. Profiles can open as a window or a split,** not only a tab (A19 plus "Open profile in new window").
- **TW12. `max_tabs` feedback is uniform** (toast everywhere) and Duplicate respects it (B16).
- **TW13. Arrangements from the keyboard:** `restore_arrangement` gets a picker in the palette; `save_arrangement` prompts for a name inline instead of opening Settings (`keybinding_display_actions.rs:149-155`).

---

## 10. Decisions for the owner

**Owner answers (2026-09-28):** D1 approved, extended to "wherever possible align with iTerm2" (Section 3.3a). D2, D3, D4, D5, D7, D8 approved. D6 pending (options below).

- **D1. Flip macOS split chords to iTerm2's meaning** (Cmd+D = Split Right) and give Linux Terminator's pair (Ctrl+Shift+E right, Ctrl+Shift+O down). Existing users keep their current bindings **only once the Section 3.3 migration rule ships**; without it the default merge would add the new chord alongside the old one (B20, B23). With the rule, only new installs and "reset to default" change. **Recommendation: yes, gated on the migration rule.** The defaults comment already claims iTerm2 parity (`par-term-config/src/defaults/misc.rs:57`), and today the chord matches but the result is the opposite.
- **D2. Give Cmd/Ctrl+Shift+P to the command palette**, moving the profile drawer to leader `P` and its menu item. **Recommendation: yes.** The palette is the discoverability hub and has no default chord at all.
- **D3. Leader key defaults:** macOS Cmd+B, Linux/Windows Ctrl+Shift+B (moving background-shader toggle to Ctrl+Alt+B there). Alternative: ship the leader disabled and prompt once on first par-mux attach. **Recommendation: enabled by default**; Cmd+B is unused, and Ctrl+Shift+B only displaces a rarely used toggle.
- **D4. Use Cmd+Shift+T for Reopen Closed Tab** (browser convention), moving throughput mode to the palette. **Recommendation: optional, low priority.**
- **D5. Splits inherit the tab's profile, including SSH.** **Recommendation: yes, behind `split_inherits_profile` default true.**
- **D6. Close-safety defaults** (TW5). Pending owner answer. The settings in question and today's values:
  - `confirm_close_running_jobs` (ask before closing a tab or pane whose shell has a foreground job not in `jobs_to_ignore`): par-term **false**. iTerm2's per-profile equivalent defaults to "never prompt", with a job-aware mode available.
  - `prompt_on_quit` (confirm quitting): par-term **false**. iTerm2 **YES**.
  - Confirm closing a window with multiple tabs: par-term has no setting. iTerm2 `ConfirmClosingMultipleTabs` **YES**.
  - `session_undo_timeout_secs` (how long Reopen Closed Tab works): par-term **5**. iTerm2 **5**.
  - `session_undo_preserve_shell` (reopen keeps the live process instead of a fresh shell): par-term **false**. iTerm2 keeps the live session during the undo window.

  **Recommendation, following the iTerm2-alignment rule:** `prompt_on_quit: true`, add `confirm_close_multiple_tabs: true`, keep undo at 5 s but set `session_undo_preserve_shell: true` (that is what iTerm2's undo does), and leave `confirm_close_running_jobs: false` for local tabs. Attached par-mux tabs always get the M1/M2 dialogs regardless, because closing them can end a session.
- **D7. Hidden tabs as a first-class par-mux concept** (M4). The alternative is making last-pane close kill the window. **Recommendation: hidden tabs**, since it keeps the deliberate "last pane close is safe" choice and gives Cmd+Z real meaning.
- **D8. Per-tab broadcast scope** instead of per-window (V5). **Recommendation: yes.**

---

## 11. Phased roadmap with acceptance criteria

Each phase is independently shippable. Every phase ends with `make checkall` green and docs updated in the same change.

### P0. Correctness quick wins (small, no design risk)

Scope: B1 (leaf swap), B2 (activity dot), B3 (`tab_show_index`), B5/M13 (roster jump), B8 (badge shows Alt+N off macOS), B9 (undo toast chord text), B10 (menu New Tab honors profile setting), B12/PN5 (focus after close), B13/TW1 (background close), B14/PN12, B15/TW3, B16, B28/PN10, B29, B31, DOC1–DOC6.

Acceptance:
- Swapping top-left with top-right in a 2×2 grid moves only those two panes, locally and attached; the existing unit test is rewritten to assert that.
- A background tab that receives output shows the activity dot; the dot clears on focus.
- Selecting an agent roster row whose pane is in tab 1 and in tab 3 focuses the correct tab and pane (new test covering the focus step).
- Closing a background tab via × leaves the active tab unchanged.
- Reopening a renamed tab restores its name and icon, and a later OSC title does not overwrite it.
- MUX.md documents the working detach action id; KEYBOARD_SHORTCUTS.md lists swap chords.

### P1. par-mux safety

Scope: M1–M9, M11, M12, M14, M16, TW2, B32, U5–U10, U14.

Acceptance:
- Closing the last attached tab shows the Detach / End session / Cancel dialog; Detach leaves the session listed by `par-mux list-sessions`.
- Closing an attached pane running `sleep 100` asks for confirmation when `confirm_close_running_jobs` is on.
- Closing an attached tab's last pane shows "1 hidden" on the session chip; Cmd+Z and the picker both re-show it; its agents' roster rows re-show it on select.
- Cmd+Z after killing an attached tab never restores a different tab.
- A forced split transport error creates no local pane and shows a persistent error.
- Quit with three windows and `restore_session: true` restores three windows (RT10 becomes a test).
- The quit dialog names attached sessions as "will keep running".
- No par-mux path shows the word "tmux".

### P2. Keymap system and leader

Scope: K2 (single registry, menus read it), K3, K4–K9 leader + which-key, K6, K10–K22, K23–K28 Settings editor, B18–B26.

Acceptance:
- Loading a config saved by the previous release preserves every existing binding and produces no duplicate chords (fixture test over a pre-change config with `split_horizontal` on Cmd+D and the profile drawer on Cmd+Shift+P).
- Every chord in Appendix C resolves through the registry; `KEY_LAYERS` holds no chords; a test enumerates menus and asserts their accelerators equal the registry binding.
- Setting a hardcoded chord (for example `Alt+1`) to "pass to terminal" delivers it to the shell and survives restart.
- The leader works identically in a local tab, an attached tab, and a tmux gateway tab; the overlay lists live bindings.
- The Settings editor records Cmd+Ctrl+Alt+Arrow and Cmd+Shift+], flags a duplicate chord across registry and menu, and filters by typed text.
- The palette shows the user's live chord after a rebind.
- A `--ui-test` script exercises leader `z`, `o`, `;`, `E` (after P3) and pane swap.

### P3. Pane power features

Scope: A1, A3–A9, PN3, PN4, PN6–PN9, PN11, PN14, PN15, V5–V7, V9, M7.

Acceptance:
- Zoom toggles in local and attached tabs, shows its indicator, and unzooms on split or focus move.
- Equalize produces equal leaf areas within one cell for the five layout presets.
- Resize with an arrow moves the divider in that direction in every nesting case (property test over random trees).
- Keyboard focus, resize, and tab switch are visible to a second `par-mux` client (`list-panes` active flag, pane sizes).
- `pane_min_size` is never violated after any operation (property test).
- Broadcast shows pane borders and a tab badge, is per tab, and includes paste.

### P4. Session management and navigation

Scope: A10–A18, A21–A23, V1–V3, V10, V13, M10, M15, U11–U13, UP1–UP8 as they land upstream.

Acceptance:
- Without editing any profile, a user can attach, create, switch, and detach a par-mux session from the session picker and palette.
- The tree picker lists windows, tabs, panes, and hidden tabs, and jumps to any of them.
- Leader `a` cycles blocked then done-unseen agents across tabs; tab and pane badges match roster state.
- The session chip shows name, health, and hidden count, and holds the last error until dismissed.
- Reattach shows scrollback up to the configured limit.
- Rename and End session work from the picker once UP1/UP2 exist; before that, the actions are shown disabled with the reason.

### P5. Polish and parity

Scope: TW4–TW13 remainder, PN2 icons, A19, A20, T-series string sweep, V12, D4, Linux/Windows chord audit (RT2, RT3).

Acceptance:
- A string search over `src/` user-facing text finds "session" only in the T4 meaning.
- F1 help and KEYBOARD_SHORTCUTS.md are generated from, or tested against, the registry.
- Tab drag-out creates a window; drag onto another window moves a local tab.

---

## Appendix A: bugs found during research

`[verified]` = read in source; `[inferred]` = needs runtime confirmation (Appendix E).

| Code | Bug | Evidence |
|---|---|---|
| B1 | Directional swap swaps whole subtrees in 3+ pane layouts; mux swaps two panes | `src/pane/types/pane_node.rs:275-293`; test `src/pane/manager/mod.rs:733-776` [verified] |
| B2 | Tab activity dot never lights: `TabManager::mark_activity` has zero callers | `src/tab/manager.rs:420-426`; parsight `find_callers` empty [verified] |
| B3 | `tab_show_index` does nothing | `src/tab_bar_ui/tab_painter.rs:149-152` [verified] |
| B4 | Quit saves only the last window for session restore | `src/app/window_manager/window_close.rs:18-23`; `menu_actions.rs:95-100` [verified code path; RT10] |
| B5 | Roster row jump uses a 0-based index with a 1-based, visible-only switch | `src/app/tmux_handler/gateway.rs:302-310`; `src/tab/manager_nav.rs:65-77` [verified] |
| B6 | `pane_min_size` is shown in Settings and never enforced | `par-term-settings-ui/src/window_tab/panes.rs:156-169` [verified] |
| B7 | `pane_title_font` is documented and never read | `docs/CONFIG_REFERENCE.md:415, 428` [verified] |
| B8 | Tab shortcut badge shows `^N` off macOS; the chord is Alt+N | `tab_painter.rs:231-236` vs `key_handler/tabs.rs:134` [verified] |
| B9 | Undo toast prints the raw config string "CmdOrCtrl+Z" | `src/app/tab_ops/lifecycle.rs:428-441` [verified] |
| B10 | Menu New Tab ignores `new_tab_shortcut_shows_profiles`; the menu owns Cmd+T on macOS and Ctrl+Shift+T on Windows | `menu_actions.rs:37-43` vs `keybinding_actions.rs:183-186` [verified; RT11] |
| B11 | `close_tab` action is a no-op on the last tab while Cmd+W closes the window | `keybinding_actions.rs:187-193`; `key_handler/tabs.rs:65-71` [verified] |
| B12 | After closing a pane, focus goes to the first pane in tree order | `src/pane/manager/focus.rs:22-33` [verified] |
| B13 | Closing a background tab moves focus to its neighbor | `action_handlers/tab_bar.rs:45-55` [verified] |
| B14 | Restored tabs get thinner, unscaled dividers | `src/tab/pane_ops.rs:376-377` vs `:124-127` [verified] |
| B15 | Reopened tabs lose custom name and icon | `src/app/tab_ops/mod.rs:22-33`; `tab_reopen.rs:119-124` [verified] |
| B16 | Duplicate tab ignores `max_tabs` | `src/app/tab_ops/tab_helpers.rs:17-81` [verified] |
| B17 | Keyboard resize ignores the arrow axis | `src/pane/manager/layout.rs:94-128` [verified] |
| B18 | Keybinding recorder drops Ctrl when Cmd is held (can't record the shipped swap default); can't record Super off macOS | `par-term-settings-ui/src/input_tab/keybindings.rs:265-278` [verified] |
| B19 | Recorded punctuation chords (`BracketLeft`, `Comma`, …) are unparseable and silently dropped | `keybindings.rs:371-379`; `par-term-keybindings/src/platform.rs:97-134`; `lib.rs:77-83` [verified] |
| B20 | Clearing a default binding doesn't survive restart | `par-term-config/src/config/keybindings_methods.rs:25-35`; `persistence.rs:102` [verified] |
| B21 | Settings legend says gray = default, white = custom; defaults render white | `keybindings.rs:47, 99` [verified] |
| B22 | Palette shows static advertised chords, not live bindings | `src/command_palette/catalog.rs:63-68` [verified] |
| B23 | Registry is a HashMap: same chord under two spellings resolves nondeterministically; new defaults can overwrite same-spelled user bindings | `par-term-keybindings/src/lib.rs:37, 74, 124` [verified] |
| B24 | On Linux, Shift+F11 toggles fullscreen instead of maximize-vertically | `keyboard_handlers.rs:19` [verified] |
| B25 | Linux in-app menu shows Ctrl+Shift+N/Q/A/, labels that nothing handles | `src/menu/linux.rs`; `menu/model.rs` [verified] |
| B26 | Ctrl+Shift+] / [ on Linux may never match because Shift changes the logical key | `key_handler/tabs.rs:75-86` [inferred; RT12] |
| B27 | tmux gateway: keyboard pane focus doesn't update the tmux target, so keys may go to the previously clicked pane | `gateway_input.rs:131`; `par-term-tmux/src/session.rs:317-325` [inferred; RT13] |
| B28 | Pane limit is silent, with a misleading log line | `app/tab_ops/pane_ops.rs:125-130` [verified] |
| B29 | Tab context menu shows two separators in a row | `src/tab_bar_ui/context_menu.rs:246-254` [verified] |
| B30 | New-tab profile dropdown always anchors top-right | `src/tab_bar_ui/profile_menu.rs:39-42` [verified] |
| B31 | Clicking outside inline tab rename saves the text | `context_menu.rs:348-352` [verified] |
| B32 | Menu Quit and the `quit` action skip `prompt_on_quit` | `handle_window_event.rs:80-96`; `menu_actions.rs:95-100` [verified] |
| B33 | Windows font-size menu accelerators are Ctrl+Shift+=/-/0 while the handler and docs use Ctrl+=/-/0 | `menu/model.rs:288-300` [verified] |
| B34 | `save_arrangement` action opens Settings without selecting the Window tab | `keybinding_display_actions.rs:149-155` [verified] |

## Appendix B: documentation drift

| Code | Doc | Problem |
|---|---|---|
| DOC1 | `docs/features/MUX.md:119` | Names `detach_mux_session`; the working id is `mux-detach` (`keybinding_actions.rs:671`) |
| DOC2 | `docs/features/MUX.md:43` | Says the glyph distinguishes mux tabs; it is drawn per window |
| DOC3 | `docs/features/MUX.md:45` | Promises an "already attached" toast that is only reachable mid-attach |
| DOC4 | `docs/guides/GETTING_STARTED.md:239-240, 258-261` | Split directions reversed; resize described as edge-based |
| DOC5 | `docs/guides/KEYBOARD_SHORTCUTS.md:171-188` | Pane table lacks swap and rename; broadcast is in another section |
| DOC6 | `KEYBOARD_SHORTCUTS.md:257, 166, 37-53` | Windows menu chords are Ctrl+Shift+N/W/Q; Shift+F11 is fullscreen only on Linux; Linux and Windows share a column though only Windows has a native menu |
| DOC7 | `src/help_ui.rs:127-198` (F1) | Wrong rows (F11 shader editor, Ctrl+Shift+S screenshot, Ctrl+Shift+F5, plain PageUp); Ctrl chords shown on macOS; no window/tab/pane/mux section |
| DOC8 | `KEYBOARD_SHORTCUTS.md:249-315` | Available Actions omits `rename_pane`, `toggle_agent_usage_panel`, `mux-detach`, `agent-roster-focus:`, `launch-agent*`, `agent-cmd:`, `plugin-action:`, `triage-crash:` |
| DOC9 | `KEYBOARD_SHORTCUTS.md:266` | Claims every palette chord candidate is taken; several are free |
| DOC10 | `MATRIX.md:152, 137` | Duplicate tab chord is Shift+J, not Shift+D; "Tab index numbers" marked done but the option does nothing |
| DOC11 | `docs/features/TABS.md` | No panes section: swap, broadcast, resize semantics, and zoom (once built) are undocumented; no dedicated panes doc exists |
| DOC12 | `docs/features/SESSION_MANAGEMENT.md:60-64` | Implies reopened tabs keep names (B15) |
| DOC13 | `par-term-settings-ui/src/window_tab/panes.rs:172-200` | Hardcoded shortcut hint ignores rebinding and omits swap, hint select, broadcast |
| DOC14 | `par-term-config/src/defaults/misc.rs:57` | "Cmd+D / Cmd+Shift+D matches iTerm2" — chord matches, direction is opposite |

## Appendix C: current keybinding inventory

Registry defaults (`par-term-config/src/defaults/misc.rs`), hardcoded layers (`src/app/input_events/key_handler/`), and native menus (`src/menu/model.rs`, `src/menu/macos.rs`). "M" = native menu accelerator (macOS and Windows only; cannot be overridden today). "H" = hardcoded (can be shadowed, not freed). "R" = registry.

### Windows and tabs

| Action | macOS | Linux | Windows | Source |
|---|---|---|---|---|
| New window | Cmd+N (M) | none | Ctrl+Shift+N (M) | `menu/model.rs:97-102` |
| Close window (smart: closes a tab) | Cmd+W (M) | none | Ctrl+Shift+W (M) | `menu_actions.rs:27-36` |
| Quit | Cmd+Q (M) | none | Ctrl+Shift+Q (M) | `macos.rs:69-75` |
| Fullscreen | F11 (M+H) | F11 (H) | F11 (M+H) | `keyboard_handlers.rs:19` |
| Maximize vertically | Shift+F11 (M) | = fullscreen (B24) | Shift+F11 (M) | `model.rs:278-283` |
| New tab | Cmd+T (M, H) | Ctrl+Shift+T (H) | Ctrl+Shift+T (M, H) | `key_handler/tabs.rs:34-49` |
| Close tab | Cmd+W (M) | Ctrl+Shift+W (H) | Ctrl+Shift+W (M) | `tabs.rs:53-72` |
| Duplicate tab | Cmd+Shift+J (R, M) | Ctrl+Shift+J (R) | Ctrl+Shift+J (R) | `misc.rs:164-167, 311-314` |
| Reopen closed tab | Cmd+Z (R) | Ctrl+Shift+Z (R) | Ctrl+Shift+Z (R) | `misc.rs:152-155, 299-302` |
| Next / prev tab | Cmd+Shift+] / [ (M, H), Ctrl+Tab / Ctrl+Shift+Tab (H) | Ctrl+Shift+] / [, Ctrl+Tab (H) | same as Linux | `tabs.rs:75-106` |
| Move tab left / right | Cmd+Shift+Left/Right (M, H) | Ctrl+Shift+Left/Right (H) | same (M, H) | `tabs.rs:109-126` |
| Tab 1–9 | Cmd+1..9 (M, H) | Alt+1..9 (H) | Alt+1..9 (M, H) | `tabs.rs:131-158` |
| Move tab to new window, promote, demote, rename pane, save arrangement, palette, agent usage, `mux-detach` | none | none | none | `keybinding_actions.rs` |

### Panes (all registry)

| Action | macOS | Linux / Windows |
|---|---|---|
| Split (stacked, today `split_horizontal`) | Cmd+D | Ctrl+Shift+D |
| Split (side by side, today `split_vertical`) | Cmd+Shift+D | Ctrl+Shift+E |
| Close pane | Cmd+Shift+W | Ctrl+Shift+X |
| Focus pane | Cmd+Opt+Arrow | Ctrl+Alt+Arrow |
| Resize pane | Cmd+Opt+Shift+Arrow | Ctrl+Alt+Shift+Arrow |
| Swap pane | Cmd+Ctrl+Opt+Arrow | Alt+Shift+Arrow |
| Select pane by letter | Cmd+Opt+P | Ctrl+Alt+P |
| Broadcast input | Cmd+Opt+I | Ctrl+Alt+I |
| tmux session picker | Cmd+Opt+T | Ctrl+Alt+T |

### Other chords that constrain new defaults

Registry: Cmd/Ctrl+Shift+B (background shader), Cmd/Ctrl+Shift+U (cursor shader), Cmd+Shift+V / Ctrl+Alt+V (paste special), Cmd/Ctrl+Shift+R (session logging), Cmd+Shift+T / Ctrl+Shift+M (throughput), Cmd+Shift+C / Ctrl+Shift+Space (copy mode), Cmd+R / Ctrl+Alt+R (command history), Cmd/Ctrl+Shift+S (SSH quick connect), Cmd/Ctrl+Shift+P (profile drawer).

Hardcoded: Cmd+F / Ctrl+Shift+F (search), Cmd+I / Ctrl+Shift+I (assistant), Cmd/Ctrl+Shift+H (clipboard history), Cmd/Ctrl+Shift+K (clear scrollback), Ctrl+L, font size on Cmd/Ctrl + `= - _ 0` (loose modifiers), Ctrl+, and Cmd+, (cursor style / settings), F1 F3 F5 F11 F12, Shift+PageUp/PageDown/Home/End, Super+Up/Down, copy/paste chords, Shift+Enter.

Modal: copy mode, pane-hint select, demote pick, custom-action prefix (`custom_action_prefix_key`, off by default), tmux prefix (`C-b`, tmux gateway only).

Known chord risks: Linux Ctrl+Alt+Arrow (GNOME workspaces), Ubuntu Ctrl+Alt+T (open terminal), IBus Ctrl+Shift+U and possibly Ctrl+Shift+E, Windows Alt+Shift (input language), macOS Cmd+Ctrl+Opt+Arrow (window managers such as Rectangle), macOS F11 (Show Desktop). These are conventions recalled from memory and are listed in Appendix E (RT2) for checking on target systems.

## Appendix D: upstream daemon gaps (par-term-emu-core-rust)

Checked against the daemon command table (`par-term-emu-core-rust/src/mux/command.rs:681-705`, version 0.55.0). Each should be filed on the core project's board when its phase starts.

Filed on the par-term-emu-core-rust board 2026-09-28: UP1+UP2 `01a0ea74ec2e74d0ae11815d14fc25f6`, UP3 `01a0ea74eddf7960bf69078e11984522`, UP4+UP8 `01a0ea74ef9b76929f4f1a84eca8a5f2`, UP5 `01a0ea74f1ba70f099f836051d9dd4a1`, UP6+UP7 `01a0ea74f3757b91be963081760df36c`.

| Code | Needed for | Missing command |
|---|---|---|
| UP1 | Rename session (A16, L `$`) | `rename-session` |
| UP2 | End session explicitly (M1, A16) | `kill-session` (today only a side effect of killing the last window) |
| UP3 | True pane zoom in attached tabs (A1) | `resize-pane -Z` or equivalent zoom state |
| UP4 | Promote/demote for attached tabs (M8, U10) | `break-pane`, `join-pane` / `move-pane` |
| UP5 | Restart dead pane (A9, U16) | `respawn-pane` |
| UP6 | Split left/up (A5) | `split-window -b` |
| UP7 | Running-job check (M2) | Foreground command per pane (`#{pane_current_command}` or a query) — confirm whether it already exists |
| UP8 | Tab order survives reattach (U11) | `move-window` / `swap-window` |

Already supported and unused by par-term: `list-sessions` (session picker), `new-session` without a profile, `split-window -p/-c`, `new-window -n/-c`, `select-window`, `select-pane`, `capture-pane` (scrollback on reattach).

## Appendix E: items to confirm at runtime

These follow from the code but nobody ran the app. Confirm each before building on it.

| Code | Check |
|---|---|
| RT1 | Cmd/Ctrl+Shift+Enter reaches the registry before the hardcoded Shift+Enter handler |
| RT2 | OS-level claims for proposed chords (Cmd+Opt+A, Ctrl+Alt+A, Ctrl+Alt+=, Ctrl+Shift+O) on macOS, GNOME, KDE, Windows |
| RT3 | winit delivers Cmd+` to par-term on macOS |
| RT4 | A keyboard resize in an attached tab snaps back on the next `%layout-change` |
| RT5 | Promote/demote on an attached tab stops output and input for the moved pane |
| RT6 | Tab order resets on reattach |
| RT7 | Clear scrollback in an attached tab has no visible effect |
| RT8 | An OSC title from a program replaces the `[mux: name]` window-title suffix |
| RT9 | Splitting a tab opened from an SSH profile gives a local shell |
| RT10 | Quit with 3 windows and `restore_session: true` restores only one |
| RT11 | Cmd+T ignores `new_tab_shortcut_shows_profiles` on macOS |
| RT12 | Ctrl+Shift+] / [ switch tabs on Linux |
| RT13 | In a tmux gateway window, keys typed after keyboard pane navigation go to the newly focused pane |
| RT14 | After detach or daemon death, a window is left with zero tabs |
