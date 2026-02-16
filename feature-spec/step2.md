# Step 2: RegexDetector Implementation

## Summary

Implement the `RegexDetector` struct — the standard, reusable implementation of `ContentDetector` that powers all built-in format detectors. This is the weighted confidence-scoring engine described in the spec, with support for rule scoping, strength-based short-circuiting, and minimum matching thresholds.

## Dependencies

- **Step 1**: Core types and traits (`ContentDetector`, `ContentBlock`, `DetectionResult`, `DetectionRule`, `RuleScope`, `RuleStrength`)

## What to Implement

### New File: `src/prettifier/regex_detector.rs`

```rust
/// The standard regex-based ContentDetector implementation.
/// Most formats use this rather than implementing ContentDetector from scratch.
pub struct RegexDetector {
    format_id: String,
    display_name: String,
    rules: Vec<DetectionRule>,
    /// Minimum total confidence score to trigger detection
    confidence_threshold: f32,
    /// Minimum number of rules that must match
    min_matching_rules: usize,
    /// If true, a single Definitive rule match bypasses threshold/count checks
    definitive_rule_shortcircuit: bool,
}
```

**Core detection algorithm** (from spec lines 162–203):

1. Iterate over all enabled rules
2. For each rule, extract the appropriate text based on `rule.scope`:
   - `AnyLine` — test each line individually, match if any line matches
   - `FirstLines(n)` — test only the first `n` lines
   - `LastLines(n)` — test only the last `n` lines
   - `FullBlock` — join all lines and test as a single string
   - `PrecedingCommand` — test `content.preceding_command` (skip if None)
3. If the rule's `command_context` is set, first check if `preceding_command` matches it; skip rule if not
4. If the rule's pattern matches:
   - Add `rule.weight` to `total_confidence`
   - Increment `match_count`
   - If `rule.strength == Definitive` and `self.definitive_rule_shortcircuit`, return immediately with confidence 1.0
5. After all rules: if `match_count >= min_matching_rules` AND `total_confidence >= confidence_threshold`, return `Some(DetectionResult)` with `confidence = total_confidence.min(1.0)`
6. Otherwise return `None`

**`quick_match()` implementation**: Run only `FirstLines(n)` and `AnyLine` Strong/Definitive rules against the first 5 lines for a fast pre-filter. Return `true` if any pattern matches.

**Builder pattern**: Provide a `RegexDetectorBuilder` for ergonomic construction:

```rust
impl RegexDetector {
    pub fn builder(format_id: &str, display_name: &str) -> RegexDetectorBuilder { ... }
}

pub struct RegexDetectorBuilder { ... }

impl RegexDetectorBuilder {
    pub fn add_rule(self, rule: DetectionRule) -> Self { ... }
    pub fn confidence_threshold(self, threshold: f32) -> Self { ... }
    pub fn min_matching_rules(self, count: usize) -> Self { ... }
    pub fn definitive_shortcircuit(self, enabled: bool) -> Self { ... }
    pub fn build(self) -> RegexDetector { ... }
}
```

**Rule merging support**: Method to merge user-defined rules into an existing detector:

```rust
impl RegexDetector {
    /// Merge user-defined rules. User rules with the same ID as built-in rules
    /// override the built-in rule's weight/scope/enabled fields.
    pub fn merge_user_rules(&mut self, user_rules: Vec<DetectionRule>) { ... }

    /// Apply overrides (enable/disable, weight changes) to existing rules by ID.
    pub fn apply_overrides(&mut self, overrides: Vec<RuleOverride>) { ... }
}

pub struct RuleOverride {
    pub id: String,
    pub enabled: Option<bool>,
    pub weight: Option<f32>,
    pub scope: Option<RuleScope>,
}
```

### Unit Tests

Write comprehensive tests in `src/prettifier/regex_detector.rs` (or a separate test file):

1. **Basic detection**: Create a detector with 2 rules, provide matching content, verify detection result
2. **Confidence threshold**: Verify that below-threshold scores return `None`
3. **Definitive short-circuit**: A single Definitive rule match returns confidence 1.0 immediately
4. **Min matching rules**: Verify that insufficient rule matches return `None` even if confidence is high
5. **Rule scoping**: Test `FirstLines`, `LastLines`, `FullBlock`, `PrecedingCommand` each apply to correct text
6. **Command context filter**: Verify rules with `command_context` only fire when preceding command matches
7. **Disabled rules**: Rules with `enabled: false` are skipped
8. **User rule merging**: User rules with same ID override built-in; new IDs are appended
9. **Quick match**: Fast-path returns true/false correctly

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/regex_detector.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod regex_detector;`) |

## Relevant Spec Sections

- **Lines 148–203**: `RegexDetector` struct and full `detect()` algorithm
- **Lines 96–147**: `DetectionRule` struct, `RuleScope`, `RuleStrength`, `RuleSource` enums
- **Lines 627–728**: User-extensible regex rules — adding rules, creating detectors, disabling/overriding rules
- **Lines 1346–1348**: Performance — quick_match() for fast rejection

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `cargo test` — all unit tests pass
- [ ] `RegexDetector` correctly implements `ContentDetector` trait
- [ ] Detection algorithm matches spec: weighted scores, threshold check, min_matching_rules, definitive short-circuit
- [ ] All five `RuleScope` variants are correctly handled
- [ ] `quick_match()` only checks first few lines with Strong/Definitive rules
- [ ] Rule merging correctly handles ID conflicts (user overrides built-in)
- [ ] `RuleOverride` can disable, re-weight, and re-scope existing rules
- [ ] Disabled rules do not contribute to confidence scoring
- [ ] `command_context` filter correctly gates rule evaluation
