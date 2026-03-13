# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

par-term is a cross-platform GPU-accelerated terminal emulator frontend built in Rust. It uses the [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) library for VT sequence processing, PTY management, and inline graphics protocols (Sixel, iTerm2, Kitty). The frontend provides GPU-accelerated rendering via wgpu with custom WGSL shaders, including support for custom post-processing shaders (Ghostty/Shadertoy-compatible GLSL).

**Language**: Rust (Edition 2024)
**Platform**: Cross-platform (macOS, Linux, Windows)
**Graphics**: wgpu (Vulkan/Metal/DirectX 12)
**Version**: 0.25.0

## Development Commands

### Build & Run

**IMPORTANT**: Use `make build` / `make run` for day-to-day development. These use the `dev-release` profile (opt-level 2, no LTO, incremental) which rebuilds in ~1-2s after code changes (~1m20s clean build) with ~90-95% of full release performance. Only use `make build-debug` when you need debug symbols for stepping through code, and `make build-full` / `make release` for distribution builds.

```bash
make build          # Dev-release build (optimized, incremental — ~1-2s rebuild, preferred)
make build-full     # Full release build (LTO, single codegen unit — ~3min, for distribution)
make build-debug    # Debug build (unoptimized, for stepping through code)
make run            # Run in dev-release mode (preferred)
make run-release    # Run in full release mode
```

### Testing & Code Quality
```bash
make test           # Run all tests
make test-one TEST=test_name  # Run specific test
make all            # Format, lint, test, and build
make pre-commit     # Run pre-commit checks (fmt-check, lint, test)
make ci             # Full CI checks (fmt-check, lint-all, test, check-all)
make fmt            # Format code with rustfmt
make lint           # Run clippy
cargo test -- --include-ignored  # Run all tests including PTY-dependent ones
```

### Debugging

**IMPORTANT**: When stopping a debug instance, NEVER use `killall par-term` — this will kill ALL par-term processes including the terminal you're working in. Use `pkill -f "target/debug/par-term"` or kill by PID.

```bash
make run-debug      # Run with DEBUG_LEVEL=3 (logs to /tmp/par_term_debug.log)
make run-trace      # Run with DEBUG_LEVEL=4 (most verbose)
make tail-log       # Monitor debug log in real-time
```

The project uses **custom debug macros**, not the standard `log` crate:
```rust
crate::debug_info!("CATEGORY", "message {}", var);   // DEBUG_LEVEL=2+
crate::debug_log!("CATEGORY", "message");            // DEBUG_LEVEL=3+
crate::debug_trace!("CATEGORY", "message");          // DEBUG_LEVEL=4
crate::debug_error!("CATEGORY", "message");          // DEBUG_LEVEL=1+
// Do NOT use log::info!() etc. — they won't appear in the debug log
```

Common log categories: `TAB`, `TAB_BAR`, `TAB_ACTION`, `MOUSE`, `RENDER`, `SHADER`, `TERMINAL`, `APP`

**When testing, use the debug build window** (started via `cargo run`), not the app bundle. The app bundle won't have your code changes.

See `docs/LOGGING.md` for full logging documentation.

### ACP Agent Debugging (Assistant Panel / Claude+Ollama)

When debugging ACP agent behavior (tool-call failures, prompt stalls, malformed XML-style tool output, Claude/Ollama wrapper issues), use the ACP harness before relying on the GUI alone:

- `make acp-harness ARGS="--list-agents"` to confirm agent discovery and custom agent config loading
- `make acp-smoke` to run the reproducible shader prompt smoke test and save a transcript

See `docs/ACP_HARNESS.md` for usage, transcript capture, and troubleshooting.

### Other Commands
```bash
make test-graphics     # Test graphics with debug logging
make test-fonts        # Run comprehensive text shaping test suite
make profile           # CPU profiling with flamegraph
make clean             # Clean build artifacts
make doc-open          # Generate and open documentation
make bundle            # Create macOS .app bundle (macOS only)
```

## Task Tracking Requirements

**IMPORTANT**: Always use the task system (TaskCreate/TaskUpdate) for ALL work, even small jobs. This enables external monitoring of progress.

1. **Create tasks** at the start of any request using `TaskCreate`
2. **Mark in_progress** when starting work using `TaskUpdate`
3. **Mark completed** when done
4. Break multi-step work into individual tasks for visibility

## Architecture Overview

See `docs/ARCHITECTURE.md` for detailed architecture documentation.

**Key layers**: App (`src/app/`) → Terminal (`src/terminal/`) → Renderer (`src/renderer/`, `src/cell_renderer/`) → GPU Shaders (`src/shaders/`)

**Data flow**: Window Events → Input Handler → PTY → VT Parser → Styled Segments → GPU Renderer (three passes: cells → graphics → egui overlay)

**Key patterns**:
- Tokio runtime for async PTY I/O, sync wrappers for the event loop
- Glyph atlas with instanced rendering for text
- RGBA texture caching for inline graphics (Sixel/iTerm2/Kitty)
- Scrollback buffer with viewport offset rendering

## Key File Map (Navigation Guide)

| Area | Primary Files | Sub-crate |
|------|--------------|-----------|
| **Rendering (pane path)** | `par-term-render/src/cell_renderer/pane_render/mod.rs`, `par-term-render/src/cell_renderer/render.rs`, `src/app/render_pipeline/gpu_submit.rs` | `par-term-render` |
| **Rendering (search highlights overlay)** | `src/app/window_state/search_highlight.rs` | main |
| **Cursor rendering** | `par-term-render/src/cell_renderer/bg_instance_builder.rs`, `cursor.rs` | `par-term-render` |
| **Block characters (▄▀ etc.)** | `par-term-render/src/cell_renderer/block_chars/` | `par-term-render` |
| **Input handling** | `src/app/input_events/`, `src/input.rs` | `par-term-input` |
| **Tab management** | `src/tab/manager.rs`, `src/app/tab_ops/` | main |
| **Tab bar UI** | `src/tab_bar_ui/` (11 subdirs) | main |
| **Settings UI** | `src/settings_window/`, `par-term-settings-ui/` | `par-term-settings-ui` |
| **Configuration** | `par-term-config/src/lib.rs` | `par-term-config` |
| **Session save/restore** | `src/session/capture.rs`, `src/app/window_manager/window_session.rs` | main |
| **Keybindings** | `par-term-keybindings/` | `par-term-keybindings` |
| **Snippets/Actions** | `src/snippets/`, `src/app/input_events/` | main |
| **Custom shaders** | `src/shader_installer.rs`, `shaders/` dir, `par-term-render/src/` | `par-term-render` |
| **SSH** | `src/ssh/`, `par-term-ssh/` | `par-term-ssh` |
| **Tmux integration** | `src/tmux_*/`, `par-term-tmux/` | `par-term-tmux` |
| **ACP / AI panel** | `src/acp_harness/`, `src/ai_inspector/`, `par-term-acp/` | `par-term-acp` |
| **Font/text shaping** | `par-term-fonts/` | `par-term-fonts` |

## Code Organization Guidelines

- **Target**: Keep files under 500 lines; refactor files exceeding 800 lines
- Extract modules when logical groupings emerge (see existing patterns: `src/app/`, `src/terminal/`, `src/cell_renderer/`)
- Centralize constants, prefer composition over duplication, create helper traits for shared functionality

## Platform-Specific Notes

- **macOS**: Metal backend, platform code in `src/macos_metal.rs`, bundle via `make bundle`
- **Linux**: Vulkan backend, requires X11/Wayland libs (`libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`)
- **Windows**: DirectX 12 backend

## Configuration

Location (XDG-compliant): `~/.config/par-term/config.yaml` (Linux/macOS), `%APPDATA%\par-term\config.yaml` (Windows)

See `par-term-config/src/config/config_struct/mod.rs` for all available settings and defaults.

## Sub-Crate Dependency Graph (for version bumps)

When bumping sub-crate versions for crates.io publishing, bump in dependency order. Update both the crate's own `version` field and any `version = "..."` in dependents' `Cargo.toml` references.

```
Layer 0 — No internal deps (bump in any order):
  par-term-acp
  par-term-ssh
  par-term-mcp

Layer 1 — Foundation (bump before anything that depends on it):
  par-term-config
    └── depends on: (none, only external par-term-emu-core-rust)

Layer 2 — Depend on par-term-config only (bump after Layer 1):
  par-term-fonts        → par-term-config
  par-term-input        → par-term-config
  par-term-keybindings  → par-term-config
  par-term-prettifier   → par-term-config
  par-term-scripting    → par-term-config
  par-term-settings-ui  → par-term-config
  par-term-terminal     → par-term-config
  par-term-tmux         → par-term-config
  par-term-update       → par-term-config

Layer 3 — Depend on Layer 2 crates (bump after Layer 2):
  par-term-render       → par-term-config, par-term-fonts

Layer 4 — Root crate (bump last):
  par-term              → all of the above
```

**Quick bump checklist:**
1. Bump `par-term-config` version + update refs in all Layer 2/3 crates
2. Bump Layer 0 crate versions
3. Bump Layer 2 crate versions
4. Bump `par-term-render` version + update its `par-term-fonts` ref
5. Update all version refs in root `Cargo.toml`
6. Run `cargo check --workspace` to verify

## Common Development Workflows

### Adding a New Configuration Option
1. Add field to `Config` struct in `par-term-config/src/config/config_struct/mod.rs` with `#[serde(default = "default_my_option")]`
2. Update `Default` impl
3. Use config value in relevant component
4. **REQUIRED**: Add UI controls in the appropriate `par-term-settings-ui/src/*_tab.rs`
   - Set `settings.has_changes = true` and `*changes_this_frame = true` on change
5. **REQUIRED**: Update search keywords in `par-term-settings-ui/src/sidebar.rs` → `tab_search_keywords()`

### Adding a New Keyboard Shortcut
1. Add key handling in `src/app/input_events.rs`
2. If needed, add sequence generation in `src/input.rs` → `InputHandler`

### Adding Snippet or Action Keybindings
See `docs/SNIPPETS.md` for full documentation. Key points:
- Snippets use `snippet:<id>`, actions use `action:<id>` as keybinding action names
- Auto-generated during config load via `generate_snippet_action_keybindings()`
- `execute_keybinding_action()` in `input_events.rs` handles execution

### Custom Shaders

**IMPORTANT**: par-term has TWO separate shader systems — **background shaders** (`custom_shader`) and **cursor shaders** (`cursor_shader`). Do not confuse them when debugging.

See `docs/CUSTOM_SHADERS.md` for full shader documentation including uniforms, creation, and debugging.

**Key rules**:
- Develop shaders in `~/.config/par-term/shaders/` first; only move to repo `shaders/` when ready for distribution
- Transpiled WGSL written to `/tmp/par_term_<name>_shader.wgsl` for debugging
- When debugging one shader type, temporarily disable the other

### Modifying Rendering
- Cell backgrounds: `par-term-render/src/cell_renderer/` + `par-term-render/src/shaders/cell_bg.wgsl`
- Text rendering: `par-term-render/src/cell_renderer/` + `par-term-render/src/shaders/cell_text.wgsl`
- Scrollbar: `par-term-render/src/scrollbar.rs` + `par-term-render/src/shaders/scrollbar.wgsl`

### Debugging PTY Issues
- Enable logging: `RUST_LOG=debug cargo run`
- Check `TerminalManager::read()` and `write()` for I/O errors

## Testing Considerations

- Some tests require active PTY sessions and are marked `#[ignore]`
- Tests use `tempfile` for temporary configuration files
- Integration tests in `tests/` directory test config, terminal, and input modules

## Critical Gotchas

- Use `try_lock()` from sync contexts when accessing `tab.terminal` (tokio::sync::Mutex). For user-initiated operations (start/stop coprocess), use `blocking_lock()`. See MEMORY.md for details.
- `log::info!()` etc. go to stdout, NOT the debug log — use `crate::debug_info!()` macros instead
- The core library (`par-term-emu-core-rust`) has a `CoprocessManager` wired into the PTY reader thread; don't create separate managers in the frontend
- **Single rendering path (pane)**: There is ONE rendering path — all rendering goes through `render_split_panes_with_data()` → `CellRenderer::build_pane_instance_buffers()` in `pane_render/mod.rs`. The `build_instance_buffers()` method in `instance_buffers.rs` is only used by the shader intermediate texture path (`render_to_texture` / `render_to_view`). Per-cell overlays (search highlights, URL detection, prettifier substitution) are applied to `pane_data[].cells` AFTER `gather_pane_render_data()` in `gpu_submit.rs`.
- **Render 3-phase ordering**: Cursor overlays MUST render in phase 3 (after text), otherwise beam/underline cursors are hidden under text glyphs. All three callers use `emit_three_phase_draw_calls()` in `render.rs` — the single source of truth for draw call sequencing.

## Docs Reference (docs/)

| Topic | File |
|-------|------|
| Architecture overview | `docs/ARCHITECTURE.md` |
| Crate dependency structure | `docs/CRATE_STRUCTURE.md` |
| Concurrency / locking | `docs/CONCURRENCY.md` |
| Mutex patterns reference | `docs/MUTEX_PATTERNS.md` |
| State lifecycle | `docs/STATE_LIFECYCLE.md` |
| Custom shaders (background + cursor) | `docs/CUSTOM_SHADERS.md` |
| Included shader gallery | `docs/SHADERS.md` |
| GPU compositor / render layers | `docs/COMPOSITOR.md` |
| Session save/restore | `docs/SESSION_MANAGEMENT.md` |
| Session logging | `docs/SESSION_LOGGING.md` |
| Snippets & actions | `docs/SNIPPETS.md` |
| Keyboard shortcuts | `docs/KEYBOARD_SHORTCUTS.md` |
| Logging & debug | `docs/LOGGING.md` |
| SSH support | `docs/SSH.md` |
| Config reference | `docs/CONFIG_REFERENCE.md` |
| ACP harness | `docs/ACP_HARNESS.md` |
| Troubleshooting | `docs/TROUBLESHOOTING.md` |
| Getting started guide | `docs/GETTING_STARTED.md` |
| Quick start: fonts | `docs/QUICK_START_FONTS.md` |
| Public API index | `docs/API.md` |
| Environment variables | `docs/ENVIRONMENT_VARIABLES.md` |
| Enterprise deployment | `docs/ENTERPRISE_DEPLOYMENT.md` |
| Automation (triggers, coprocesses) | `docs/AUTOMATION.md` |
| Content prettifier | `docs/PRETTIFIER.md` |
| Assistant panel / ACP agents | `docs/ASSISTANT_PANEL.md` |
| Split tabs | `docs/TABS.md` |
| Window management | `docs/WINDOW_MANAGEMENT.md` |
| Window arrangements | `docs/ARRANGEMENTS.md` |
| Profiles | `docs/PROFILES.md` |
| Accessibility | `docs/ACCESSIBILITY.md` |
| Self-update | `docs/SELF_UPDATE.md` |
| Mouse features | `docs/MOUSE_FEATURES.md` |
| Copy mode | `docs/COPY_MODE.md` |
| Search | `docs/SEARCH.md` |
| Status bar | `docs/STATUS_BAR.md` |
| Badges | `docs/BADGES.md` |
| Integrations | `docs/INTEGRATIONS.md` |
| Command history | `docs/COMMAND_HISTORY.md` |
| Command separators | `docs/COMMAND_SEPARATORS.md` |
| File transfers | `docs/FILE_TRANSFERS.md` |
| Paste special | `docs/PASTE_SPECIAL.md` |
| Preferences import/export | `docs/PREFERENCES_IMPORT_EXPORT.md` |
| Progress bars | `docs/PROGRESS_BARS.md` |
| Scrollback buffer | `docs/SCROLLBACK.md` |
| Semantic history | `docs/SEMANTIC_HISTORY.md` |

## Supplemental Memory Notes

Key rendering root-cause findings are preserved in `MEMORY.md` (auto-memory file loaded by Claude Code) and inline in the `## Critical Gotchas` section above. The two most important areas to know before touching rendering code:

- **Cursor rendering**: 3-phase draw order (bgs → text → cursor overlays) enforced via `emit_three_phase_draw_calls()`. Hollow cursor opacity is independent of window opacity.
- **Block characters (▄/▀)**: Both halves rendered entirely via the text pipeline; bg pipeline emits a full-height quad that text overwrites. No partial-cell seam between pipelines.

## Quick Debugging Checklist by Category

**Rendering issue (wrong color, invisible element, cursor problem):**
1. All rendering goes through `pane_render/mod.rs` → `emit_three_phase_draw_calls()` in `render.rs`
2. Check 3-phase ordering: bgs → text → cursor overlays
3. For per-cell overlays: modify `pane_data[].cells` in `gpu_submit.rs` after `gather_pane_render_data()`
4. Use `make run-debug` and `make tail-log` with `crate::debug_info!("RENDER", ...)`

**Session restore issue (shell dies, wrong CWD):**
1. Single-pane tabs must NOT call `restore_pane_layout()` — check `src/session/capture.rs`
2. `pane_layout = None` for Leaf nodes, `Some(...)` only for Split roots

**Tab bar context menu inline mode (dismisses immediately):**
1. Add `*_activated_frame: u64` field to `TabBarUI`
2. Store `ui.ctx().cumulative_frame_nr()` on activation
3. Guard click-outside with `&& current_frame > self.*_activated_frame`
4. If opening an egui Popup: also add `&& !self.*_picking` to click-outside guard

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **par-term** (10114 symbols, 24289 relationships, 300 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## When Debugging

1. `gitnexus_query({query: "<error or symptom>"})` — find execution flows related to the issue
2. `gitnexus_context({name: "<suspect function>"})` — see all callers, callees, and process participation
3. `READ gitnexus://repo/par-term/process/{processName}` — trace the full execution flow step by step
4. For regressions: `gitnexus_detect_changes({scope: "compare", base_ref: "main"})` — see what your branch changed

## When Refactoring

- **Renaming**: MUST use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` first. Review the preview — graph edits are safe, text_search edits need manual review. Then run with `dry_run: false`.
- **Extracting/Splitting**: MUST run `gitnexus_context({name: "target"})` to see all incoming/outgoing refs, then `gitnexus_impact({target: "target", direction: "upstream"})` to find all external callers before moving code.
- After any refactor: run `gitnexus_detect_changes({scope: "all"})` to verify only expected files changed.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept | `gitnexus_query({query: "auth validation"})` |
| `context` | 360-degree view of one symbol | `gitnexus_context({name: "validateUser"})` |
| `impact` | Blast radius before editing | `gitnexus_impact({target: "X", direction: "upstream"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({scope: "staged"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/par-term/context` | Codebase overview, check index freshness |
| `gitnexus://repo/par-term/clusters` | All functional areas |
| `gitnexus://repo/par-term/processes` | All execution flows |
| `gitnexus://repo/par-term/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing any code modification task, verify:
1. `gitnexus_impact` was run for all modified symbols
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->
