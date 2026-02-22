# Step 14: JSON Renderer

## Summary

Implement the JSON detector and renderer with syntax highlighting, proper indentation, tree-drawing guide lines, collapsible nodes, value type indicators, and large array truncation. This is the first structured-data renderer and establishes shared tree-rendering infrastructure.

## Dependencies

- **Step 1**: `ContentDetector`, `ContentRenderer` traits, types
- **Step 2**: `RegexDetector` for detection rules
- **Step 4**: `RendererRegistry` for registration
- **Step 6**: Config types for JSON renderer settings

## What to Implement

### New File: `src/prettifier/detectors/json.rs`

Create the JSON detector using the built-in rules from spec (lines 311–346):

```rust
pub fn create_json_detector() -> RegexDetector {
    RegexDetector::builder("json", "JSON")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_shortcircuit(false) // No single definitive rule for JSON
        .add_rule(DetectionRule {
            id: "json_open_brace".into(),
            pattern: Regex::new(r"^\s*\{\s*$").unwrap(),
            weight: 0.4,
            scope: RuleScope::FirstLines(3),
            strength: RuleStrength::Strong,
            ..
        })
        .add_rule(/* json_open_bracket, weight 0.35, FirstLines(3), Strong */)
        .add_rule(/* json_key_value, weight 0.3, AnyLine, Strong */)
        .add_rule(/* json_close_brace, weight 0.2, LastLines(3), Supporting */)
        .add_rule(/* json_curl_context, weight 0.3, PrecedingCommand, Supporting */)
        .add_rule(/* json_jq_context, weight 0.3, PrecedingCommand, Supporting */)
        .build()
}
```

All 6 rules from spec lines 311–346:

| Rule ID | Pattern | Weight | Scope | Strength |
|---------|---------|--------|-------|----------|
| `json_open_brace` | `^\s*\{\s*$` | 0.4 | FirstLines(3) | Strong |
| `json_open_bracket` | `^\s*\[\s*$` | 0.35 | FirstLines(3) | Strong |
| `json_key_value` | `^\s*"[^"]+"\s*:\s*` | 0.3 | AnyLine | Strong |
| `json_close_brace` | `^\s*\}\s*,?\s*$` | 0.2 | LastLines(3) | Supporting |
| `json_curl_context` | `^(curl\|http\|httpie\|wget)\s+` | 0.3 | PrecedingCommand | Supporting |
| `json_jq_context` | `^(jq\|gron\|fx)\s+` | 0.3 | PrecedingCommand | Supporting |

### New File: `src/prettifier/renderers/json.rs`

```rust
/// Renders JSON content with syntax highlighting, tree guides, and collapsible nodes.
pub struct JsonRenderer {
    config: JsonRendererConfig,
}

#[derive(Clone, Debug)]
pub struct JsonRendererConfig {
    pub indent: usize,             // Default: 2
    pub max_depth_expanded: usize, // Auto-collapse beyond this depth (default: 3)
    pub max_string_length: usize,  // Truncate long strings (default: 200)
    pub show_array_length: bool,   // Show [5 items] next to arrays (default: true)
    pub show_types: bool,          // Show type annotations (default: false)
    pub sort_keys: bool,           // Sort object keys (default: false)
    pub highlight_nulls: bool,     // Visually distinguish null (default: true)
    pub clickable_urls: bool,      // URLs in strings → OSC 8 (default: true)
}
```

**JSON rendering features** (from spec lines 1050–1075):

#### Syntax Highlighting

Color different JSON value types distinctly:
- **Keys**: One color (e.g., cyan/blue)
- **Strings**: Another color (e.g., green)
- **Numbers**: Another color (e.g., yellow)
- **Booleans**: Another color (e.g., magenta)
- **Null**: Dimmed or highlighted based on `highlight_nulls`

```rust
fn style_json_value(value: &serde_json::Value, theme: &ThemeColors) -> Vec<StyledSegment> {
    match value {
        Value::String(s) => vec![StyledSegment { text: format!("\"{}\"", s), fg: Some(theme.string_color), .. }],
        Value::Number(n) => vec![StyledSegment { text: n.to_string(), fg: Some(theme.number_color), .. }],
        Value::Bool(b) => vec![StyledSegment { text: b.to_string(), fg: Some(theme.bool_color), .. }],
        Value::Null => vec![StyledSegment { text: "null".into(), fg: Some(theme.null_color), .. }],
        _ => vec![],
    }
}
```

#### Tree-Drawing Guide Lines

Use vertical line characters (`│`) for indentation guides:

```
{
│  "name": "par-term",
│  "version": "0.16.0",
│  "features": [
│  │  "gpu-rendering",
│  │  "sixel",
│  │  "prettifier"
│  ],
│  "config": {
│  │  "fps": 60,
│  │  "vsync": true
│  }
}
```

#### Collapsible Nodes

Objects and arrays can be collapsed:
- Clicking `{` or `[` toggles fold/unfold for that node
- Auto-collapse beyond `max_depth_expanded`
- When collapsed, show summary: `{ 3 keys }` or `[ 5 items ]`

```rust
/// State for a collapsible JSON node.
pub struct CollapsibleNode {
    pub path: Vec<String>,  // JSON path to this node
    pub depth: usize,
    pub collapsed: bool,
    pub summary: String,    // e.g., "{ 3 keys }" or "[ 5 items ]"
    pub row: usize,         // Row in rendered output where this node starts
}
```

#### Value Type Indicators

When `show_types` is enabled:
- Show type annotation next to values: `"hello" (string)`, `42 (number)`, `true (bool)`
- Show array length: `[5 items]`
- Show object key count: `{3 keys}`

#### Large Array Truncation

For arrays with many elements:
- Show first N elements
- Show `... and M more items` indicator
- Click to expand full array

#### URL Detection in Strings

When `clickable_urls` is enabled, detect URLs in string values and render them as OSC 8 hyperlinks.

### Shared Tree Renderer Infrastructure

Create shared infrastructure for tree rendering (used by JSON, YAML, TOML, XML):

```rust
/// Shared tree rendering utilities.
pub mod tree_renderer {
    /// Generate tree guide characters for a given depth.
    pub fn tree_guides(depth: usize, is_last: bool) -> String { ... }

    /// Generate a collapsed summary for a container node.
    pub fn collapsed_summary(node_type: &str, count: usize) -> String { ... }
}
```

### JSON Parsing

Use `serde_json` to parse JSON content, then render the parsed AST:

```rust
impl ContentRenderer for JsonRenderer {
    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError> {
        let text = content.lines.join("\n");
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| RenderError::RenderFailed(format!("Invalid JSON: {}", e)))?;

        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();
        self.render_value(&value, 0, &mut lines, &mut line_mapping, config);

        Ok(RenderedContent {
            styled_lines: lines,
            line_mapping,
            graphics: vec![],
            format_badge: "{}".to_string(),
        })
    }
}
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/detectors/json.rs` |
| Create | `src/prettifier/renderers/json.rs` |
| Create | `src/prettifier/renderers/tree_renderer.rs` (shared tree utilities) |
| Modify | `src/prettifier/detectors/mod.rs` (add `pub mod json;`) |
| Modify | `src/prettifier/renderers/mod.rs` (add `pub mod json; pub mod tree_renderer;`) |

## Relevant Spec Sections

- **Lines 311–346**: JSON detection rules
- **Lines 1050–1075**: JSON renderer specification — features and config
- **Lines 1339**: Phase 2a — JSON prettifier with collapsible nodes
- **Lines 1356–1366**: Shared infrastructure — tree renderer used by JSON, YAML, TOML, XML
- **Lines 1469–1470**: Acceptance criteria — JSON auto-detected, syntax highlighting, indentation, collapsible

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] JSON detection rules match spec (6 rules with correct weights and scopes)
- [ ] Valid JSON is parsed and rendered with syntax highlighting
- [ ] Keys, strings, numbers, booleans, and nulls have distinct colors
- [ ] Tree guide lines display at each indentation level
- [ ] Collapsible nodes auto-collapse beyond `max_depth_expanded`
- [ ] Collapsed nodes show key/item count summary
- [ ] Large arrays are truncated with "... and N more items"
- [ ] Long strings are truncated with "..."
- [ ] URL strings are rendered as OSC 8 hyperlinks when `clickable_urls` is true
- [ ] Object keys are sorted when `sort_keys` is true
- [ ] Invalid JSON produces an appropriate error
- [ ] Tree renderer utilities are reusable by other renderers
- [ ] Unit tests for parsing, highlighting, collapsing, truncation
