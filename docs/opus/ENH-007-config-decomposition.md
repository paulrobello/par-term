# ENH-007 — Drain `Config` (238 fields) into its 14 existing sub-config structs

> **Impact**: medium · **Effort**: large · **Source**: AUDIT.md **ARC-003** (High) — deferred out of the
> remediation cycle

> **Status: largely done.** `Config` is now **70 members — 41 flattened sub-configs and 29 leaf fields**,
> and `config_struct/mod.rs` is **760 production lines** (was 1,529), so its `.line-count-exempt` entry
> is gone. 27 new sub-configs took 195 of the 224 leaf fields. Corrections to the text below:
>
> - The 238 count is the **total member count**, of which 14 were already flattened sub-configs, so
>   **224 leaf fields** were actually drainable. The `sed … | grep -c "^\s*pub "` command in Step 0
>   returns **239** — it counts the `pub struct Config {` line itself.
> - Step 2's **delegating accessors were not used**. Moving fields and letting rustc's `E0609`/`E0560`
>   spans drive the call-site rewrite is exact, and accessors would only have to be deleted again.
> - Step 2's choice of **font as the pilot was wrong** — 32 fields across 82 files is the largest and
>   riskiest group, not the safest. Panes (20 fields, 70 refs) is the right shape for a first move.
> - **`#[derive(Default)]` is not safe by default here**: 137 of the 224 leaf fields do not default to
>   their type's `Default`. Each receiving sub-config's `Default` copies the exact initialiser from
>   `default_impl.rs`; only three groups whose fields are all genuinely type-default derive it.
> - Step 6's **version bump was deliberately not done** — it is a release action, not part of the
>   refactor. The field-path change is still a breaking API change for `par-term-config` and needs the
>   documented Layer 1 → Layer 4 bump order before publishing.
>
> Deliberately left on the root: terminal size, the font family/metrics block, one-field settings with
> no group, `keybindings`/`snippets` (their natural member names shadow the fields they replace, which
> disarms the compiler-guided rewrite for no structural gain), and the two `#[serde(skip)]` runtime
> security lists, which are computed state rather than configuration.
>
> The acceptance gate of Step 4 exists as `par-term-config/tests/config_yaml_compat.rs` (9 tests).

## Goal

Finish a decomposition that was started and abandoned. Move each thematic field cluster out of `Config`'s
238-field body into the matching `*_config.rs` struct that **already exists** beside it, keeping
`#[serde(flatten)]` so every existing `config.yaml` on disk continues to load unchanged.

## Current State

`par-term-config/src/config/config_struct/mod.rs:145` — the struct spans lines **145–1527**, holding **238 `pub`
fields**. par-mem metrics:

- **Betweenness centrality 0.052** — the highest in the entire graph, meaning this is the single symbol whose
  change fragments the most of the codebase
- **166 inbound type references**
- The file is **1,528 production lines**, the second-largest in the repo (also ARC-009 / ENH-009)

**The pattern already exists and was simply never applied.** The same directory holds 14 extracted sub-config
structs: `cursor_config.rs`, `font_config.rs`, `mouse_config.rs`, `notification_config.rs`, `status_bar_config.rs`,
`window_config.rs`, and eight more. Someone began this work, built the scaffolding, and stopped. So this is
continuation, not design.

**Why it keeps growing**: CLAUDE.md's "Adding a New Configuration Option" workflow directs every new setting into
this one file. That makes it a guaranteed merge-conflict point for concurrent feature work — and the workflow
itself needs updating as part of this change, or the file will refill.

Constraints:

1. **`par-term-config` is published to crates.io** at 0.12.2 and is **Layer 1** of the documented dependency
   graph — every Layer 2 and 3 crate depends on it. All 238 fields are semver surface.
2. **On-disk compatibility is non-negotiable.** ~237 documented options exist in `docs/CONFIG_REFERENCE.md` and
   real users have real `config.yaml` files. `#[serde(flatten)]` is what makes this a pure refactor rather than a
   migration.

**Why deferred from the remediation cycle**: moving field declarations across files invalidates every
line-anchored edit in the audit, and it transitively touches every `config.<field>` read site across 14 crates.
Deferring it unblocked DOC-020 (three wrong default-value doc comments in this very file).

**One thing that must NOT wait**: **QA-008** is a Critical panic in this crate's serde path
(`par-term-config/src/types/shader.rs:132,137` — a byte-slice on non-ASCII hex defeating the
`deserialize_uniforms` error handler). Fix it in the remediation cycle regardless of whether this item ever runs.

## Implementation

**One sub-config per PR.** Fourteen or so small, independently verifiable steps.

### Step 0 — Map fields to their destination

```bash
# the 238 fields
sed -n '145,1527p' par-term-config/src/config/config_struct/mod.rs | grep -c "^\s*pub "
# the destinations that already exist
ls par-term-config/src/config/config_struct/*_config.rs
# what each existing sub-config already holds
grep -n "pub struct\|^\s*pub " par-term-config/src/config/config_struct/font_config.rs
```

For each of the 14 existing sub-configs, list the fields still sitting on the root that belong to it. Fields with
no natural home either need a new sub-config or should stay on the root — decide explicitly rather than forcing a
grouping.

Cross-reference `docs/CONFIG_REFERENCE.md`, whose section headings are already a de facto grouping and a good
sanity check on your mapping.

### Step 1 — Understand the `serde(flatten)` contract before touching anything

This is the crux of the whole plan. The goal is that YAML like:

```yaml
font_family: "JetBrains Mono"
font_size: 14.0
```

continues to deserialize after `font_family`/`font_size` move into `FontConfig`. That requires:

```rust
pub struct Config {
    #[serde(flatten)]
    pub font: FontConfig,
    // …
}
```

**Known `serde(flatten)` pitfalls — read these before starting:**

- `flatten` uses an internal buffering deserializer, so it is **incompatible with `deny_unknown_fields`** on the
  same struct. Check whether `Config` or any sub-config sets it.
- `#[serde(default = "…")]` on individual fields still works, but a `Default` impl on the *flattened struct* does
  not apply unless the field itself has `#[serde(default)]`. par-term relies heavily on per-field defaults
  (`#[serde(default = "default_my_option")]` is the documented convention), so verify each moved field keeps its
  default function.
- Numeric/bool coercion behavior can differ subtly through the buffering path. Round-trip real configs (Step 4).
- Multiple `flatten` members on one struct are allowed but each adds buffering overhead; `Config::load` is not
  hot, so this is acceptable.

### Step 2 — Pilot with one sub-config

Choose `font_config.rs` (well-bounded, heavily documented, and it already exists):

1. Move the font fields from the root struct into `FontConfig`, preserving every `#[serde(default = "…")]`
   attribute and doc comment verbatim.
2. Add `#[serde(flatten)] pub font: FontConfig` to `Config`.
3. Add temporary delegating accessors on `Config` (`pub fn font_size(&self) -> f32 { self.font.font_size }`) so
   the ~14 crates' call sites keep compiling. Migrate call sites in a **separate** PR — this is what keeps the
   change reviewable.
4. `cargo test -p par-term-config`, then `make checkall`.

Land this alone. Confirm the round-trip test in Step 4 passes before continuing.

### Step 3 — Repeat, then migrate call sites

Repeat Step 2 for each remaining sub-config. Once the root struct is drained, remove the delegating accessors in
a final pass, updating `config.<field>` → `config.<group>.<field>` across consumers. The compiler finds every
site.

Note the interaction with **ENH-006**: `par-term-settings-ui` reads a large number of these fields, so sequence
the two rather than running them concurrently.

### Step 4 — Round-trip tests are the acceptance gate

Add `par-term-config/tests/config_yaml_compat.rs` **before** the first move:

1. Check in several representative real `config.yaml` files as fixtures — minimal, fully-populated, and a
   legacy one exercising deprecated keys.
2. Assert each deserializes, and that a chosen set of values reads back correctly.
3. Assert serialize → deserialize → serialize is stable.

This suite must pass unchanged after every PR. It is the only thing standing between this refactor and breaking
users' configs, and it also composes with **ENH-001**'s documented-value validation.

### Step 5 — Update the workflow that caused this

Change CLAUDE.md's "Adding a New Configuration Option" step 1 to direct new fields into the appropriate
`*_config.rs`, not `config_struct/mod.rs`. Without this, the root struct refills. Also update the required
follow-on steps (settings UI control, and the search keywords in
`par-term-settings-ui/src/sidebar.rs` → `tab_search_keywords()`).

### Step 6 — Release

`par-term-config` is Layer 1, so follow CLAUDE.md's documented bump order: bump `par-term-config`, then update its
`version = "…"` reference in every Layer 2/3 dependent, then the root. Record the breaking API change (not a
config-format change — that stays compatible) in `CHANGELOG.md` and `docs/guides/MIGRATION.md`.

## Files to Touch

| File | Change |
|---|---|
| `par-term-config/src/config/config_struct/mod.rs:145-1527` | drain 238 fields → ~14 flattened members |
| `par-term-config/src/config/config_struct/*_config.rs` (14) | receive fields |
| `par-term-config/tests/config_yaml_compat.rs` | **new** — round-trip gate |
| `par-term-config/tests/fixtures/*.yaml` | **new** — representative real configs |
| all 14 crates' `config.<field>` sites | access-path updates (final pass) |
| `CLAUDE.md` | fix the workflow that directs new fields to the root struct |
| `par-term-config/Cargo.toml` + all dependents + root | version bump per documented order |
| `CHANGELOG.md`, `docs/guides/MIGRATION.md` | breaking-change notes |

## Verification

Per PR:

```bash
cargo test -p par-term-config                          # includes the compat suite
cargo test -p par-term-config --test config_yaml_compat
make checkall
```

The check that actually matters — a real user config must still load:

```bash
cp ~/.config/par-term/config.yaml /tmp/real-config.yaml
make build && ./target/dev-release/par-term --dump-config --exit-after 3   # values match pre-refactor
```

Diff the `--dump-config` output before and after the whole series; it should be identical. Also confirm
`docs/CONFIG_REFERENCE.md` still describes reality (ENH-001's test enforces this mechanically if it has landed).

At the end:

```bash
sed -n '/^pub struct Config/,/^}/p' par-term-config/src/config/config_struct/mod.rs | grep -c "pub "  # ~14
wc -l par-term-config/src/config/config_struct/mod.rs                                                 # well under 800
```

## Rollback

Per-PR revert; each moves one sub-config's worth of fields.

Risks, ordered by consequence:

1. **Breaking users' `config.yaml`** — the worst outcome, and entirely preventable. The Step 4 compat suite must
   exist **before** the first move, not after. If a real config fails to load at any point, revert immediately
   rather than patching forward.
2. **A lost `#[serde(default = "…")]`** — a field silently becomes required, so an existing config that omits it
   now fails to parse. Diff the attributes on every moved field; do not retype them from memory.
3. **`deny_unknown_fields` incompatibility** with `flatten` — check for it up front (Step 1); discovering it
   mid-series means reworking every completed PR.
4. **Partial migration left indefinitely** — a half-drained struct is worse than either end state. Finish or
   revert; do not ship it mid-flight across releases.
