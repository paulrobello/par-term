# ENH-006 — Decompose `SettingsUI` (215 fields) into per-tab state structs

> **Impact**: medium · **Effort**: large · **Source**: AUDIT.md **ARC-002** (High) — deferred out of the
> remediation cycle

## Goal

Replace `SettingsUI`'s 215 flat fields with ~25 named per-tab state structs mirroring the existing `*_tab/`
directory layout, so each tab's state can be reasoned about, tested, and changed in isolation — and so the
published crate's semver surface shrinks.

## Current State

`par-term-settings-ui/src/settings_ui/mod.rs:22` declares **215 fields in a single struct body**. par-mem ranks
it as the most central symbol in the entire repository:

- **PageRank 0.0098** — highest in the repo
- **In-degree 245** — highest fan-in, 2.7× `WindowState`'s 91
- Flagged as an **articulation point**: removing it disconnects the graph

Behavior is spread across **11 `impl SettingsUI` blocks in 11 files**, and roughly **25 `*_tab/` directories
mutate its fields directly** (`settings.some_field = …`). The consequence is that no tab's state is separable:
every settings tab is coupled to every other through one struct, and there is no unit of state small enough to
test.

Two constraints that shape the whole plan:

1. **`par-term-settings-ui` is published to crates.io** (`.github/workflows/publish-crates.yml:170`, version
   0.15.x). All 215 fields are `pub`, so every one is a semver commitment and any rename is a breaking release.
2. **The prior audit cycle's tracking note covers only `WindowState`.** This larger object had no tracking entry
   at all until this audit, which is why it grew unnoticed — see ARC-005 for the same failure mode on the
   smaller object.

**Why this was deferred from the remediation cycle**: regrouping rewrites the field access path
(`settings.foo` → `settings.appearance.foo`) across ~25 tab modules, colliding with nearly every line QA-014
touches in `profiles_tab/dynamic_sources.rs`. Running it alongside 100+ other audit fixes maximizes conflict risk
for zero correctness gain. QA-014 was unblocked precisely by deferring this.

## Implementation

**Stage per tab. One tab per PR.** This is the entire risk-management strategy — a single 215-field change across
25 modules is unreviewable and will conflict with everything.

### Step 0 — Inventory and group

```bash
# the 215 fields
sed -n '/^pub struct SettingsUI/,/^}/p' par-term-settings-ui/src/settings_ui/mod.rs | grep -c "pub "
# the tab modules that will become the grouping
ls -d par-term-settings-ui/src/*_tab*/ | sed 's|.*/\([^/]*\)/|\1|'
# where each field is actually read/written
grep -rn "settings\.<field>" par-term-settings-ui/src src/ | wc -l
```

Produce a mapping table of field → owning tab **before writing any code**, and record it in the PR description.
Fields used by more than one tab are the interesting cases: they either belong in a shared `CommonState` or
indicate the tabs are genuinely coupled. Decide per field; do not default to duplicating.

Also identify fields the **root crate** reads (`grep -rn "settings_ui\|SettingsUI" src/`). Those must stay `pub`;
everything else becomes `pub(crate)`, which is where most of the semver-surface reduction comes from.

### Step 1 — Pilot with the smallest tab

Pick the tab with the fewest fields and least cross-tab usage (likely `ssh_tab` — note it is also SEC-002's file,
so coordinate if that is still in flight, or pick another).

1. Create `par-term-settings-ui/src/settings_ui/state/<tab>_state.rs` with a `#[derive(Default)]` struct holding
   that tab's fields.
2. Replace those fields on `SettingsUI` with one member: `pub <tab>: <Tab>State`.
3. Update the tab module's accesses to `settings.<tab>.field`.
4. Update the root crate if it reads any of them.
5. `make checkall`. The compiler finds every missed site — that is what makes this safe despite the scale.

Land this PR alone and confirm the pattern before continuing.

### Step 2 — Repeat per tab

Roughly 25 iterations of Step 1. Keep each PR to one tab. After each, re-run the gate.

Track progress in the PR series so a partial migration is legible: `SettingsUI` will temporarily hold a mix of
grouped members and ungrouped flat fields, which is fine and is the point of staging.

### Step 3 — Preserve `has_changes` semantics

The repo's config-option workflow requires that on any change a tab sets `settings.has_changes = true` and
`*changes_this_frame = true` (CLAUDE.md, "Adding a New Configuration Option"). Those two flags are cross-cutting
and must **stay on the root** `SettingsUI`, not move into a tab struct. Verify after each PR that changing a
setting in the migrated tab still marks the config dirty and persists — this is the most likely silent
regression, because it compiles fine either way.

### Step 4 — Shrink the published surface

Once grouped, mark each tab-state field `pub(crate)` unless the root crate reads it. Add accessors where the root
crate needs read-only access. This is a **breaking change** to a 0.15.x crate — coordinate with the release
process:

- Bump the minor version (0.x, so minor signals breaking).
- Note it in `CHANGELOG.md` and `docs/guides/MIGRATION.md`, which already has the structure for this.
- Follow CLAUDE.md's documented sub-crate bump order: `par-term-settings-ui` is Layer 2, so bump it and update
  the version reference in the root `Cargo.toml`.

### Step 5 — Add the tests the decomposition enables

The payoff. Each tab state is now constructible in isolation, so add per-tab tests for defaults, change
detection, and validation. `par-term-settings-ui` currently sits at ~88% docstring coverage but has little
behavioral testing.

### Step 6 — Update the tracking record

Add a docstring to `SettingsUI` recording the object's size, the decomposition plan, and a **date** — and learn
from ARC-005: prefer a command that re-derives the count over a hardcoded number, because a hand-maintained
figure will drift and then actively mislead.

## Files to Touch

| File | Change |
|---|---|
| `par-term-settings-ui/src/settings_ui/mod.rs:22` | the struct; 215 fields → ~25 members |
| `par-term-settings-ui/src/settings_ui/state/*.rs` | **new** — one per tab |
| `par-term-settings-ui/src/settings_ui/*.rs` (11 impl files) | access-path updates |
| `par-term-settings-ui/src/*_tab/**` (~25 dirs) | access-path updates |
| `src/settings_window/**` | root-crate access-path updates |
| `par-term-settings-ui/Cargo.toml`, root `Cargo.toml` | version bump |
| `CHANGELOG.md`, `docs/guides/MIGRATION.md` | breaking-change notes |

## Verification

Per PR:

```bash
make checkall
cargo test -p par-term-settings-ui
cargo check -p par-term-settings-ui --all-features
```

Then manually, for the migrated tab: open Settings, change each control, confirm the value applies, `has_changes`
is set, the config persists to `~/.config/par-term/config.yaml`, and the value survives a restart. The compiler
cannot catch a lost `has_changes` assignment, so this check is mandatory rather than optional.

At the end of the series:

```bash
sed -n '/^pub struct SettingsUI/,/^}/p' par-term-settings-ui/src/settings_ui/mod.rs | grep -c "pub "  # ~25
grep -c "pub " par-term-settings-ui/src/settings_ui/state/*.rs                                        # sums to ~215
```

## Rollback

Per-PR revert. Because each PR touches exactly one tab, reverting one restores that tab's flat fields without
affecting the others — the staged design *is* the rollback plan.

Risks, in order of likelihood:

1. **Lost `has_changes` / `changes_this_frame` assignments** — compiles fine, silently stops persisting a
   setting. Mitigated only by the manual per-tab check in Verification. This is the one to watch.
2. **Breaking downstream crates.io consumers** — unavoidable and intended; handle via version bump and migration
   notes rather than trying to preserve 215 `pub` fields.
3. **Merge conflicts with in-flight settings work** — check for concurrent sessions before starting each PR
   (`git status`, and the fleet-coordination tooling if another agent may be active).
4. **Partial migration left indefinitely** — a half-grouped struct is *worse* than either end state for
   readability. If the series stalls, either finish it or revert the remaining PRs; do not leave it mid-flight
   across releases.
