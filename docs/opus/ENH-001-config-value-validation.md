# ENH-001 — Validate every documented config value against its type

> **Impact**: high · **Effort**: medium · **Source**: AUDIT.md DOC-001 (Critical), DOC-009, DOC-012, DOC-021

## Goal

Make it impossible for `docs/CONFIG_REFERENCE.md` or `docs/API.md` to document a config value the
deserializer would reject. A test extracts every documented enum value and round-trips it through
`serde_yaml_ng` against the real type; any mismatch fails the build.

## Current State

Two hand-maintained documents describe the config surface, and nothing checks either:

- `docs/CONFIG_REFERENCE.md` — 694 lines, no staleness disclaimer, treated by users as authoritative. The
  audit found **four** enum value-sets that cause a hard config parse failure (DOC-001) and **two** more that
  silently deserialize to the default (DOC-012).
- `docs/API.md` — 619 lines, carries an honest disclaimer at `:3` and even proposes a `make doc-check` gate at
  `:8-14`. All 317 rows resolve to real public items, but **eleven** enums document variants that do not exist
  (DOC-009), and one documents YAML parsing as TOML (DOC-021).

The concrete failures, each verified against the type:

| Doc site | Documented | Real | Why it drifted |
|---|---|---|---|
| `CONFIG_REFERENCE.md:268` | `nfc, nfd, nfkc, nfkd, none` | `NFC, NFD, NFKC, NFKD, none` | `types/unicode.rs:80-104` has **no** `rename_all`; only `None` has `#[serde(rename)]` |
| `CONFIG_REFERENCE.md:266` | `unicode_9 … unicode_16` | `unicode9 … unicode16`, `unicode15_1` | `rename_all = "snake_case"` on `Unicode9` gives no underscore before a digit |
| `CONFIG_REFERENCE.md:537` | `bar_with_text` | `barwithtext` | `rename_all = "lowercase"` on `BarWithText` |
| `CONFIG_REFERENCE.md:186` | `custom` + path | `!custom /path` | `types/font.rs:56` is a newtype variant `Custom(String)` |

The pattern is consistent: **the serde attribute, not the variant name, decides the wire value**, and a human
transcribing from the variant name gets it wrong in a way that is invisible until a user tries it.

`par-term-config` publishes to crates.io at 0.12.2 with ~237 top-level config fields, so this surface is both
large and externally visible.

## Implementation

### Step 1 — Make the enum values machine-readable

The generator needs a list of `(type, accepted values)`. Two options; **prefer (a)**:

**(a) Derive from the types at test time.** For each config enum, add a `#[cfg(test)] fn all_serde_names() ->
&'static [&'static str]` — or better, derive it. Most of these enums already implement `Default` and several
already have `all()`-style helpers (e.g. `src/paste_transform/mod.rs`). Where a helper exists, reuse it.

**(b) Parse the doc and probe the deserializer.** No type changes: extract candidate values from the markdown
table and attempt deserialization. Weaker (it cannot detect a *missing* documented value) but zero-friction.

Do both if time allows — (a) catches omissions, (b) catches wrong values.

### Step 2 — Write the extractor

Add `par-term-config/tests/doc_config_values.rs`:

1. Read `docs/CONFIG_REFERENCE.md` (path relative to `CARGO_MANIFEST_DIR/..`).
2. Parse rows of the form `| \`key\` | \`enum\` | \`default\` | …: \`a\`, \`b\`, \`c\` |`. Match only rows whose
   type column is `enum`, and pull the backticked tokens from the description cell.
3. For each `(key, value)`, build a minimal YAML document `{key}: {value}` and deserialize it into `Config`.
4. Assert `Ok`. On `Err`, fail with the key, the rejected value, and the serde error — the message is the
   deliverable, so make it name the file and line.

### Step 3 — Handle the three shapes the naive parser gets wrong

These are the cases that will otherwise produce false failures. Handle them explicitly:

- **Newtype variants** (`Custom(String)`) need YAML tag syntax `!custom /path`, not a bare scalar. Detect the
  `!` form in the doc and construct accordingly; if the doc lacks it, that is the DOC-001 `:186` defect and the
  test should fail.
- **Non-enum rows** — skip anything whose type column is not `enum`. Do not try to validate free-form strings,
  paths, or numeric ranges here; that is scope creep and will generate noise.
- **Prose values** — descriptions sometimes contain backticked text that is not a value (`` `true` ``,
  `` `~/.config` ``). Require the row's type column to be `enum` **and** restrict to the segment after the
  final `:` in the description cell.

### Step 4 — Fix the drifted values

Run the test. It should fail on the four DOC-001 sites and the two DOC-012 sites. Correct the documentation
(not the code) until green. **This is DOC-001's remediation** — coordinate so it is not done twice.

### Step 5 — Extend to `docs/API.md` (optional, second pass)

Same approach against the enum rows in `docs/API.md`. This is the DOC-009 surface. Lower priority: API.md
carries a disclaimer and its audience is developers who can read the type.

### Step 6 — Wire into CI

Add to the existing test job — no new workflow needed, since `cargo test --workspace` already runs in
`.github/workflows/ci.yml`. Optionally add the `make doc-check` target `docs/API.md:8-14` proposes, aggregating
this test with ENH-002's and the DOC-022 link check.

## Files to Touch

| File | Change |
|---|---|
| `par-term-config/tests/doc_config_values.rs` | **new** — the extractor and round-trip test |
| `par-term-config/src/types/unicode.rs` | add `all_serde_names()` under `#[cfg(test)]` (approach (a)) |
| `par-term-config/src/types/integration.rs` | same |
| `par-term-config/src/types/tab_bar.rs`, `terminal.rs`, `font.rs`, `rendering.rs` | same |
| `docs/CONFIG_REFERENCE.md` | correct the six drifted value sets (DOC-001, DOC-012) |
| `Makefile` | optional `doc-check` target |

## Verification

```bash
cargo test -p par-term-config --test doc_config_values   # the new gate
make checkall                                            # full project gate
```

Then prove it actually catches drift — this is the real acceptance test:

```bash
# Introduce a deliberate error and confirm the test fails
sed -i.bak 's/`barwithtext`/`bar_with_text`/' docs/CONFIG_REFERENCE.md
cargo test -p par-term-config --test doc_config_values   # MUST fail
mv docs/CONFIG_REFERENCE.md.bak docs/CONFIG_REFERENCE.md
```

A test that passes but would not have caught the original four defects is worse than no test — it manufactures
confidence. Verify against the real historical values before declaring done.

## Rollback

Entirely additive: one new test file plus `#[cfg(test)]` helpers. Delete the test file to revert. The
documentation corrections in Step 4 stand on their own and should **not** be rolled back — they are DOC-001's
fix and are correct independent of the test.

Risk: a brittle parser producing false failures that get `#[ignore]`d, which is worse than nothing. Mitigate by
keeping the parser strict about what it matches (Step 3) and failing loudly with the exact doc line rather than
a generic assertion.
