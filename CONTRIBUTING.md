# Contributing to par-term

Guidelines for contributing to par-term, a cross-platform GPU-accelerated terminal emulator built in Rust.

## Table of Contents

- [Overview](#overview)
- [Development Setup](#development-setup)
  - [Prerequisites](#prerequisites)
  - [Platform-Specific Requirements](#platform-specific-requirements)
  - [Cloning and Initial Build](#cloning-and-initial-build)
- [Build Commands](#build-commands)
- [Running the Application](#running-the-application)
- [Testing](#testing)
- [Code Quality](#code-quality)
- [Debug Logging](#debug-logging)
- [Architecture Overview](#architecture-overview)
- [Code Style](#code-style)
- [macOS-Specific Notes](#macos-specific-notes)
- [Linux-Specific Notes](#linux-specific-notes)
- [Windows-Specific Notes](#windows-specific-notes)
- [Configuration](#configuration)
- [Common Development Workflows](#common-development-workflows)
  - [Adding a Configuration Option](#adding-a-configuration-option)
  - [Adding a Keyboard Shortcut](#adding-a-keyboard-shortcut)
- [Sub-Crate Architecture](#sub-crate-architecture)
  - [Dependency Layers](#dependency-layers)
  - [Version Bump Checklist](#version-bump-checklist)
- [PR Guidelines](#pr-guidelines)
- [Commit Message Format](#commit-message-format)
- [Related Documentation](#related-documentation)

## Overview

par-term is a cross-platform GPU-accelerated terminal emulator frontend written in Rust (Edition 2024). It uses the [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) library for VT sequence processing, PTY management, and inline graphics protocols (Sixel, iTerm2, Kitty). The frontend provides GPU-accelerated rendering via `wgpu` with custom WGSL shaders, including support for Ghostty/Shadertoy-compatible GLSL post-processing shaders.

**Language:** Rust (Edition 2024)
**License:** MIT
**Platforms:** macOS (Metal), Linux (Vulkan), Windows (DirectX 12)

## Development Setup

### Prerequisites

- **Rust stable toolchain** — install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Make** for build orchestration (available by default on macOS and most Linux distributions)
- **Git**

### Platform-Specific Requirements

#### macOS

No additional system libraries are required. The Metal GPU backend is used automatically.

#### Linux

Install the required X11/Wayland and SSL development libraries before building:

```bash
# Debian / Ubuntu
sudo apt-get install \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxkbcommon-dev \
  libssl-dev \
  pkg-config

# Fedora / RHEL
sudo dnf install \
  libxcb-devel \
  libxkbcommon-devel \
  openssl-devel \
  pkg-config
```

#### Windows

No additional steps are required beyond a working Rust stable toolchain. The DirectX 12 backend is used automatically.

### Cloning and Initial Build

```bash
git clone https://github.com/paulrobello/par-term.git
cd par-term
make build
```

The first build downloads and compiles all dependencies, which takes several minutes. Subsequent incremental builds complete in roughly 30-40 seconds with `make build`.

## Build Commands

> **Use `make build` for all day-to-day development.** The `dev-release` Cargo profile (opt-level 3, thin LTO, 16 codegen-units) delivers approximately 95% of full-release performance with a much shorter compile time. Only switch to `make build-full` when preparing distribution binaries.

| Command | Cargo Profile | Compile Time | Description |
|---------|---------------|--------------|-------------|
| `make build` | `dev-release` | ~30-40s | Optimized, thin LTO. **Preferred for development.** |
| `make build-full` | `release` | ~3 min | Full LTO, single codegen unit. Use for distribution. |
| `make build-debug` | `debug` | Fast | Unoptimized, includes debug symbols. Use when stepping through code with a debugger. |
| `make release` | `release` | ~3 min | Alias for `make build-full`. |

Output binary locations:

| Profile | Binary |
|---------|--------|
| `dev-release` | `target/dev-release/par-term` |
| `release` | `target/release/par-term` |
| `debug` | `target/debug/par-term` |

## Running the Application

```bash
make run            # Run in dev-release mode (preferred)
make run-release    # Run in full release mode
```

> **Warning:** When stopping a running debug instance, never use `killall par-term`. This kills all par-term processes on the system, including the terminal session you are working in. Use `pkill -f "target/debug/par-term"` or kill by specific PID instead.

When testing your code changes, always run from the build output (`make run`, `cargo run`) rather than from an installed `.app` bundle. An installed bundle will not contain your local changes.

## Testing

```bash
make test                        # Run all workspace tests
make test-one TEST=test_name     # Run a specific test by name
make test-verbose                # Run tests with captured output printed to the terminal
cargo test -- --include-ignored  # Run all tests, including PTY-dependent ones
```

### PTY-Dependent Tests

Some tests require an active PTY session and are marked `#[ignore]`. The default `make test` run skips them. To include them explicitly:

```bash
cargo test -- --include-ignored
```

### Integration Tests

Integration tests live in the `tests/` directory and cover configuration loading, terminal state, and input handling. They use the `tempfile` crate for isolated temporary files and do not modify real user configuration.

### Specialized Testing Targets

```bash
make test-fonts         # Comprehensive text shaping and font rendering test suite
make benchmark-shaping  # Text shaping performance benchmarks
make test-text-shaping  # Run font tests and benchmarks together
make test-graphics      # Graphics protocol tests with DEBUG_LEVEL=4 logging
make test-animations    # Kitty animation protocol tests
```

## Code Quality

Run the full quality suite before every commit:

```bash
make checkall    # Format, lint, and test — required before committing
```

Individual targets:

| Command | Tool | Description |
|---------|------|-------------|
| `make fmt` | `cargo fmt` | Format all code with rustfmt |
| `make fmt-check` | `cargo fmt -- --check` | Check formatting without modifying files |
| `make lint` | `cargo clippy -- -D warnings` | Lint with warnings treated as errors |
| `make lint-all` | `cargo clippy --all-targets -- -D warnings` | Lint all targets (bins, tests, examples, benches) |
| `make check` | `cargo check` | Type-check without producing a binary |
| `make check-all` | `cargo check --all-targets` | Type-check all targets |

### Pre-Commit and CI Targets

```bash
make pre-commit    # fmt-check + lint + test — run this locally before pushing
make ci            # fmt-check + lint-all + test + check-all — equivalent to CI checks
make all           # fmt + lint + test + build
```

### Optional Tools

```bash
make coverage      # Test coverage report via cargo-tarpaulin (must be installed separately)
make audit         # Dependency security audit via cargo-audit (must be installed separately)
make profile       # CPU profiling with cargo-flamegraph (must be installed separately)
make doc-open      # Generate and open rustdoc documentation in a browser
```

## Debug Logging

par-term uses a custom debug macro system that writes to `/tmp/par_term_debug.log`. The standard `log` crate macros (`log::info!()`, `log::warn!()`, etc.) write to stdout and will not appear in the debug log file.

### Running with Debug Output

In one terminal, start par-term with debug logging enabled:

```bash
make run-debug    # DEBUG_LEVEL=3 — standard debug output → /tmp/par_term_debug.log
make run-trace    # DEBUG_LEVEL=4 — most verbose, includes trace events
```

In a second terminal, monitor the log in real time:

```bash
make tail-log          # Stream all log entries
make watch-graphics    # Stream only graphics-related log entries
```

### Debug Macro Reference

Always use these project-specific macros when adding log output to source code:

```rust
crate::debug_error!("CATEGORY", "message {}", var);  // DEBUG_LEVEL ≥ 1 — errors
crate::debug_info!("CATEGORY", "message {}", var);   // DEBUG_LEVEL ≥ 2 — informational
crate::debug_log!("CATEGORY", "message");            // DEBUG_LEVEL ≥ 3 — standard debug
crate::debug_trace!("CATEGORY", "message");          // DEBUG_LEVEL = 4 — trace/verbose
```

Common log categories: `TAB`, `TAB_BAR`, `TAB_ACTION`, `MOUSE`, `RENDER`, `SHADER`, `TERMINAL`, `APP`

For the full logging documentation, see `docs/LOGGING.md`.

### ACP Agent Debugging

When debugging the Assistant Panel (Claude/Ollama ACP integration):

```bash
make acp-harness ARGS="--list-agents"   # Verify agent discovery and config loading
make acp-smoke                           # Run the reproducible shader prompt smoke test
```

See `docs/ACP_HARNESS.md` for full usage details and transcript capture.

## Architecture Overview

For a detailed description of the codebase structure, data flow, threading model, and GPU rendering pipeline, see `docs/ARCHITECTURE.md`.

**Key layers:**

```
App (src/app/)
  └── Terminal (src/terminal/)
        └── Renderer (src/renderer/, src/cell_renderer/)
              └── GPU Shaders (src/shaders/)
```

**Data flow:**

```
Window Events → Input Handler → PTY → VT Parser → Styled Segments
  → GPU Renderer (three passes: cells → graphics → egui overlay)
```

**Key implementation patterns:**

- Tokio runtime for async PTY I/O; sync wrappers bridge to the winit event loop
- Glyph atlas with instanced rendering for efficient text output
- RGBA texture caching for inline graphics (Sixel, iTerm2, Kitty)
- Scrollback buffer with viewport offset rendering

## Code Style

### File Size Targets

- **Target:** Keep all source files under 500 lines.
- **Refactor threshold:** Any file exceeding 800 lines must be split into sub-modules.
- Follow the existing patterns: `src/app/`, `src/terminal/`, `src/cell_renderer/`.
- Centralize constants; avoid magic numbers scattered across files.
- Prefer composition over duplication; create helper traits for shared functionality.

### Documentation Standards

Use `///` doc comments for all public API items (structs, enums, traits, public functions and methods). Use `//` inline comments for implementation notes and non-obvious logic. Focus comments on *why* rather than *what* — the code already shows what is happening.

```rust
/// Renders all visible cells for the current viewport into the GPU instance buffer.
///
/// Must be called once per frame before `submit_render_pass`.
pub fn upload_cells(&mut self, cells: &[StyledCell]) { ... }

// Skip zero-width cells — they have no visual representation and will corrupt
// the atlas packing algorithm if submitted.
if cell.width == 0 {
    continue;
}
```

### Concurrency Rules

- Use `try_lock()` from synchronous contexts when accessing `tab.terminal` (`tokio::sync::Mutex`).
- For user-initiated operations (e.g., start/stop coprocess), use `blocking_lock()` instead.
- Never call `process::exit()` directly — use the graceful shutdown path.

## macOS-Specific Notes

- **GPU backend:** Metal
- **Platform-specific code:** `src/macos_metal.rs`
- **App bundle commands:**
  ```bash
  make bundle          # Create par-term.app in target/release/bundle/
  make bundle-install  # Install .app to /Applications, binary to ~/.cargo/bin, and ACP bridge
  make run-bundle      # Build and launch the .app (shows the correct dock icon)
  ```
- When testing code changes, always run from the build output (`make run`) rather than the installed `.app` bundle. The bundle will not reflect your local changes.

## Linux-Specific Notes

- **GPU backend:** Vulkan
- Both X11 and Wayland display servers are supported.
- Install the required system libraries listed in [Platform-Specific Requirements](#platform-specific-requirements) before your first build.

## Windows-Specific Notes

- **GPU backend:** DirectX 12
- No additional system libraries are required beyond the Rust stable toolchain.

## Configuration

The configuration file uses the XDG base directory convention:

- **Linux / macOS:** `~/.config/par-term/config.yaml`
- **Windows:** `%APPDATA%\par-term\config.yaml`

See `src/config.rs` for all available settings and their defaults. For a human-readable reference, see `docs/CONFIG_REFERENCE.md`.

To generate an example configuration file:

```bash
make config-example    # Writes config.yaml.example to the project root
```

## Common Development Workflows

### Adding a Configuration Option

1. Add the field to the `Config` struct in `src/config.rs` with a serde default attribute:
   ```rust
   #[serde(default = "default_my_option")]
   pub my_option: MyType,
   ```
2. Implement the `default_my_option` function and update the `Default` impl.
3. Use the config value in the relevant component.
4. **Required:** Add UI controls in the appropriate settings tab (`src/settings_ui/*_tab.rs`). Set `settings.has_changes = true` and `*changes_this_frame = true` when the value changes.
5. **Required:** Update the search keywords in `src/settings_ui/sidebar.rs` inside `tab_search_keywords()`.

### Adding a Keyboard Shortcut

1. Add key handling in `src/app/input_events.rs`.
2. If the shortcut generates a terminal sequence, add sequence generation in `src/input.rs` via `InputHandler`.

For snippet or action keybindings, see `docs/SNIPPETS.md`. Key points:

- Snippets use `snippet:<id>`, actions use `action:<id>` as keybinding action names.
- Bindings are auto-generated during config load via `generate_snippet_action_keybindings()`.
- `execute_keybinding_action()` in `input_events.rs` handles execution.

## Sub-Crate Architecture

par-term is organized as a Cargo workspace with 13 sub-crates plus the root application crate. The dependency graph is a strict layered DAG.

### Dependency Layers

| Layer | Crates | Notes |
|-------|--------|-------|
| **Layer 0** | `par-term-acp`, `par-term-ssh`, `par-term-mcp` | No internal deps; bump in any order |
| **Layer 1** | `par-term-config` | Foundation; depends only on external `par-term-emu-core-rust` |
| **Layer 2** | `par-term-fonts`, `par-term-input`, `par-term-keybindings`, `par-term-scripting`, `par-term-settings-ui`, `par-term-terminal`, `par-term-tmux`, `par-term-update` | All depend on `par-term-config` |
| **Layer 3** | `par-term-render` | Depends on `par-term-config` and `par-term-fonts` |
| **Layer 4** | `par-term` (root) | Depends on all of the above |

### Version Bump Checklist

When bumping sub-crate versions for crates.io publishing, follow dependency order. Update both the crate's own `version` field and any `version = "..."` in dependents' `Cargo.toml` references.

1. Bump `par-term-config` version and update references in all Layer 2/3 crates
2. Bump Layer 0 crate versions
3. Bump Layer 2 crate versions
4. Bump `par-term-render` version and update its `par-term-fonts` reference
5. Update all version references in root `Cargo.toml`
6. Run `cargo check` to verify

## PR Guidelines

1. Fork the repository and create a feature branch from `main`.
2. Make atomic commits — one logical change per commit.
3. Run `make checkall` and confirm it passes without errors before pushing.
4. Open a pull request against `main`.
5. Describe your changes clearly in the PR body. The PR title and description become the squash commit message on main.
6. PRs are merged via **squash merge** to keep the main branch history linear. Feature branches are deleted after merge.

### What "Atomic" Means

Each commit should:
- Compile and pass all tests on its own
- Have a single, clear purpose that can be stated in one sentence
- Not mix refactoring with behavior changes

### Before Submitting

- `make checkall` passes without errors or warnings
- New behavior is covered by tests where practical
- Public API additions carry `///` doc comments
- Settings additions update both the settings tab and the search keywords

## Commit Message Format

```
<type>(<scope>): <subject>

[optional body — 72-character wrapped lines explaining what and why]

[optional footer — e.g., Closes #123]
```

### Commit Types

| Type | When to use |
|------|-------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `perf` | A performance improvement |
| `refactor` | Code restructuring with no behavior change |
| `test` | Adding or correcting tests |
| `docs` | Documentation only |
| `style` | Formatting or whitespace, no logic change |
| `chore` | Build system, dependency updates, tooling |

### Rules

- Subject line: maximum 50 characters, imperative mood ("add", "fix", "remove"), no trailing period.
- Body: wrap at 72 characters; explain *why*, not *what*.
- Footer: reference issues with `Closes #123` or `Fixes #456`.

### Examples

```
feat(tab-bar): add drag-and-drop reordering for tabs

Users can now drag tabs to reorder them within the tab bar.
Reorder state is persisted to the session arrangement file
so it survives restarts.

Closes #247
```

```
fix(renderer): clamp scrollback offset to valid range

A race between the PTY reader and the render thread could
produce a viewport offset larger than the scrollback buffer,
causing an out-of-bounds read in the cell upload path.
```

```
perf(render): pre-allocate GPU instance buffers

Avoids per-frame Vec reallocation for the cell and graphics
instance arrays by reserving capacity at startup based on
the initial terminal dimensions.
```

## Related Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — Full architecture, data flow, and threading model
- [docs/LOGGING.md](docs/LOGGING.md) — Custom debug macro system and log levels
- [docs/CUSTOM_SHADERS.md](docs/CUSTOM_SHADERS.md) — Background and cursor shader development
- [docs/CONFIG_REFERENCE.md](docs/CONFIG_REFERENCE.md) — All configuration options
- [docs/KEYBOARD_SHORTCUTS.md](docs/KEYBOARD_SHORTCUTS.md) — Default keybinding reference
- [docs/SNIPPETS.md](docs/SNIPPETS.md) — Snippet and action keybinding system
- [docs/ACP_HARNESS.md](docs/ACP_HARNESS.md) — ACP agent debugging harness
- [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) — Common issues and solutions
- [docs/DOCUMENTATION_STYLE_GUIDE.md](docs/DOCUMENTATION_STYLE_GUIDE.md) — Documentation conventions for this project
- [CLAUDE.md](CLAUDE.md) — Full AI assistant development context and workflow reference
