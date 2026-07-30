# ENH-002 — Non-ASCII and wide-character test corpus

> **Impact**: high · **Effort**: medium · **Source**: AUDIT.md QA-004 … QA-008, QA-014 (5 Critical + 1 High)

## Goal

Make the column/byte/char index-confusion class *structurally* detectable. Introduce a shared non-ASCII
fixture set and a proptest generator, then drive every text-index path with them, so a future site that
conflates a terminal column with a UTF-8 byte offset fails a test instead of shipping.

## Current State

Six defects — five Critical, one High — share exactly one root cause: **a terminal column index used as a UTF-8
byte offset or a `char`-vector index.**

| ID | Site | Trigger |
|---|---|---|
| QA-004 | `src/app/copy_mode/search.rs:113,149` | copy mode on a line with one accented char, press `/` |
| QA-005 | `src/copy_mode/motion.rs:46,50,114,118` | press `$` then `b` on a line with a wide char |
| QA-006 | `src/paste_transform/encoding.rs:63` | any codepoint ≥ U+0100 in the Paste Special preview |
| QA-007 | `src/paste_transform/encoding.rs:164-165` | even-byte-length ASCII + 2-byte char mix |
| QA-008 | `par-term-config/src/types/shader.rs:132,137` | `#1é234` in a config or shared `.glsl` |
| QA-014 | 4 display-truncation sites | non-ASCII near the cut point |

**Why 1,965 green tests missed all six**: column index, byte offset, and char index coincide *only* for
pure-ASCII single-width rows, and every test input in the suite is ASCII. The suite is not weak — it is
precise (`par-term-keybindings` asserts exact escape bytes; `par-term-config` has 140 tests) — it simply never
exercises the one condition under which these paths diverge.

Compounding it: there is **no `catch_unwind` anywhere in the workspace**, so each is an unrecoverable whole-app
crash losing all terminal state (see ENH-003).

Two correct references already exist in-tree and should be the models:
- `src/ai_inspector/panel_helpers.rs:21` — `truncate_chars`, the char-safe truncation QA-014 should reuse.
- `src/copy_mode/motion.rs:65` — the *forward* word motion, which correctly guards both bounds while its
  backward sibling does not. The asymmetry is the bug.

## Implementation

### Step 1 — Build the fixture module

Create `tests/fixtures/unicode.rs` (or `par-term-config/src/test_fixtures.rs` if it must be shared across
crates — a `pub` module gated on `#[cfg(any(test, feature = "test-fixtures"))]`).

Each fixture needs its **three lengths** recorded, because the whole bug class is confusing them:

```rust
pub struct UnicodeFixture {
    pub text: &'static str,
    pub byte_len: usize,     // text.len()
    pub char_count: usize,   // text.chars().count()
    pub display_cols: usize, // sum of unicode-width per char
}
```

Cover, at minimum:

| Category | Example | Why it matters |
|---|---|---|
| Accented Latin | `"café"` | byte_len ≠ char_count, width == char_count. **Cheapest trigger for QA-004.** |
| CJK | `"日本語"` | width == 2 × char_count → `chars().len() < cols`. **Triggers QA-005.** |
| Emoji | `"🎉"` | 4-byte scalar, width 2 |
| Combining marks | `"e\u{0301}"` | 2 chars, 1 grapheme, width 1 — grapheme ≠ char |
| Emoji + ZWJ | `"👨‍👩‍👧"` | multiple scalars, one grapheme |
| Cyrillic | `"Привет"` | 2-byte chars, width 1 — **triggers QA-006** (≥ U+0100, width 1) |
| Mixed | `"a日b🎉c"` | boundaries at irregular byte offsets |
| RTL | `"مرحبا"` | bidi; guards against ordering assumptions |
| Curly quotes | `"“x”"` | 3-byte chars in otherwise-ASCII text — realistic paste content |

Assert the three lengths in a self-test so the fixtures themselves cannot rot.

### Step 2 — Add targeted regression tests, one per defect

Each must **fail before** the corresponding fix and pass after. Write them against the fixture set:

- **QA-004**: enter copy mode with `"café"` in the row, set `cursor_col` mid-string, run search forward and
  backward. Also test with a query whose `to_lowercase()` changes byte length (e.g. `"İ"`) — that is the subtle
  half of the defect and a naive fix misses it.
- **QA-005**: build a row from the CJK fixture, `move_to_line_end()` then `move_word_backward()`. Repeat for the
  whitespace-motion sibling at `:114,118`.
- **QA-006/QA-007**: feed base64 and hex decoders the Cyrillic and mixed fixtures; assert `Err`, not panic.
- **QA-008**: `ShaderColorValue::from_hex("#1é234")` returns `Err`.
- **QA-014**: truncate each fixture at every byte offset from 0 to `byte_len` and assert no panic.

### Step 3 — Add the proptest generator

This is what makes the class *structurally* covered rather than covered by memory. Add `proptest` as a
dev-dependency in `[workspace.dependencies]` (repo policy centralizes shared deps).

Write a strategy producing strings drawn from the fixture alphabet, then property tests asserting invariants
rather than outputs — invariants survive refactoring:

```
for any string s and any column c in 0..=display_cols(s):
    column_to_byte_offset(s, c) is a char boundary of s
    column_to_byte_offset(s, c) <= s.len()
    truncate_chars(s, n).chars().count() <= n
    truncate_chars(s, n) is a prefix of s
    base64_decode(s) and hex_decode(s) never panic  (Err is fine)
    move_word_backward from any cursor_col never panics
```

The "never panics" properties are the highest-value ones — they generalize to sites nobody has audited yet.

### Step 4 — Wire the corpus into the existing highest-risk suites

Parameterize the existing copy-mode, search, and paste-transform tests over the fixture set instead of their
current ASCII literals. Where a test asserts an exact output, add the expected non-ASCII output rather than
skipping — a fixture that only checks "no panic" misses off-by-one *correctness* bugs like QA-004's
byte-offset-returned-as-column at `search.rs:114`.

### Step 5 — Document the convention

Add a short section to `CONTRIBUTING.md`: any new function taking a column, byte offset, or char index must be
tested against the corpus. Name the three-way distinction explicitly — that vocabulary is what was missing.

## Files to Touch

| File | Change |
|---|---|
| `tests/fixtures/unicode.rs` | **new** — fixture set + self-test |
| `tests/unicode_regression_tests.rs` | **new** — the six targeted regressions |
| `tests/unicode_properties.rs` | **new** — proptest invariants |
| `Cargo.toml` (root) | add `proptest` to `[workspace.dependencies]` and root `[dev-dependencies]` |
| `par-term-config/Cargo.toml` | `proptest.workspace = true` under `[dev-dependencies]` (for QA-008) |
| existing copy-mode / search / paste tests | parameterize over the corpus |
| `CONTRIBUTING.md` | document the convention |

## Verification

```bash
cargo test --workspace                      # all new tests pass
cargo test --workspace unicode              # the corpus suite specifically
make checkall
```

**The acceptance test is that these tests fail on the unfixed code.** Verify by stashing the QA fixes:

```bash
git stash                                   # remove the index-confusion fixes
cargo test --workspace unicode              # MUST fail, ideally on all six
git stash pop
```

If the corpus passes against unfixed code, it does not cover the defect it claims to — fix the corpus, not the
assertion. Also confirm no test runtime blowup: proptest defaults to 256 cases per property, which is fine, but
cap it if the suite grows past a few seconds.

## Rollback

Purely additive — new test files plus one dev-dependency. Delete them to revert; no production code changes.

Two risks worth naming. **Flaky proptest failures**: if a property fails on a rare generated input, that is a
real bug — record the seed in the failure and fix the code, do not weaken the strategy. **Fixture rot**: the
self-test in Step 1 guards the byte/char/width numbers, but `display_cols` depends on the Unicode width table
version (par-term has a configurable `unicode_version`), so pin the fixtures to one version and note it, or
compute width through the same code path the terminal uses.
