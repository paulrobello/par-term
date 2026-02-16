# Step 7: Markdown Detection Rules

## Summary

Implement the built-in markdown detection rules as a `RegexDetector` instance. This creates the first concrete format detector using the `RegexDetector` framework from Step 2, with all 12+ regex rules from the spec for identifying markdown content in terminal output.

## Dependencies

- **Step 2**: `RegexDetector`, `RegexDetectorBuilder`, `DetectionRule`
- **Step 1**: `RuleScope`, `RuleStrength`, `RuleSource`

## What to Implement

### New File: `src/prettifier/detectors/markdown.rs`

Create the built-in markdown `RegexDetector` with all rules from the spec (lines 220–308):

```rust
/// Create the built-in Markdown detector with default regex rules.
pub fn create_markdown_detector() -> RegexDetector {
    RegexDetector::builder("markdown", "Markdown")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_shortcircuit(true)
        // Definitive rules
        .add_rule(DetectionRule {
            id: "md_fenced_code".into(),
            pattern: Regex::new(r"^```\w*\s*$").unwrap(),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Fenced code block opening (``` or ```language)".into(),
            enabled: true,
        })
        .add_rule(DetectionRule {
            id: "md_fenced_tilde".into(),
            pattern: Regex::new(r"^~~~\w*\s*$").unwrap(),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Tilde-style fenced code block".into(),
            enabled: true,
        })
        // Strong rules
        .add_rule(DetectionRule {
            id: "md_atx_header".into(),
            pattern: Regex::new(r"^#{1,6}\s+\S").unwrap(),
            weight: 0.5,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "ATX-style header (# through ######)".into(),
            enabled: true,
        })
        .add_rule(DetectionRule {
            id: "md_table".into(),
            pattern: Regex::new(r"^\|.*\|.*\|").unwrap(),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Markdown table row with pipe delimiters".into(),
            enabled: true,
        })
        // Supporting rules
        .add_rule(/* md_table_separator */)
        .add_rule(/* md_bold */)
        .add_rule(/* md_italic */)
        .add_rule(/* md_link */)
        .add_rule(/* md_list_bullet */)
        .add_rule(/* md_list_ordered */)
        .add_rule(/* md_blockquote */)
        .add_rule(/* md_inline_code */)
        .add_rule(/* md_horizontal_rule */)
        // Command context rule
        .add_rule(DetectionRule {
            id: "md_claude_code_context".into(),
            pattern: Regex::new(r"(claude|cc|claude-code)").unwrap(),
            weight: 0.2,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Output follows a Claude Code command".into(),
            enabled: true,
        })
        .build()
}
```

All 12+ rules from spec lines 220–308 must be implemented:

| Rule ID | Pattern | Weight | Scope | Strength |
|---------|---------|--------|-------|----------|
| `md_fenced_code` | `^```\w*\s*$` | 0.8 | AnyLine | Definitive |
| `md_fenced_tilde` | `^~~~\w*\s*$` | 0.8 | AnyLine | Definitive |
| `md_atx_header` | `^#{1,6}\s+\S` | 0.5 | AnyLine | Strong |
| `md_table` | `^\|.*\|.*\|` | 0.4 | AnyLine | Strong |
| `md_table_separator` | `^\|[\s\-:\|]+\|` | 0.3 | AnyLine | Supporting |
| `md_bold` | `\*\*[^*]+\*\*` | 0.2 | AnyLine | Supporting |
| `md_italic` | `(?<!\*)\*[^*]+\*(?!\*)` | 0.15 | AnyLine | Supporting |
| `md_link` | `\[([^\]]+)\]\(([^)]+)\)` | 0.2 | AnyLine | Supporting |
| `md_list_bullet` | `^\s*[-*+]\s+\S` | 0.15 | AnyLine | Supporting |
| `md_list_ordered` | `^\s*\d+[.)]\s+\S` | 0.15 | AnyLine | Supporting |
| `md_blockquote` | `^>\s+` | 0.15 | AnyLine | Supporting |
| `md_inline_code` | `` `[^`]+` `` | 0.1 | AnyLine | Supporting |
| `md_horizontal_rule` | `^([-*_])\s*\1\s*\1[\s\1]*$` | 0.15 | AnyLine | Supporting |
| `md_claude_code_context` | `(claude\|cc\|claude-code)` | 0.2 | PrecedingCommand | Supporting |

### New Directory: `src/prettifier/detectors/`

```
src/prettifier/detectors/
├── mod.rs          # pub mod markdown;
└── markdown.rs     # create_markdown_detector()
```

### Register with Registry

Provide a function to register the markdown detector with a `RendererRegistry`:

```rust
/// Register the markdown detector (and later, renderer) with the registry.
pub fn register_markdown(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.markdown.enabled {
        let detector = create_markdown_detector();
        registry.register_detector(config.markdown.priority, Box::new(detector));
    }
}
```

### Unit Tests

Comprehensive detection tests:

1. **Fenced code block** — content with ` ``` ` lines → detected as markdown with high confidence
2. **Headers only** — content with `# Title` → detected as markdown (Strong rule, above threshold)
3. **Mixed signals** — content with bold, links, and lists → cumulative confidence exceeds threshold
4. **Below threshold** — content with only a single inline code span → NOT detected (too low)
5. **Claude Code context** — content following `claude` command gets confidence boost
6. **Table detection** — content with pipe-delimited rows → detected
7. **False positive resistance** — shell script output with `#` comments → NOT detected as markdown (no supporting signals)
8. **Not markdown** — JSON content → NOT detected as markdown

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/detectors/mod.rs` |
| Create | `src/prettifier/detectors/markdown.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod detectors;`) |

## Relevant Spec Sections

- **Lines 220–308**: Full markdown detection rule set with all patterns, weights, scopes, and strengths
- **Lines 920–928**: Markdown detection heuristics summary
- **Lines 148–203**: How `RegexDetector::detect()` processes rules (implemented in Step 2)

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] All 14 regex rules compile without error
- [ ] Fenced code blocks trigger definitive detection (confidence = 1.0)
- [ ] ATX headers alone produce confidence >= 0.5 (Strong rule)
- [ ] Multiple supporting signals (bold + link + list) sum to >= 0.6 threshold
- [ ] Content with only a single weak signal is NOT detected
- [ ] `PrecedingCommand` rule correctly checks the command context
- [ ] `quick_match()` returns true for content with headers or code blocks in first 5 lines
- [ ] `quick_match()` returns false for plain text without markdown signals
- [ ] Registration function respects `enabled` and `priority` from config
- [ ] All unit tests pass
