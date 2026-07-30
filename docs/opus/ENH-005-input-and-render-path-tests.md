# ENH-005 — Tests for `par-term-input` and the pane render path

> **Impact**: high · **Effort**: large · **Source**: AUDIT.md Code Quality test-coverage assessment;
> enabled QA-011 and ARC-004

## Goal

Add coverage to the two subsystems CLAUDE.md itself flags as highest-risk and which currently have none:
byte-exact encoding tests for `par-term-input`, and golden-image tests for the pane render path.

## Current State

Coverage across the workspace is **sharply bimodal**, so a single percentage misrepresents it. Well covered:
`par-term-config` (140 tests), `par-term-keybindings` (118 integration tests asserting exact escape bytes), the
GLSL→WGSL transpiler (19). At or near zero — and these are the load-bearing paths:

**`par-term-input`** — no tests **within the crate** across all four source files, despite being the live
winit-event → terminal-bytes path. It holds two of the repo's most complex functions:
`handle_key_event_with_mode` (complexity 67, `key_encoding.rs:76`) and a second at 52. Commit `c11e308e`
rewrote the root crate's `tests/input_tests.rs` to drive `par_term::input::KeyInput`, so the crate is exercised
*indirectly* — better than nothing, but the crate's own encoding matrix (modifier combinations × key types ×
terminal modes) is untested at the unit level.

**The pane render path** — `pane_render/mod.rs` (863 lines, `build_pane_instance_buffers` at complexity 75),
`text_instance_builder.rs`, `bg_instance_builder.rs`, and `instance_buffers.rs` have **no tests at all**. This is
the single designated rendering path for all normal terminal output.

Two audit findings exist *because* of that second gap, which is the argument for this item:

- **QA-011** — screenshots render through a separate, overlay-less, single-pane builder, so the CLI
  `--screenshot` hook and the MCP `terminal_screenshot` tool return frames missing split panes, search
  highlights, and URL underlines. Both are the project's *agent-operability verification hooks*, so automated
  visual checking silently misleads. Nothing caught it because nothing compares rendered output.
- **ARC-004** — batching per-pane GPU submits before suballocating the shared instance buffers renders garbage
  while compiling cleanly and passing every test, because no test inspects multi-pane pixel output.

Also uncovered: `src/session/capture.rs` (which ENH-003 depends on), `src/app/window_manager/` (~4,500 lines),
`src/app/tab_ops/` (~2,136), and `par-term-config/src/config/persistence.rs` (564 lines — and it holds the
atomic-save reference implementation QA-023 extracts from).

## Implementation

Two independent halves. **Do Part A first** — it is cheaper, needs no GPU, and pays off immediately.

### Part A — `par-term-input` encoding matrix

#### A1. Establish the contract

Read `par-term-input/src/key_encoding.rs:76` (`handle_key_event_with_mode`) and enumerate the axes it switches
on: key type (named/character/function/keypad), modifier set, and terminal mode (application cursor keys,
application keypad, Kitty keyboard protocol level, bracketed paste). The complexity of 67 *is* this matrix.

Do not invent expected bytes. Take them from the two authoritative sources already in the repo:
`par-term-keybindings/tests/keybinding_integration_tests.rs` (which already asserts exact escape bytes and has a
table-driven pattern at `:301-328,466-482` worth copying), and the `par-term-emu-core-rust` VT parser's own
expectations.

#### A2. Write table-driven tests inside the crate

Create `par-term-input/tests/key_encoding_tests.rs`:

```rust
struct Case { key: Key, mods: ModifiersState, mode: TerminalMode, expect: &'static [u8] }
```

Cover: bare named keys (arrows, Home/End, Page Up/Down, F1–F12, Insert/Delete); each modifier alone and in
combination (Shift, Ctrl, Alt, Super) against arrows and letters; application-cursor vs normal mode for arrows;
application-keypad mode; Ctrl+letter → control codes (Ctrl+A → `0x01`, and specifically the edge cases Ctrl+@,
Ctrl+[, Ctrl+?); and the Kitty protocol levels if supported.

**Use `KeyInput`, not a fabricated `KeyEvent`.** Commit `53705aaf` removed exactly that anti-pattern after it
caused a Linux SIGSEGV via `MaybeUninit::assume_init()` on a struct with a private field — `tests/input_tests.rs`
now carries a comment explaining why. Do not reintroduce it.

#### A3. Add the ENH-002 corpus

Drive character-key encoding with the non-ASCII fixtures (accented, CJK, emoji) — multi-byte characters going
to the PTY is a path where a byte/char confusion would land, and it is currently untested.

### Part B — Pane render path golden images

#### B1. Decide the harness

The repo already has the hooks: `--screenshot <path>`, `--screenshot-after <s>`, `--exit-after <s>` (see
CLAUDE.md's agent-operability conventions and `src/app/window_manager/cli_timer.rs:92`). Prefer driving the real
binary over constructing a headless `wgpu` device in-process — it tests the path users actually get, and it needs
no new abstraction.

**Prerequisite**: QA-011 must land first. Until it does, `--screenshot` uses the *divergent* single-pane builder,
so golden images taken now would enshrine the bug. Verify with
`grep -rn "render_cells_to_target" par-term-render/src` — after QA-011 it should be gone.

#### B2. Make rendering deterministic

Non-negotiable for image comparison. Add or confirm flags for: fixed window size, fixed font and font size
(ship a test font or pin to a guaranteed-present one), animations off, cursor blink off, a fixed RNG seed for any
shader using one, and a fixed `unicode_version` (width tables affect layout).

Feed content through a deterministic source — a scripted input file or the existing scripting protocol — not a
live shell.

#### B3. Golden-image cases

Prioritize exactly what QA-011 and ARC-004 would have caught:

| Case | Guards against |
|---|---|
| Single pane, ASCII text | baseline |
| **2 and 6 split panes** | **ARC-004** — the garbage-output failure mode |
| Search highlights active | QA-011 — per-cell overlays applied in `gpu_submit.rs:360` |
| URL underline visible | QA-011 — and QA-002's clone path |
| Wide chars (CJK) + block chars (`▄▀`) | wide-cell spacer handling; ENH-002 overlap |
| Cursor styles (block/beam/underline) | the documented phase-3 ordering rule |
| Background image + custom shader | the shader intermediate-texture path |

Store goldens under `tests/golden/`. Compare with a small perceptual tolerance, not bit-exactness — GPU drivers
differ across machines, and a bit-exact test will be `#[ignore]`d within a month, which is worse than no test.

#### B4. Gate on platform

Mark these `#[ignore]` by default (matching the repo's existing convention for PTY-dependent tests, using
`#[ignore = "requires GPU"]` per QA-035's style) and run them in a dedicated CI job on a runner with a known GPU,
or locally via `cargo test -- --include-ignored`. Do **not** put driver-dependent image comparison in the main
`cargo test --workspace` path — it will produce cross-platform flakes and erode trust in the suite.

#### B5. Assert the QA-011 invariant directly

Beyond images, add a cheap structural test: take a screenshot with 2+ panes and assert the captured frame's
dimensions and pane count match the live layout. That catches "screenshot used the wrong path" without needing
pixel comparison, and it is the specific regression worth locking down.

## Files to Touch

| File | Change |
|---|---|
| `par-term-input/tests/key_encoding_tests.rs` | **new** — Part A matrix |
| `par-term-input/Cargo.toml` | `[dev-dependencies]` for the test harness |
| `tests/render_golden_tests.rs` | **new** — Part B driver |
| `tests/golden/*.png` | **new** — reference images |
| `src/cli.rs` (or equivalent) | determinism flags if any are missing |
| `Makefile` | `test-render` target (ignored-by-default suite) |
| `docs/guides/TROUBLESHOOTING.md` | how to regenerate goldens |
| `CONTRIBUTING.md` | note that render changes require golden review |

## Verification

```bash
# Part A
cargo test -p par-term-input                 # new matrix passes
make checkall

# Part B (after QA-011)
cargo test --test render_golden_tests -- --include-ignored
make build && ./target/dev-release/par-term --screenshot /tmp/shot.png --exit-after 6
```

Acceptance criteria that actually prove value:

1. **Part A must catch a deliberate regression** — flip application-cursor-mode handling in
   `key_encoding.rs` and confirm tests fail. A matrix that passes against broken encoding is decoration.
2. **Part B must catch the ARC-004 failure mode** — with 6 panes, revert to a single shared buffer at offset 0
   and confirm the golden comparison fails.
3. **Part B must catch QA-011** — point `take_screenshot` back at `render_cells_to_target` and confirm the
   split-pane and overlay cases fail.
4. Run the render suite twice on the same machine and confirm identical results (no nondeterminism) before
   committing goldens.

## Rollback

Purely additive; delete the test files and goldens to revert. No production changes except the determinism flags,
which are independently useful for debugging.

Risks worth naming up front. **Golden-image flakiness across GPUs** is the main one — mitigate with perceptual
tolerance, an ignored-by-default gate, and a single known CI runner; if flakes persist, keep the structural
assertions from B5 and drop pixel comparison rather than disabling the suite. **Goldens enshrining bugs** —
review every reference image by eye before committing, and do not generate them until QA-011 has landed.
**Font availability** — a golden rendered with a substituted font differs everywhere; ship a test font or pin
explicitly.
