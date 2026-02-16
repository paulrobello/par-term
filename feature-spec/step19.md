# Step 19: User Extensibility, Custom Renderers & Settings UI

## Summary

Implement the full user extensibility layer (custom renderer registration, custom regex-only detectors, fenced block language registration) and the comprehensive Settings UI for the Content Prettifier system. This is the capstone step that makes the framework user-configurable and discoverable through the GUI.

## Dependencies

- **Steps 1–6**: Full framework (types, traits, registry, pipeline, config)
- **Steps 7–18**: All built-in detectors and renderers
- **Step 11**: Trigger system integration

## What to Implement

### Part A: Custom Renderer Registration

#### Extend: `src/prettifier/registry.rs`

Support registering user-defined renderers from config (spec lines 1223–1258):

```rust
/// A user-defined renderer that delegates to an external command.
pub struct ExternalCommandRenderer {
    format_id: String,
    display_name: String,
    render_command: String,
    render_args: Vec<String>,
    render_type: ExternalRenderType,
    cache: bool,
}

pub enum ExternalRenderType {
    Text,   // Command outputs styled text (ANSI escape codes)
    Image,  // Command outputs an image (PNG/SVG)
}

impl ContentRenderer for ExternalCommandRenderer {
    fn format_id(&self) -> &str { &self.format_id }
    fn display_name(&self) -> &str { &self.display_name }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::ExternalCommand]
    }

    fn render(&self, content: &ContentBlock, _config: &RendererConfig) -> Result<RenderedContent, RenderError> {
        // 1. Pipe content to external command
        // 2. Capture output
        // 3. Parse ANSI output into StyledLines (for text type)
        //    Or decode image (for image type)
        // 4. Return RenderedContent
        ...
    }

    fn supports_format(&self, format_id: &str) -> bool {
        format_id == self.format_id
    }
}
```

**Loading custom renderers from config** (spec lines 1232–1258, 658–710):

```rust
/// Load and register custom renderers from config.
pub fn register_custom_renderers(
    registry: &mut RendererRegistry,
    custom_configs: &[CustomRendererConfig],
) {
    for config in custom_configs {
        // Create a RegexDetector from the config's detection rules
        let detector = create_custom_detector(config);
        registry.register_detector(50, Box::new(detector)); // Default priority for custom

        // Create an ExternalCommandRenderer
        if let Some(ref command) = config.render_command {
            let renderer = ExternalCommandRenderer {
                format_id: config.format_id.clone(),
                display_name: config.display_name.clone(),
                render_command: command.clone(),
                render_args: config.render_args.clone().unwrap_or_default(),
                render_type: config.render_type.clone().into(),
                cache: config.cache.unwrap_or(true),
            };
            registry.register_renderer(&config.format_id, Box::new(renderer));
        }
    }
}
```

### Part B: Custom Fenced Block Languages

#### Extend: `src/prettifier/renderers/diagrams.rs`

Support user-registered diagram languages (spec lines 1262–1285):

```rust
/// Register custom diagram languages from config.
pub fn register_custom_diagram_languages(
    renderer: &mut DiagramRenderer,
    languages: &[CustomDiagramLanguage],
) {
    for lang in languages {
        renderer.add_language(DiagramLanguage {
            tag: lang.tag.clone(),
            display_name: lang.display_name.clone(),
            kroki_type: lang.kroki_type.clone(),
            local_command: lang.local_command.clone(),
            local_args: lang.local_args.clone().unwrap_or_default(),
        });
    }
}
```

### Part C: User-Defined Regex Detectors

Support creating entirely new detectors from regex patterns alone (spec lines 658–710):

```rust
/// Create a new detector entirely from user-defined regex rules (no Rust code needed).
fn create_custom_detector(config: &CustomRendererConfig) -> RegexDetector {
    let mut builder = RegexDetector::builder(&config.format_id, &config.display_name)
        .confidence_threshold(config.confidence_threshold.unwrap_or(0.6))
        .min_matching_rules(config.min_matching_rules.unwrap_or(1))
        .definitive_shortcircuit(true);

    for rule in &config.detection_rules {
        builder = builder.add_rule(rule.clone().into());
    }

    builder.build()
}
```

### Part D: Settings UI — Prettifier Tab

#### New File: `src/settings_ui/prettifier_tab.rs`

Create a comprehensive Settings UI tab for the Content Prettifier (spec lines 1370–1428):

```rust
/// Renders the Content Prettifier settings tab in the egui-based Settings UI.
pub fn prettifier_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    changes_this_frame: &mut bool,
    registry: &RendererRegistry,
) {
    // 1. Master toggle
    // 2. Detection settings
    // 3. Per-renderer cards
    // 4. Custom renderers section
    // 5. Detection rules section
    // 6. Profile override indicators
}
```

**Top-level controls:**

1. **Enable Prettifier toggle**:
   - Maps to `enable_prettifier` in config
   - Dynamic subtitle listing enabled renderers: *"Automatically detects and renders structured content including {format_list}."*
   - Scope badge: `[Global]` or `[Profile: {name}]`
   - "Reset to global" link when profile-overridden

```rust
fn render_master_toggle(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    registry: &RendererRegistry,
    changes: &mut bool,
) {
    let formats: Vec<&str> = registry.registered_formats()
        .iter().map(|(_, name)| *name).collect();
    let subtitle = format!(
        "Automatically detects and renders structured content including {}.",
        formats.join(", ")
    );

    ui.horizontal(|ui| {
        let mut enabled = settings.config.enable_prettifier;
        if ui.checkbox(&mut enabled, "Enable Prettifier").changed() {
            settings.config.enable_prettifier = enabled;
            settings.has_changes = true;
            *changes = true;
        }
        // Scope badge
        render_scope_badge(ui, settings, "enable_prettifier");
    });
    ui.label(egui::RichText::new(subtitle).small().weak());
}
```

2. **Detection scope** dropdown: Command Output / All / Manual Only
3. **Confidence threshold** slider (0.0 - 1.0)
4. **Global toggle keybinding** display

**Per-renderer cards** (collapsible, one per registered renderer):

```rust
fn render_renderer_card(
    ui: &mut egui::Ui,
    format_id: &str,
    display_name: &str,
    settings: &mut Settings,
    changes: &mut bool,
) {
    egui::CollapsingHeader::new(display_name)
        .default_open(false)
        .show(ui, |ui| {
            // Enable/disable toggle
            // Priority slider
            // Renderer-specific settings
            // "Overridden by profile" badge if applicable
        });
}
```

**Per-renderer tabs/cards features:**
- Enable/disable toggle with profile override indicator
- Priority slider
- Renderer-specific settings (expandable)
- "Test detection" button — paste sample content to verify detection works
- Detection confidence preview — shows what score sample content gets

**Profile override section** (within Profile editor):

```rust
fn render_profile_prettifier_overrides(
    ui: &mut egui::Ui,
    profile: &mut Profile,
    global_config: &Config,
    changes: &mut bool,
) {
    // Tri-state toggle: On / Off / Inherit
    let current = profile.enable_prettifier;
    let label = match current {
        Some(true) => "On",
        Some(false) => "Off",
        None => "Inherit from global",
    };
    // ... tri-state selector UI

    // Collapsible "Prettifier Overrides" panel
    // "Clear all overrides" button
    // Visual diff indicators for changed settings
}
```

**Detection rules section** (spec lines 1416–1423):

```rust
fn render_detection_rules(
    ui: &mut egui::Ui,
    format_id: &str,
    detector: &dyn ContentDetector,
    settings: &mut Settings,
    changes: &mut bool,
) {
    // Table: ID | Pattern | Weight | Scope | Strength | Source | Enabled
    egui::Grid::new(format!("rules_{}", format_id))
        .striped(true)
        .show(ui, |ui| {
            for rule in detector.detection_rules() {
                ui.label(&rule.id);
                ui.label(rule.pattern.as_str());
                ui.label(format!("{:.2}", rule.weight));
                ui.label(format!("{:?}", rule.scope));
                ui.label(format!("{:?}", rule.strength));
                ui.label(match rule.source {
                    RuleSource::BuiltIn => "Built-in",
                    RuleSource::UserDefined => "User",
                });
                // Checkbox for enable/disable
                let mut enabled = rule.enabled;
                if ui.checkbox(&mut enabled, "").changed() {
                    // Apply override
                }
                ui.end_row();
            }
        });

    // "Add rule" button
    // "Test rules" button with sample content input
}
```

**Test rules feature:**

```rust
fn render_test_rules(
    ui: &mut egui::Ui,
    detector: &dyn ContentDetector,
    test_content: &mut String,
) {
    ui.text_edit_multiline(test_content);
    if ui.button("Test Detection").clicked() {
        let block = ContentBlock::from_text(test_content);
        if let Some(result) = detector.detect(&block) {
            // Show which rules fired, their weights, and total confidence
            // Visual confidence meter
        } else {
            // Show "No detection"
        }
    }
}
```

**Trigger integration in Automation tab** (spec lines 1424–1427):

Modify the existing Settings > Automation trigger editor to:
1. Include `Prettify` in the action type dropdown
2. When Prettify is selected, show: format selector, scope dropdown, block-end regex, command filter
3. Include "Suppress auto-detection" (`none`) option

**Diagram settings subsection:**
- Backend selection (Auto / Local / Kroki / Self-hosted)
- Kroki URL input
- Theme dropdown
- Max dimensions
- Cache management (size display, clear button)

**Custom renderers section:**
- List of user-defined renderers with Add/Edit/Remove
- Format ID, display name, detection patterns, render command, render type
- Import/export as YAML

### Part E: Sidebar Registration

#### Modify: `src/settings_ui/sidebar.rs`

Register the new "Content Prettifier" tab with icon and search keywords:

```rust
// Add to tab list:
Tab::ContentPrettifier => {
    icon: "🎨",
    label: "Content Prettifier",
}

// Add search keywords:
fn tab_search_keywords(tab: &Tab) -> &[&str] {
    match tab {
        Tab::ContentPrettifier => &[
            "prettifier", "prettify", "pretty", "content", "detect", "detection",
            "render", "markdown", "json", "yaml", "toml", "xml", "csv", "diff",
            "diagram", "mermaid", "log", "stack trace", "confidence", "gutter",
            "badge", "toggle", "source", "rendered", "trigger", "custom",
        ],
        // ...
    }
}
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/settings_ui/prettifier_tab.rs` |
| Modify | `src/settings_ui/mod.rs` (register prettifier tab) |
| Modify | `src/settings_ui/sidebar.rs` (add tab + search keywords) |
| Modify | `src/prettifier/registry.rs` (custom renderer registration) |
| Modify | `src/prettifier/renderers/diagrams.rs` (custom language registration) |
| Modify | Settings > Automation tab (Prettify action type in trigger editor) |

## Relevant Spec Sections

- **Lines 627–728**: User-extensible regex rules — adding, overriding, disabling, creating new detectors
- **Lines 1223–1285**: User-defined custom renderers — external command, fenced block languages
- **Lines 1370–1428**: Full Settings UI specification
- **Lines 1375–1398**: Top-level controls, per-renderer cards, profile override section
- **Lines 1399–1412**: Diagram settings, Claude Code settings, custom renderers section
- **Lines 1416–1423**: Detection rules section with test feature
- **Lines 1424–1427**: Trigger integration in Automation tab
- **Lines 1343–1344**: Phase 3 & 4 — user extensibility, custom renderers, Settings UI
- **Lines 1477–1487**: Acceptance criteria for extensibility

## Verification Criteria

### Custom Renderers
- [ ] User-defined renderers from config are registered at startup
- [ ] External command renderers pipe content to command and capture output
- [ ] Custom regex-only detectors work without any Rust code
- [ ] Custom fenced block diagram languages are registered
- [ ] Custom diagram languages render via Kroki when configured

### Settings UI
- [ ] "Content Prettifier" tab appears in Settings with 🎨 icon
- [ ] Master toggle displays with dynamic subtitle listing enabled formats
- [ ] Scope badge shows `[Global]` or `[Profile: name]` correctly
- [ ] "Reset to global" link appears when profile overrides are active
- [ ] Detection scope dropdown works (Command Output / All / Manual Only)
- [ ] Confidence threshold slider adjusts detection sensitivity
- [ ] Per-renderer cards show enable/disable, priority, and renderer-specific settings
- [ ] Profile editor has tri-state prettifier toggle (On / Off / Inherit)
- [ ] Profile "Prettifier Overrides" panel shows only changed settings
- [ ] "Clear all overrides" resets profile to fully inheriting global
- [ ] Visual diff indicators appear next to profile-overridden settings
- [ ] Detection rules table shows all rules (built-in + user) with enable/disable
- [ ] Built-in rules can be disabled but not deleted
- [ ] User-defined rules can be edited and removed
- [ ] "Add rule" creates new user-defined rules
- [ ] "Test rules" evaluates sample content against rules with confidence meter
- [ ] Trigger editor includes Prettify action with format/scope/block-end fields
- [ ] Diagram settings show backend selector, Kroki URL, theme, cache management
- [ ] Custom renderers section supports add/edit/remove
- [ ] All settings changes persist to config file
- [ ] Search keywords work for finding prettifier settings
