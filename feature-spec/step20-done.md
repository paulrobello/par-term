# Step 20: Integration Verification & Findings

## Summary

Runtime verification of the Content Prettifier implementation (steps 1–19) against the expectations in `spec.md`. This step documents what has been built, what passes automated testing, and what critical wiring gaps remain before the prettifier is functional in a live terminal session.

## Verification Environment

- **Branch**: `markdown-rendering`
- **Date**: 2026-02-22
- **Build**: `cargo build --release` — clean compilation, no warnings
- **Tests**: `cargo test` — 1,557 tests pass (0 failures), ~667 prettifier-related

## Findings

### What Is Fully Implemented (Code Exists & Unit Tests Pass)

| Component | Files | Status |
|-----------|-------|--------|
| `ContentDetector` / `ContentRenderer` traits | `src/prettifier/traits.rs` | Complete |
| `RegexDetector` standard implementation | `src/prettifier/regex_detector.rs` | Complete |
| `DetectionRule` model (weight, scope, strength) | `src/prettifier/types.rs` | Complete |
| `RendererRegistry` | `src/prettifier/registry.rs` | Complete |
| `PrettifierPipeline` orchestrator | `src/prettifier/pipeline.rs` | Complete |
| `DualViewBuffer` (source/rendered) | `src/prettifier/buffer.rs` | Complete |
| `BoundaryDetector` (OSC 133, blank lines, debounce) | `src/prettifier/boundary.rs` | Complete |
| `RenderCache` with LRU eviction | `src/prettifier/cache.rs` | Complete |
| `GutterManager` (indicators, hit-testing) | `src/prettifier/gutter.rs` | Complete |
| `ConfigBridge` (YAML config to runtime config) | `src/prettifier/config_bridge.rs` | Complete |
| Claude Code integration (expand/collapse tracking) | `src/prettifier/claude_code.rs` | Complete |
| External command renderers | `src/prettifier/custom_renderers.rs` | Complete |
| Config system with profile override resolution | `src/config/prettifier.rs` (937 lines) | Complete |
| Settings UI tab (7 collapsible sections, 30+ controls) | `src/settings_ui/prettifier_tab.rs` (800+ lines) | Complete |
| Settings sidebar entry with search keywords | `src/settings_ui/sidebar.rs` | Complete |
| `Prettify` trigger action type | `src/config/automation.rs`, `src/app/triggers.rs` | Complete |

### Built-In Detectors (11 formats)

| Detector | File | Rules | Status |
|----------|------|-------|--------|
| Markdown | `src/prettifier/detectors/markdown.rs` | 20+ rules | Unit tests pass |
| JSON | `src/prettifier/detectors/json.rs` | Key/value, braces, brackets | Unit tests pass |
| YAML | `src/prettifier/detectors/yaml.rs` | Key: value, dashes, `---` | Unit tests pass |
| TOML | `src/prettifier/detectors/toml.rs` | `[section]`, `key = value` | Unit tests pass |
| XML | `src/prettifier/detectors/xml.rs` | `<?xml`, `<tags>`, DOCTYPE | Unit tests pass |
| CSV | `src/prettifier/detectors/csv.rs` | Delimiter consistency, quoting | Unit tests pass |
| Diff | `src/prettifier/detectors/diff.rs` | `---`/`+++`/`@@`, `+`/`-` lines | Unit tests pass |
| Log | `src/prettifier/detectors/log.rs` | Timestamps, log levels | Unit tests pass |
| Diagrams | `src/prettifier/detectors/diagrams.rs` | Fenced block language tags | Unit tests pass |
| Stack Trace | `src/prettifier/detectors/stack_trace.rs` | Language-specific patterns | Unit tests pass |
| SQL Results | `src/prettifier/detectors/sql_results.rs` | Column headers, separators | Unit tests pass |

### Built-In Renderers (11 formats + 2 shared)

| Renderer | File | Status |
|----------|------|--------|
| Markdown | `src/prettifier/renderers/markdown.rs` | Unit tests pass |
| JSON | `src/prettifier/renderers/json.rs` | Unit tests pass |
| YAML | `src/prettifier/renderers/yaml.rs` | Unit tests pass |
| TOML | `src/prettifier/renderers/toml.rs` | Unit tests pass |
| XML | `src/prettifier/renderers/xml.rs` | Unit tests pass |
| CSV | `src/prettifier/renderers/csv.rs` | Unit tests pass |
| Diff | `src/prettifier/renderers/diff.rs` | Unit tests pass |
| Log | `src/prettifier/renderers/log.rs` | Unit tests pass |
| Diagrams | `src/prettifier/renderers/diagrams.rs` | Unit tests pass |
| Stack Trace | `src/prettifier/renderers/stack_trace.rs` | Unit tests pass |
| SQL Results | `src/prettifier/renderers/sql_results.rs` | Unit tests pass |
| **Shared: Table** | `src/prettifier/renderers/table.rs` | Unit tests pass |
| **Shared: Tree** | `src/prettifier/renderers/tree_renderer.rs` | Unit tests pass |

---

## Critical Finding: Pipeline Not Connected to Live Terminal

The prettifier system is **architecturally complete** but **functionally disconnected** from the terminal output flow. The entire library (~12,000 lines) works correctly in unit tests but has no effect at runtime.

### Gap 1: Pipeline Never Instantiated

In `src/tab/mod.rs`, the `prettifier` field is always set to `None`:

```
line 371:    pub prettifier: Option<PrettifierPipeline>,
line 575:            prettifier: None,    // Tab::new()
line 805:            prettifier: None,    // Tab::from_config()
line 1515:           prettifier: None,    // Tab clone
```

`PrettifierPipeline::new()` is only called in unit tests — never in production code.

### Gap 2: Terminal Output Never Fed to Pipeline

`pipeline.process_output(line, row)` is the entry point for feeding terminal output to the detection system. It is defined in `src/prettifier/pipeline.rs:155` but **only called in tests**.

The render loop in `src/app/window_state.rs` reads terminal cells but never passes them to the prettifier:

```
line 1606–1622: Skeleton code checks `tab.prettifier` (always None):
  - on_alt_screen_change() — would be called, but pipeline is None
  - check_debounce() — would be called, but pipeline is None
  - process_output() — NOT CALLED at all, even in the skeleton
```

### Gap 3: OSC 133 Shell Markers Not Forwarded

`pipeline.on_command_start()` and `pipeline.on_command_end()` are defined in `src/prettifier/pipeline.rs:165-176` but **never called** from the terminal/shell integration layer. These are critical for `detection.scope: "command_output"` mode.

### Gap 4: Rendered Content Not Displayed

The cell renderer (`src/cell_renderer/`) and graphics renderer (`src/renderer/`) have **zero references** to any prettifier type. There is no code path that:

1. Checks if a row belongs to a prettified block
2. Substitutes raw terminal cells with rendered content from `DualViewBuffer`
3. Draws gutter indicators alongside terminal content
4. Handles the visual toggle between source and rendered views

### Gap 5: Keyboard/Mouse Handlers Guarded by None

The integration points in input handlers exist but are guarded by `if let Some(ref mut pipeline) = tab.prettifier`, which is always `None`:

- `src/app/mouse_events.rs:341,351` — gutter click handling (never reached)
- `src/app/input_events.rs:1152` — global toggle keybinding (never reached)
- `src/app/triggers.rs:327,393,429` — trigger-based prettify dispatch (never reached)
- `src/app/text_selection.rs:349` — prettifier copy text (never reached)

---

## What Would Be Required to Activate the System

### Step A: Instantiate the Pipeline

In `Tab::new()` (and `from_config()`, clone), create an actual `PrettifierPipeline`:

```rust
// In Tab::new():
let prettifier = if config.enable_prettifier {
    let resolved = resolve_prettifier_config(
        config.enable_prettifier,
        &config.content_prettifier,
        None, // profile override
        None,
    );
    let registry = build_default_registry(&resolved);
    let renderer_config = RendererConfig::from(&resolved);
    Some(PrettifierPipeline::new(
        resolved.into(),
        registry,
        renderer_config,
    ))
} else {
    None
};
```

### Step B: Feed Terminal Output

In the render loop (`window_state.rs`), after reading terminal cells, extract text lines and feed them to the pipeline:

```rust
// After terminal read:
if let Some(ref mut pipeline) = tab.prettifier {
    for (row, line) in new_output_lines.iter().enumerate() {
        pipeline.process_output(line, absolute_row + row);
    }
}
```

### Step C: Hook OSC 133 Markers

When the terminal emits OSC 133 command start/end markers, forward them:

```rust
// On OSC 133 "C" (command started):
if let Some(ref mut pipeline) = tab.prettifier {
    pipeline.on_command_start(&command_text);
}

// On OSC 133 "D" (command ended):
if let Some(ref mut pipeline) = tab.prettifier {
    pipeline.on_command_end();
}
```

### Step D: Display Rendered Content

Modify the cell renderer to check if rows are prettified and substitute rendered cells:

```rust
// In cell rendering:
if let Some(ref pipeline) = tab.prettifier {
    if let Some(rendered) = pipeline.rendered_block_at_row(row) {
        // Use rendered.styled_lines instead of raw terminal cells
        // Draw gutter indicator
    }
}
```

---

## Spec Acceptance Criteria Status

### Framework (spec lines 1434–1451)
- [x] ContentDetector and ContentRenderer traits implemented
- [x] RegexDetector with weighted confidence scoring implemented
- [x] All built-in detectors powered by RegexDetector with inspectable rules
- [x] Built-in regex rules loaded and merged with user-defined rules
- [x] Users can add/disable/override regex rules via config
- [x] Users can create new detectors from regex rules alone via config
- [x] Prettify action type registered in trigger system
- [x] Trigger-based prettifying dispatches directly to renderer
- [x] `prettify_format: "none"` suppresses auto-detection
- [x] Trigger `command_filter` scopes to specific commands
- [x] Block-scoped triggers with `prettify_block_end` supported
- [x] Renderer registry supports dynamic registration
- [x] Source/rendered dual-view maintained (DualViewBuffer)
- [ ] **Global toggle works at runtime** — pipeline is None
- [ ] **Per-block toggle works at runtime** — pipeline is None
- [ ] **Gutter indicators display** — no cell renderer integration
- [ ] **Copy operations work** — pipeline is None
- [ ] **Zero impact on non-prettified output** — not measurable (no runtime path)

### Configuration & Profiles (spec lines 1453–1461)
- [x] `enable_prettifier` setting exists with default `true`
- [x] `enable_prettifier` can be overridden per-profile
- [x] Profile-level overrides take precedence over global
- [x] All sub-settings follow global-to-profile override chain
- [x] Settings UI shows toggle with dynamic subtitle
- [x] Settings UI indicates inherited vs overridden values
- [x] Profile editor includes tri-state toggle and overrides panel
- [ ] **Profile switching applies prettifier settings at runtime** — no runtime pipeline

### Phase 1 — Markdown & Diagrams (spec lines 1463–1468)
- [ ] **Markdown auto-detected and rendered in live terminal** — pipeline not connected
- [ ] **Mermaid rendered as inline graphics** — pipeline not connected
- [ ] **3+ diagram languages via Kroki** — pipeline not connected
- [ ] **Diagram rendering async with placeholder** — pipeline not connected
- [ ] **Ctrl+O expand triggers prettifier** — pipeline not connected

### Phase 2 — Structured Data & Diffs (spec lines 1470–1475)
- [ ] **JSON auto-detected and rendered live** — pipeline not connected
- [ ] **YAML/TOML auto-detected live** — pipeline not connected
- [ ] **Diff output detected and colored live** — pipeline not connected
- [ ] **Log output detected with coloring** — pipeline not connected
- [ ] **CSV/TSV rendered as tables** — pipeline not connected

### Extensibility (spec lines 1477–1487)
- [x] Custom renderers registerable via config
- [x] Custom diagram languages registerable via config
- [x] Users can add regex rules to existing detectors
- [x] Users can disable/override built-in rules
- [x] Trigger-based prettifying in Automation settings
- [x] Settings UI shows all rules with enable/disable
- [x] Settings UI search keywords work
- [x] Rendering respects active theme (in unit tests)
- [x] Settings fully per-profile-capable
- [x] Settings UI provides full configuration access
- [x] Adding a new renderer only requires rules + ContentRenderer impl

---

## Conclusion

**Steps 1–19 delivered a complete, well-tested prettifier library.** All architectural components exist and work correctly in isolation (~667 unit tests pass). The remaining work is **integration wiring** — connecting the library to the live terminal output flow so detection, rendering, and display happen at runtime.

The four required integration points are:
1. **Instantiate** `PrettifierPipeline` from config during tab creation
2. **Feed** terminal output lines to `process_output()` in the render loop
3. **Forward** OSC 133 shell markers to `on_command_start()` / `on_command_end()`
4. **Display** rendered content by having the cell renderer consult `DualViewBuffer`

These are the next steps to make the prettifier functional in a live session.
