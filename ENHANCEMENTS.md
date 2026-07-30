# par-term Enhancements

> **What this file is.** A standing, cumulative backlog of performance, functionality, and
> maintainability opportunities — distinct from `AUDIT.md`, which tracks *defects*. Items here are
> improvements worth making, not bugs.
>
> **Who works it.** `/enhancement-all` implements the unchecked items, handing each plan to an Opus 5
> subagent; `/enhancement-next` does a single item after confirming the choice. `/fix-audit` deliberately
> ignores this file.
>
> **Checkbox discipline.** An item is marked `[x]` **only after its plan's verification passes** — not for
> "code written". Finished items are **marked, never deleted**, so this file stays a durable record of what
> shipped.
>
> **Numbering.** IDs are permanent and never reused or renumbered — each appears on a kanban card and in a
> `docs/opus/` plan filename. New ideas continue from the highest existing ID.

---

## Prevention Infrastructure

The audit's two most serious defect classes were both invisible to a fully green gate (1,965 tests, clippy
clean). These three items close the gaps that let that happen, and are the highest-leverage work in this file.

- [ ] **ENH-001 — Validate every documented config value against its type** — `docs/CONFIG_REFERENCE.md`
  documented four enum value-sets that make par-term fail to start (AUDIT.md DOC-001), and `docs/API.md`
  documented eleven more that do not exist (DOC-009). Both files are hand-maintained with nothing checking
  them. Add a test that extracts every documented enum value from the config reference and round-trips it
  through `serde_yaml_ng` against the real type, failing on any value the deserializer rejects. This turns a
  whole class of user-facing "config won't load" reports into a build failure, and it would have caught all
  fifteen drifted values mechanically. (impact: high, effort: medium, plan: docs/opus/ENH-001-config-value-validation.md)

- [ ] **ENH-002 — Non-ASCII and wide-character test corpus** — six Critical/High defects (QA-004 … QA-008,
  QA-014) share one root cause: a terminal column index used as a UTF-8 byte offset or a `char`-vector index.
  Column, byte, and char indices coincide **only for pure-ASCII single-width rows**, and every test input in
  the suite is ASCII — which is precisely why 1,965 green tests never saw any of them. Introduce a shared
  fixture set (accented Latin, CJK, emoji with combining marks, RTL, mixed) and drive the copy-mode, search,
  paste-transform, and rendering paths with it. Add a proptest generator so new sites are covered by default
  rather than by remembering. (impact: high, effort: medium, plan: docs/opus/ENH-002-unicode-test-corpus.md)

- [ ] **ENH-003 — Panic boundary that preserves session state** — there is **no `catch_unwind` anywhere in the
  workspace**, so every reachable panic is an unrecoverable whole-app crash that loses all terminal state,
  scrollback, and unsaved session data across every tab and window. That is what turned each of the six
  index-confusion defects from an annoyance into data loss. Install a boundary around the event loop that, on
  panic, flushes the session/arrangement/command-history state through the atomic-save path and reports the
  panic to the user before exiting. Pairs naturally with AUDIT.md QA-023's atomic-save helper.
  (impact: high, effort: medium, plan: docs/opus/ENH-003-panic-boundary-session-preservation.md)

## Maintainability & Test Coverage

- [ ] **ENH-004 — Dispatch tables for the four Critical-complexity event handlers** — par-mem ranks these as
  both the highest-complexity and highest-churn functions in the repo: `handle_menu_action` (complexity 104,
  churn 20), `handle_key_event` (96), `about_to_wait` (86), `handle_window_event` (83), plus
  `execute_keybinding_action` (71) and `apply_renderer_config` (74, the single highest hotspot score at 1480).
  Each is one enormous match ladder that every feature touches, making them the repo's worst merge-conflict
  points and effectively untestable. Convert each to a dispatch table mapping action → handler function, so
  adding an action becomes a table entry plus a small testable function rather than another arm in a 100-branch
  match. (impact: medium, effort: large, plan: docs/opus/ENH-004-event-handler-dispatch-tables.md)

- [ ] **ENH-005 — Tests for `par-term-input` and the pane render path** — the two areas CLAUDE.md itself flags
  as highest-risk have at or near zero coverage. `par-term-input` has **no tests within the crate** across all
  four source files despite being the live winit-event → terminal-bytes path and holding two functions of
  complexity 67 and 52; it is exercised only indirectly by the root crate's integration tests. The designated
  single rendering path — `pane_render/mod.rs` (863 lines), `text_instance_builder.rs`, `bg_instance_builder.rs`
  — has none at all, which is why QA-011 (screenshots silently diverging from the screen) and ARC-004 (garbage
  output if submits are batched before buffers are suballocated) were both invisible. Add byte-exact encoding
  tests for the input crate and golden-image tests for the pane path.
  (impact: high, effort: large, plan: docs/opus/ENH-005-input-and-render-path-tests.md)

## Deferred Audit Refactors

These three were found by the audit (AUDIT.md ARC-002, ARC-003, ARC-009, QA-022) and **deliberately excluded
from its remediation cycle** — each moves code across files and would invalidate the line anchors every other
fix depends on. They are real findings with real severity, tracked here so they are scheduled rather than
dropped. Their kanban cards already exist from the audit; do not file duplicates.

- [ ] **ENH-006 — Decompose `SettingsUI` (215 fields) into per-tab state structs** — the largest god object in
  the repo and the highest by both PageRank (0.0098) and fan-in (in-degree 245), flagged by par-mem as an
  articulation point 2.7× more central than `WindowState`. Behavior spans 11 `impl` blocks across 11 files
  while ~25 `*_tab/` directories mutate its fields directly, so no tab's state can be reasoned about or tested
  in isolation. Because `par-term-settings-ui` is published to crates.io, all 215 `pub` fields are semver
  commitments. Group them into ~25 per-tab structs mirroring the existing directory layout, staged per tab.
  (impact: medium, effort: large, plan: docs/opus/ENH-006-settingsui-decomposition.md)

- [ ] **ENH-007 — Drain `Config` (238 fields) into its 14 existing sub-config structs** — highest betweenness
  centrality in the entire graph (0.052) with 166 inbound type references, making it the single symbol whose
  change fragments the most of the codebase. The decomposition pattern is already established — 14 sub-config
  structs sit in the same directory — but the root struct was never drained into them, and CLAUDE.md's
  "Adding a New Configuration Option" workflow directs every new setting straight back into it. Move each
  thematic cluster into its matching `*_config.rs`, keeping `#[serde(flatten)]` so on-disk YAML stays
  compatible. (impact: medium, effort: large, plan: docs/opus/ENH-007-config-decomposition.md)

- [ ] **ENH-009 — Split the five oversized files and add a CI line-count gate** — five files exceed the
  project's own 800-line production threshold (`config_struct/mod.rs` 1528, `shader_controls.rs` 1041,
  `snippets.rs` 1038, `pane_render/mod.rs` 863, `triggers/mod.rs` 816) and 83 exceed the 500-line target.
  Includes `parse_shader_controls`, a single 655-line function (complexity 71, the worst long method in the
  repo) whose helpers already exist alongside it. The reason this keeps recurring: the five `ARC-009` header
  comments meant to warn about it all **understate** their own line counts, so the early-warning mechanism is
  silently dead. Split the files, delete the hand-maintained counts, and enforce the threshold in CI so the
  signal is mechanical. `box_drawing_data.rs` (1051 lines) is a static data table and should be explicitly
  exempted. (impact: medium, effort: large, plan: docs/opus/ENH-009-oversized-file-splits.md)

## Behavior Changes Requiring a Decision

- [ ] **ENH-008 — Honor `XDG_CONFIG_HOME` (and decide about the other XDG variables)** —
  `docs/guides/ENVIRONMENT_VARIABLES.md` claims par-term follows the XDG Base Directory specification and
  documents five variables, but `par-term-config/src/config/persistence.rs:186-190` hardcodes
  `~/.config/par-term/config.yaml` and none of the five is ever read for path resolution. AUDIT.md DOC-005
  fixes the *documentation* to match reality as the honest short-term move; this item is the other branch —
  actually implementing XDG support. It matters most on Linux and in dotfile-managed setups, where a
  non-default `XDG_CONFIG_HOME` currently means the user edits a config par-term never reads. This is a
  **behavior change** affecting where existing users' configs are found, so it needs a migration path (the repo
  already has `src/config_migration.rs` and `docs/guides/MIGRATION.md` for exactly this) and is a product
  decision, not a pure cleanup. (impact: medium, effort: medium, plan: docs/opus/ENH-008-xdg-base-directory-support.md)
