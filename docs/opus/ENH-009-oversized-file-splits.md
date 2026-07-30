# ENH-009 — Split the five oversized files and add a CI line-count gate

> **Impact**: medium · **Effort**: large · **Source**: AUDIT.md **ARC-009** (Medium) + **QA-022** (Medium) +
> **QA-021** (Medium) — deferred out of the remediation cycle

## Goal

Bring the five files over the project's own 800-line production threshold back under it, split the single
655-line function inside one of them, and replace the five stale hand-maintained line-count comments with a CI
check — so the threshold is enforced mechanically instead of by comments that have already silently failed.

## Current State

CLAUDE.md sets the standard: *"Keep files under 500 lines; refactor files exceeding 800 lines."* Measured with
`#[cfg(test)]` tails excluded, five files breach the hard threshold and **83** breach the 500-line target:

| File | Production lines | Note |
|---|---:|---|
| `par-term-config/src/config/config_struct/mod.rs` | **1528** | also ENH-007 (`Config`, 238 fields) |
| `par-term-config/src/shader_controls.rs` | **1041** | contains QA-022's 655-line function |
| `par-term-config/src/snippets.rs` | **1038** | also QA-037 (`git rev-parse` at `:1004-1024`) |
| `par-term-render/src/cell_renderer/pane_render/mod.rs` | **863** | also ARC-004 |
| `src/app/triggers/mod.rs` | **816** | |

Two measurement corrections worth keeping, because a naive `wc -l` sweep gets them wrong: `par-term-mcp/src/lib.rs`
is 920 total but only **413** production (the rest is tests), and `par-term-acp/src/agent.rs` is 807 total but
**641** production. Neither is a violation. And
`par-term-render/src/cell_renderer/block_chars/box_drawing_data.rs` (1051 lines) is a static
`BOX_DRAWING_ENTRIES` glyph table — **legitimately exempt**, and CLAUDE.md should say so explicitly rather than
leaving it as a standing violation.

**QA-022** — `parse_shader_controls` (`par-term-config/src/shader_controls.rs:368-1022`) is a **655-line single
function** with cyclomatic complexity 71 and **zero nested functions**: the worst long method in the repo. Its
helpers already exist at `:126-342`; the body is one per-line branch ladder that should delegate per control kind.

**QA-021 — why this keeps recurring.** Five files carry `ARC-009` header comments meant to warn when they
approach the limit. **Every one understates its own count**, so the early-warning mechanism is silently dead:

| File | Comment claims | Actual |
|---|---:|---:|
| `par-term-render/src/renderer/mod.rs:1` | 743 | **798** ← 2 lines under threshold, advertising 55 to spare |
| `par-term-render/src/renderer/rendering.rs:1` | 705 | **796** |
| `par-term-render/src/graphics_renderer.rs:1` | 726 | **771** |
| `par-term-render/src/cell_renderer/mod.rs:1` | 742 | **766** |
| `par-term-render/src/cell_renderer/background.rs:1` | 693 | **701** |

A hand-maintained number that no process updates is worse than none — it reads as a fresh measurement. Same
failure mode as ARC-005's `WindowState` docstring and DOC-011's version line, and the same conclusion: **delete
the number, automate the check.**

**Why deferred from the remediation cycle**: splitting files invalidates every line anchor in the audit. That is
also why **QA-008 must be fixed independently** — it is a Critical panic in `par-term-config`, unrelated to whether
this ever runs.

## Implementation

**Do the CI gate first (Step 1), then one file per PR.** The gate is cheap, immediately useful, and tells you when
you are done.

### Step 1 — Add the gate and delete the stale comments (small, do this alone)

1. Write `scripts/check-line-counts.sh`:
   - Count **production** lines per `.rs` file — everything before the first `#[cfg(test)]` module. This matters:
     a naive count would falsely flag `par-term-mcp/src/lib.rs` and `par-term-acp/src/agent.rs` and erode trust in
     the check immediately.
   - Fail over 800; warn over 500.
   - Read exemptions from a checked-in allowlist file (`.line-count-exempt`) with a required reason per entry.
     Seed it with `box_drawing_data.rs` (static data table) and, temporarily, the five current violators so the
     gate can land green and then be tightened as each is fixed.
2. Add a `make check-line-counts` target and wire it into the `ci` target and `.github/workflows/ci.yml`.
3. **Delete** the five stale `ARC-009` header counts (QA-021), keeping the extraction plans in those comments.
   Do not update the numbers — that just resets the drift clock.

Landing this alone is genuinely valuable even if no file is ever split: it converts an unreliable manual signal
into a reliable automatic one.

### Step 2 — `shader_controls.rs` + QA-022 (highest value, do first)

The 655-line function is the worst offender and the most mechanical to fix.

1. Read `:126-342` to see which helpers already exist — the split should route through them, not duplicate them.
2. Identify the branch discriminant in `parse_shader_controls` (`:368-1022`) — it is a per-control-kind ladder.
3. Extract one `fn parse_<kind>_control(...) -> Result<ShaderControl, String>` per kind, then reduce
   `parse_shader_controls` to a dispatcher. This is the same shape as **ENH-004**'s dispatch-table work; if that
   has landed, reuse its pattern.
4. Move the extracted parsers into `par-term-config/src/shader_controls/` as a directory module
   (`mod.rs` + one file per kind), which addresses both the function length and the file length in one change.
5. Add per-kind unit tests. The function currently has none, and each extracted parser is small enough to test
   directly — that is the payoff.

**Note the interaction with QA-008**: `types/shader.rs`'s hex-color panic is adjacent to this parsing surface.
Land QA-008 first (it is Critical); this change should not touch it.

### Step 3 — `snippets.rs`

Split along its existing concerns: snippet types, parsing/validation, keybinding generation, and the git/workflow
integration at `:1004-1024`. That last one is **QA-037**'s site (unbounded `git rev-parse` on the main thread) —
extracting it into its own module makes QA-037's timeout fix easier, so coordinate rather than colliding.

### Step 4 — `triggers/mod.rs`

Split into trigger matching, action execution, and the security controls (`:378-530` — the exemplary
allowlist/denylist/rate-limit/audit stack the audit praised, and the model **SEC-002** should adopt). Keep that
security stack intact and cohesive; if **SEC-002** is in flight and reusing it, sequence after that lands so it is
not refactoring a moving target.

### Step 5 — `pane_render/mod.rs`

**Blocked on ARC-004.** That change restructures instance-buffer addressing throughout this file, and QA-011
follows it. Splitting first would duplicate the divergent screenshot path into new files and make QA-011 strictly
harder — the audit says so explicitly. Wait for both, then split by phase (background instances, text instances,
draw-call emission).

### Step 6 — `config_struct/mod.rs`

**This is ENH-007's work, not a separate split.** Draining the 238 fields into the 14 existing sub-configs takes
the file from 1528 lines to a few hundred as a side effect. Do not attempt an independent split — you would be
inventing a second grouping that conflicts with the one already scaffolded.

### Step 7 — Tighten the gate

As each file drops below 800, remove its exemption. When all five are gone, the allowlist should contain only
`box_drawing_data.rs`. Add the exemption rationale to CLAUDE.md so the next reader knows the data table is
intentional.

## Files to Touch

| File | Change |
|---|---|
| `scripts/check-line-counts.sh` | **new** — production-line counter |
| `.line-count-exempt` | **new** — allowlist with reasons |
| `Makefile` | `check-line-counts` target; add to `ci` |
| `.github/workflows/ci.yml` | run the check |
| 5 × `par-term-render/**` header comments | **delete** stale counts (QA-021) |
| `par-term-config/src/shader_controls.rs` → `shader_controls/` | Step 2 (+ QA-022) |
| `par-term-config/src/snippets.rs` → `snippets/` | Step 3 |
| `src/app/triggers/mod.rs` → split | Step 4 |
| `par-term-render/src/cell_renderer/pane_render/mod.rs` | Step 5 — **after ARC-004 + QA-011** |
| `CLAUDE.md` | document the exemption and the automated check |

## Verification

Per PR:

```bash
make checkall
make check-line-counts          # the new gate
cargo test -p <affected-crate>
```

The gate itself needs proving — a check that never fails is not a check:

```bash
# it must flag a real violation
printf '\n// filler\n%.0s' {1..900} >> src/lib.rs && make check-line-counts   # MUST fail
git checkout src/lib.rs
# and it must NOT flag the two test-heavy false positives
make check-line-counts 2>&1 | grep -E "par-term-mcp/src/lib.rs|par-term-acp/src/agent.rs"  # expect no match
```

For each split, the acceptance criterion is **behavioral equivalence**, since these are pure reorganizations:

```bash
make build && ./target/dev-release/par-term --screenshot /tmp/a.png --exit-after 6
```

Confirm shader controls still parse (load a shader with each control kind), snippets and triggers still fire, and
`--dump-config` output is unchanged.

## Rollback

Per-PR revert. Step 1 (the gate) is independent and should be kept even if every split is reverted — it is the part
that prevents recurrence.

Risks:

1. **Splitting a file whose line anchors other work depends on.** This is the entire reason the item is deferred.
   Before each PR, check `git status` and the board for in-flight work in that file, and respect the two hard
   orderings: `pane_render/mod.rs` after ARC-004 + QA-011; `config_struct/mod.rs` via ENH-007 only.
2. **A gate that produces false positives** gets bypassed or deleted within weeks. The production-line counting in
   Step 1 and the verification above are what prevent that — do not ship a naive `wc -l`.
3. **Splitting for line count rather than cohesion** — arbitrary boundaries are worse than a long cohesive file.
   If a file has no natural seam, exempt it with a reason instead of forcing a split. `box_drawing_data.rs` is the
   precedent.
4. **Behavior change during "pure" reorganization** — `pub`/`pub(crate)` visibility shifts when code moves between
   modules. `make checkall` catches compile breaks but not accidental API widening on a published crate; review
   visibility explicitly on `par-term-config` and `par-term-render`.
