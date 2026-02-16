# Step 11: Trigger System Integration

## Summary

Register the `Prettify` action type in par-term's existing trigger/action system (v0.11.0), enabling users to create trigger rules that invoke specific renderers on matched content. This provides a second entry point to the renderer registry (alongside auto-detection) and supports suppressing auto-detection with `prettify_format: "none"`.

## Dependencies

- **Step 4**: `PrettifierPipeline` with `trigger_prettify()` method, `RendererRegistry`
- **Step 6**: Config structures for trigger prettify settings

## What to Implement

### Modify: Existing Trigger/Action System

Locate the existing trigger action types (the spec references 7 existing actions: Highlight Line, Highlight Text, Post Notification, Set Mark, Send Text, Run Command, Run Coprocess) and add the 8th: **Prettify**.

#### New Action Type

```rust
/// Add to the existing action enum:
pub enum TriggerAction {
    // ... existing 7 actions ...
    /// Invoke a specific prettifier renderer on matched content
    Prettify {
        /// Which renderer to invoke (e.g., "json", "markdown", "none")
        format: String,
        /// What scope to apply the renderer to
        scope: PrettifyScope,
        /// Optional: regex for block end (for block-scoped rendering)
        block_end: Option<String>,
        /// Optional: sub-format (e.g., "plantuml" for diagrams)
        sub_format: Option<String>,
    },
}

pub enum PrettifyScope {
    /// Apply to the matched line only
    Line,
    /// Apply to a delimited block (start pattern → block_end pattern)
    Block,
    /// Apply to the entire command output containing the match
    CommandOutput,
}
```

#### Trigger Configuration

Support the trigger config format from spec (lines 450–496):

```yaml
triggers:
  - name: "Prettify myapi output"
    regex: '^\{"api_version":'
    action: prettify
    prettify_format: "json"
    prettify_scope: "command_output"
    enabled: true

  - name: "Skip prettifier for bat"
    regex: '.'
    command_filter: '^bat\s+'
    action: prettify
    prettify_format: "none"        # Suppress auto-detection
    prettify_scope: "command_output"
    enabled: true
```

#### Config Deserialization

Add serde support for the new trigger fields:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub name: String,
    pub regex: String,
    pub action: String,  // "prettify", "highlight_line", etc.
    #[serde(default)]
    pub command_filter: Option<String>,
    // Prettify-specific fields (only used when action = "prettify")
    #[serde(default)]
    pub prettify_format: Option<String>,
    #[serde(default)]
    pub prettify_scope: Option<String>,
    #[serde(default)]
    pub prettify_block_end: Option<String>,
    #[serde(default)]
    pub prettify_sub_format: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
```

### New File: `src/app/triggers.rs` (or modify existing trigger handler)

#### Trigger → Prettify Dispatch

When a trigger with `action: prettify` fires:

1. **Check master toggle**: If prettifying is disabled (globally or by session toggle), skip
2. **Handle "none" format**: If `prettify_format == "none"`, mark the content as "suppress auto-detection" — the auto-detection pipeline will skip this block
3. **Determine scope**:
   - `line`: Create a `ContentBlock` with just the matched line
   - `command_output`: Create a `ContentBlock` with the full output of the current command (from OSC 133 markers)
   - `block`: Accumulate lines from the trigger match until `block_end` regex matches, then create a `ContentBlock`
4. **Dispatch**: Call `PrettifierPipeline::trigger_prettify(format_id, content_block)` — this bypasses confidence scoring and goes directly to the renderer registry with confidence 1.0
5. **Command filter**: If `command_filter` is set, only fire the trigger when the preceding command matches the filter regex

```rust
/// Handle a Prettify trigger action.
pub fn handle_prettify_trigger(
    pipeline: &mut PrettifierPipeline,
    trigger: &TriggerConfig,
    matched_line: &str,
    current_command: Option<&str>,
    terminal_output: &[String],
) -> TriggerResult { ... }
```

#### Suppression Tracking

Maintain a set of content block IDs or row ranges that have been suppressed via `prettify_format: "none"`. The auto-detection pipeline checks this set before running detectors:

```rust
impl PrettifierPipeline {
    /// Mark a row range as suppressed (no auto-detection).
    pub fn suppress_detection(&mut self, row_range: Range<usize>) { ... }

    /// Check if auto-detection is suppressed for a row range.
    pub fn is_suppressed(&self, row_range: &Range<usize>) -> bool { ... }
}
```

### Integration Flow

```
Terminal output line arrives
    │
    ├─→ Trigger engine checks all trigger rules (existing flow)
    │       │
    │       ├─→ If action = "prettify" and format != "none"
    │       │       → dispatch to PrettifierPipeline::trigger_prettify()
    │       │
    │       ├─→ If action = "prettify" and format == "none"
    │       │       → suppress auto-detection for this command output
    │       │
    │       └─→ Other actions → existing handling
    │
    └─→ Boundary detector accumulates lines (existing Step 3 flow)
            │
            └─→ On boundary: check suppression → if not suppressed → auto-detect
```

## Key Files

| Action | Path |
|--------|------|
| Modify | Existing trigger action enum (add `Prettify` variant) |
| Modify | Existing trigger config deserialization (add prettify fields) |
| Create | `src/app/triggers.rs` (or modify existing trigger handler) |
| Modify | `src/prettifier/pipeline.rs` (add suppression tracking) |
| Modify | `src/config/prettifier.rs` (trigger prettify config types) |

## Relevant Spec Sections

- **Lines 416–506**: Full trigger system integration specification
- **Lines 438–496**: Trigger → Prettify configuration YAML examples
- **Lines 498–506**: Key integration points — one-way bridge, "none" format, command_filter, block scope
- **Lines 564–625**: Detection pipeline diagram showing both trigger and auto-detection paths
- **Lines 1322**: "Prettifier registers as new 8th action type in existing trigger/action system"
- **Lines 1440–1444**: Acceptance criteria for trigger integration

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `Prettify` action type is registered alongside existing 7 action types
- [ ] Trigger with `prettify_format: "json"` dispatches directly to JSON renderer
- [ ] Trigger with `prettify_format: "none"` suppresses auto-detection for matched content
- [ ] `prettify_scope: "line"` creates a single-line content block
- [ ] `prettify_scope: "command_output"` creates a block from the full command output
- [ ] `prettify_scope: "block"` accumulates until `prettify_block_end` matches
- [ ] `command_filter` correctly scopes triggers to specific command contexts
- [ ] Trigger-based prettifying respects the master `enable_prettifier` toggle
- [ ] Trigger-based prettifying bypasses confidence scoring (confidence = 1.0)
- [ ] Suppression tracking prevents auto-detection on suppressed blocks
- [ ] Trigger config deserializes correctly from YAML
- [ ] Existing trigger actions continue to work unchanged
- [ ] Unit tests for dispatch, suppression, scope handling
