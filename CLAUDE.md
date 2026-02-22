# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

par-term is a cross-platform GPU-accelerated terminal emulator frontend built in Rust. It uses the [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) library for VT sequence processing, PTY management, and inline graphics protocols (Sixel, iTerm2, Kitty). The frontend provides GPU-accelerated rendering via wgpu with custom WGSL shaders, including support for custom post-processing shaders (Ghostty/Shadertoy-compatible GLSL).

**Language**: Rust (Edition 2024)
**Platform**: Cross-platform (macOS, Linux, Windows)
**Graphics**: wgpu (Vulkan/Metal/DirectX 12)
**Version**: 0.20.0

## Development Commands

### Build & Run

**IMPORTANT**: Use `make build` / `make run` for day-to-day development. These use the `dev-release` profile (opt-level 3, thin LTO, 16 codegen-units) which compiles in ~30-40s with ~95% of full release performance. Only use `make build-debug` when you need debug symbols for stepping through code, and `make build-full` / `make release` for distribution builds.

```bash
make build          # Dev-release build (optimized, thin LTO — ~30-40s, preferred)
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

See `src/config.rs` for all available settings and defaults.

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
6. Run `cargo check` to verify

## Common Development Workflows

### Adding a New Configuration Option
1. Add field to `Config` struct in `src/config.rs` with `#[serde(default = "default_my_option")]`
2. Update `Default` impl
3. Use config value in relevant component
4. **REQUIRED**: Add UI controls in the appropriate `src/settings_ui/*_tab.rs`
   - Set `settings.has_changes = true` and `*changes_this_frame = true` on change
5. **REQUIRED**: Update search keywords in `src/settings_ui/sidebar.rs` → `tab_search_keywords()`

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
- Cell backgrounds: `src/cell_renderer/` + `src/shaders/cell_bg.wgsl`
- Text rendering: `src/cell_renderer/` + `src/shaders/cell_text.wgsl`
- Scrollbar: `src/scrollbar.rs` + `src/shaders/scrollbar.wgsl`

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

<!-- gitnexus:start -->
# GitNexus MCP

This project is indexed by GitNexus as **par-term** (4952 symbols, 16866 relationships, 300 execution flows).

GitNexus provides a knowledge graph over this codebase — call chains, blast radius, execution flows, and semantic search.

## Always Start Here

For any task involving code understanding, debugging, impact analysis, or refactoring, you must:

1. **Read `gitnexus://repo/{name}/context`** — codebase overview + check index freshness
2. **Match your task to a skill below** and **read that skill file**
3. **Follow the skill's workflow and checklist**

> If step 1 warns the index is stale, run `npx gitnexus analyze` in the terminal first.

## Skills

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/refactoring/SKILL.md` |

## Tools Reference

| Tool | What it gives you |
|------|-------------------|
| `query` | Process-grouped code intelligence — execution flows related to a concept |
| `context` | 360-degree symbol view — categorized refs, processes it participates in |
| `impact` | Symbol blast radius — what breaks at depth 1/2/3 with confidence |
| `detect_changes` | Git-diff impact — what do your current changes affect |
| `rename` | Multi-file coordinated rename with confidence-tagged edits |
| `cypher` | Raw graph queries (read `gitnexus://repo/{name}/schema` first) |
| `list_repos` | Discover indexed repos |

## Resources Reference

Lightweight reads (~100-500 tokens) for navigation:

| Resource | Content |
|----------|---------|
| `gitnexus://repo/{name}/context` | Stats, staleness check |
| `gitnexus://repo/{name}/clusters` | All functional areas with cohesion scores |
| `gitnexus://repo/{name}/cluster/{clusterName}` | Area members |
| `gitnexus://repo/{name}/processes` | All execution flows |
| `gitnexus://repo/{name}/process/{processName}` | Step-by-step trace |
| `gitnexus://repo/{name}/schema` | Graph schema for Cypher |

## Graph Schema

**Nodes:** File, Function, Class, Interface, Method, Community, Process
**Edges (via CodeRelation.type):** CALLS, IMPORTS, EXTENDS, IMPLEMENTS, DEFINES, MEMBER_OF, STEP_IN_PROCESS

```cypher
MATCH (caller)-[:CodeRelation {type: 'CALLS'}]->(f:Function {name: "myFunc"})
RETURN caller.name, caller.filePath
```

<!-- gitnexus:end -->
