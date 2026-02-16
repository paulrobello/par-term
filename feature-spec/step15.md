# Step 15: YAML & TOML Renderers

## Summary

Implement YAML and TOML detectors and renderers with syntax highlighting, indentation guides, collapsible sections, and format-specific features (YAML anchor/alias indicators, TOML section headers). Both share the tree-rendering infrastructure created in Step 14.

## Dependencies

- **Step 1**: Core traits and types
- **Step 2**: `RegexDetector`
- **Step 4**: `RendererRegistry`
- **Step 14**: Shared tree renderer infrastructure (`tree_renderer.rs`)

## What to Implement

### YAML Detector

#### New File: `src/prettifier/detectors/yaml.rs`

Rules from spec lines 348–372:

```rust
pub fn create_yaml_detector() -> RegexDetector {
    RegexDetector::builder("yaml", "YAML")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_shortcircuit(true)
        .add_rule(/* yaml_doc_start: ^---\s*$, weight 0.5, FirstLines(3), Definitive */)
        .add_rule(/* yaml_key_value: ^[a-zA-Z_]\w*:\s+, weight 0.3, AnyLine, Strong */)
        .add_rule(/* yaml_nested: ^\s{2,}[a-zA-Z_]\w*:\s+, weight 0.2, AnyLine, Supporting */)
        .add_rule(/* yaml_list: ^\s*-\s+[a-zA-Z_], weight 0.15, AnyLine, Supporting */)
        .build()
}
```

| Rule ID | Pattern | Weight | Scope | Strength |
|---------|---------|--------|-------|----------|
| `yaml_doc_start` | `^---\s*$` | 0.5 | FirstLines(3) | Definitive |
| `yaml_key_value` | `^[a-zA-Z_][a-zA-Z0-9_]*:\s+` | 0.3 | AnyLine | Strong |
| `yaml_nested` | `^\s{2,}[a-zA-Z_][a-zA-Z0-9_]*:\s+` | 0.2 | AnyLine | Supporting |
| `yaml_list` | `^\s*-\s+[a-zA-Z_]` | 0.15 | AnyLine | Supporting |

**Important disambiguation**: YAML `---` can conflict with Markdown horizontal rules. The detector should require additional YAML signals (key-value pairs) alongside `---` to avoid false positives. The `min_matching_rules: 2` requirement helps here.

### YAML Renderer

#### New File: `src/prettifier/renderers/yaml.rs`

```rust
pub struct YamlRenderer {
    config: YamlRendererConfig,
}

impl ContentRenderer for YamlRenderer {
    fn format_id(&self) -> &str { "yaml" }
    fn display_name(&self) -> &str { "YAML" }
    fn capabilities(&self) -> Vec<RendererCapability> { vec![RendererCapability::TextStyling] }
    fn supports_format(&self, format_id: &str) -> bool { format_id == "yaml" }

    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError> { ... }
}
```

**YAML rendering features** (from spec lines 1077–1085):

1. **Syntax highlighting**: Distinct colors for:
   - Keys (bold or colored)
   - String values, numbers, booleans
   - Anchors (`&anchor`) and aliases (`*alias`) in special color
   - Tags (`!!str`, `!!int`) dimmed
   - Comments dimmed
2. **Indentation guide lines**: Use the shared tree renderer to show `│` guides at each indentation level
3. **Collapsible sections**: YAML mapping keys with nested children can be collapsed
4. **Anchor/alias resolution**: On hover or with a visual indicator, show what `*alias` resolves to
5. **Document separator styling**: `---` rendered as a prominent visual separator

**YAML parsing approach**: Use a line-by-line parser that tracks indentation depth rather than a full YAML parser. This handles display without needing to validate the YAML:

```rust
fn classify_yaml_line(line: &str) -> YamlLineType {
    if line.trim() == "---" { YamlLineType::DocumentStart }
    else if line.trim() == "..." { YamlLineType::DocumentEnd }
    else if line.trim().starts_with('#') { YamlLineType::Comment }
    else if /* key: value pattern */ { YamlLineType::KeyValue { indent, key, value } }
    else if line.trim().starts_with("- ") { YamlLineType::ListItem { indent } }
    else { YamlLineType::Continuation }
}
```

### TOML Detector

#### New File: `src/prettifier/detectors/toml.rs`

TOML detection rules (spec mentions `[section]` headers, `key = "value"`, `[[array]]`):

```rust
pub fn create_toml_detector() -> RegexDetector {
    RegexDetector::builder("toml", "TOML")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_shortcircuit(false)
        .add_rule(DetectionRule {
            id: "toml_section_header".into(),
            pattern: Regex::new(r"^\[[\w.-]+\]\s*$").unwrap(),
            weight: 0.5,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        .add_rule(/* toml_array_table: ^\[\[[\w.-]+\]\]\s*$, weight 0.6, AnyLine, Definitive */)
        .add_rule(/* toml_key_value: ^[\w.-]+\s*=\s*, weight 0.3, AnyLine, Strong */)
        .add_rule(/* toml_string_value: =\s*"[^"]*"\s*$, weight 0.2, AnyLine, Supporting */)
        .add_rule(/* toml_comment: ^\s*#, weight 0.1, AnyLine, Supporting */)
        .build()
}
```

### TOML Renderer

#### New File: `src/prettifier/renderers/toml.rs`

```rust
pub struct TomlRenderer {
    config: TomlRendererConfig,
}
```

**TOML rendering features** (from spec lines 1087–1095):

1. **Section headers styled prominently**: `[section]` rendered with bold/colored text, possibly with a background
2. **Array table headers**: `[[array]]` styled distinctly from regular section headers
3. **Key-value alignment**: Align `=` signs within a section for readability
4. **Inline table expansion**: Inline tables `{ a = 1, b = 2 }` can be expanded to multi-line
5. **Type-aware value coloring**: Strings, integers, floats, booleans, dates in distinct colors
6. **Comment dimming**: Comments styled as dimmed text

**TOML parsing**: Line-by-line classification similar to YAML:

```rust
fn classify_toml_line(line: &str) -> TomlLineType {
    if line.trim().starts_with('[') && !line.trim().starts_with("[[") { TomlLineType::SectionHeader }
    else if line.trim().starts_with("[[") { TomlLineType::ArrayTable }
    else if line.trim().starts_with('#') { TomlLineType::Comment }
    else if /* key = value pattern */ { TomlLineType::KeyValue { key, value } }
    else { TomlLineType::Other }
}
```

### Registration

Register both detectors and renderers with the registry:

```rust
pub fn register_yaml(registry: &mut RendererRegistry, config: &RenderersConfig) { ... }
pub fn register_toml(registry: &mut RendererRegistry, config: &RenderersConfig) { ... }
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/detectors/yaml.rs` |
| Create | `src/prettifier/detectors/toml.rs` |
| Create | `src/prettifier/renderers/yaml.rs` |
| Create | `src/prettifier/renderers/toml.rs` |
| Modify | `src/prettifier/detectors/mod.rs` (add modules) |
| Modify | `src/prettifier/renderers/mod.rs` (add modules) |

## Relevant Spec Sections

- **Lines 348–372**: YAML detection rules
- **Lines 1077–1085**: YAML renderer features
- **Lines 1087–1095**: TOML renderer features
- **Lines 1339**: Phase 2a — YAML, TOML share tree-rendering infrastructure with JSON
- **Lines 1362**: Shared tree renderer infrastructure used by JSON, YAML, TOML, XML
- **Lines 1471**: Acceptance criteria — YAML and TOML auto-detected and rendered

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] YAML `---` document start triggers detection (with supporting signals)
- [ ] YAML key-value pairs are highlighted with distinct key/value colors
- [ ] YAML indentation guides display correctly
- [ ] YAML anchors and aliases are visually distinct
- [ ] YAML detection does not false-positive on Markdown horizontal rules alone
- [ ] TOML `[section]` headers are rendered prominently
- [ ] TOML `[[array]]` tables are styled distinctly
- [ ] TOML key-value pairs have aligned `=` signs
- [ ] TOML values are colored by type (string, number, boolean, date)
- [ ] Both renderers use the shared tree renderer for indentation guides
- [ ] Collapsible sections work for both YAML and TOML
- [ ] Both detectors and renderers register correctly with the registry
- [ ] Unit tests for detection, rendering, and edge cases
