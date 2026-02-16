# Step 13: Claude Code Integration

## Summary

Implement Claude Code-specific integration for the Content Prettifier: detecting Claude Code sessions, respecting `Ctrl+O` expand/collapse interactions, rendering format badges on collapsed blocks, and handling multi-format responses (Markdown with embedded JSON, diffs, and diagrams).

## Dependencies

- **Step 4**: `PrettifierPipeline` (process_output, active_blocks)
- **Step 6**: `ClaudeCodeConfig` configuration
- **Step 8–9**: Markdown renderer
- **Step 12**: Diagram renderer (for embedded diagram blocks)

## What to Implement

### New File: `src/prettifier/claude_code.rs`

```rust
/// Manages Claude Code integration for the prettifier system.
pub struct ClaudeCodeIntegration {
    config: ClaudeCodeConfig,
    /// Whether we've detected a Claude Code session
    is_claude_code_session: bool,
    /// Tracks expanded/collapsed state of Claude Code output blocks
    expand_states: HashMap<u64, ExpandState>,
}

pub enum ExpandState {
    /// Content is collapsed (showing "(ctrl+o to expand)")
    Collapsed {
        /// Preview content (first header + format badge)
        preview: Option<RenderedPreview>,
    },
    /// Content is expanded (full content visible)
    Expanded {
        /// Whether prettifier has processed this expanded content
        prettified: bool,
    },
}

pub struct RenderedPreview {
    pub format_badge: String,
    pub first_header: Option<String>,
    pub content_summary: String,
}

impl ClaudeCodeIntegration {
    pub fn new(config: ClaudeCodeConfig) -> Self { ... }

    /// Detect if this is a Claude Code session.
    /// Checks: process name, CLAUDE_CODE env var, characteristic output patterns.
    pub fn detect_session(&mut self, env_vars: &HashMap<String, String>, process_name: &str) -> bool { ... }

    /// Handle terminal output in the context of Claude Code.
    /// Returns true if the line was a Claude Code control pattern (expand/collapse).
    pub fn process_line(&mut self, line: &str, row: usize) -> Option<ClaudeCodeEvent> { ... }

    /// Check if a row is in a collapsed Claude Code block.
    pub fn is_collapsed(&self, row: usize) -> bool { ... }

    /// Get preview content for a collapsed block.
    pub fn get_preview(&self, block_id: u64) -> Option<&RenderedPreview> { ... }
}

pub enum ClaudeCodeEvent {
    /// User pressed Ctrl+O to expand — trigger prettifier on newly visible content
    ContentExpanded {
        row_range: Range<usize>,
    },
    /// Content collapsed — show preview with format badge
    ContentCollapsed {
        row_range: Range<usize>,
    },
    /// Detected Claude Code format indicator
    FormatDetected {
        format: String,
    },
}
```

### Claude Code Session Detection

From spec lines 1199–1200:

```rust
impl ClaudeCodeIntegration {
    fn detect_session(&mut self, env_vars: &HashMap<String, String>, process_name: &str) -> bool {
        // 1. Check CLAUDE_CODE environment variable
        if env_vars.contains_key("CLAUDE_CODE") {
            self.is_claude_code_session = true;
            return true;
        }

        // 2. Check process name
        if process_name.contains("claude") || process_name.contains("claude-code") {
            self.is_claude_code_session = true;
            return true;
        }

        false
    }
}
```

### Ctrl+O Expand/Collapse Awareness

From spec lines 1200–1208:

When expanded content is revealed:
1. Run the full detection pipeline on newly visible content
2. Apply appropriate prettifiers (Markdown body, JSON in tool results, diffs in file changes)
3. Maintain rendered state across collapse/expand cycles

When content is collapsed (`(ctrl+o to expand)` pattern):
1. Display a compact rendered preview (first header + content type badge)
2. Show format indicator: `📝 Markdown` / `{} JSON` / `📊 Diagram`

```rust
/// Detect the Claude Code expand/collapse pattern in terminal output.
fn detect_ctrl_o_pattern(line: &str) -> Option<CtrlOPattern> {
    // Claude Code shows "(ctrl+o to expand)" for collapsed content
    if line.contains("(ctrl+o to expand)") || line.contains("ctrl+o") {
        return Some(CtrlOPattern::CollapseMarker);
    }
    None
}
```

### Multi-Format Handling

A single Claude Code response may contain:
- Markdown prose
- Embedded JSON in code blocks
- Embedded diffs
- Embedded Mermaid diagrams

The prettifier should handle nested formats correctly:
1. Detect the outer format as Markdown
2. Within the Markdown renderer, recognize fenced code blocks
3. For `json` fenced blocks: apply JSON syntax highlighting
4. For `diff` fenced blocks: apply diff coloring
5. For `mermaid` fenced blocks: invoke the diagram renderer (Step 12)

```rust
impl MarkdownRenderer {
    /// Check if a fenced code block should be sub-rendered by another renderer.
    fn should_sub_render(&self, language: &str, registry: &RendererRegistry) -> bool {
        // Diagram languages are sub-rendered
        // JSON, YAML, diff may also be sub-rendered in the future
        registry.get_renderer(language).is_some()
    }
}
```

### Format Badges for Collapsed Blocks

When Claude Code content is collapsed, show format type badges:

```rust
impl ClaudeCodeIntegration {
    /// Generate a preview line for a collapsed block.
    fn generate_preview(&self, content: &ContentBlock, detection: &DetectionResult) -> RenderedPreview {
        let badge = match detection.format_id.as_str() {
            "markdown" => "📝 Markdown",
            "json" => "{} JSON",
            "diagrams" => "📊 Diagram",
            "yaml" => "📋 YAML",
            "diff" => "± Diff",
            _ => &detection.format_id,
        };

        // Extract first header from content (if markdown)
        let first_header = content.lines.iter()
            .find(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string());

        RenderedPreview {
            format_badge: badge.to_string(),
            first_header,
            content_summary: format!("{} lines", content.lines.len()),
        }
    }
}
```

### Integration with Pipeline

Modify `PrettifierPipeline` to incorporate Claude Code awareness:

```rust
impl PrettifierPipeline {
    /// Called when Claude Code integration detects a content expansion.
    pub fn on_claude_code_expand(&mut self, row_range: Range<usize>) {
        if self.claude_code.config.auto_render_on_expand {
            // Re-process the expanded content through the detection pipeline
            let block = self.extract_content_block(row_range);
            if let Some(detection) = self.registry.detect(&block) {
                self.render_block(block, detection);
            }
        }
    }
}
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/claude_code.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod claude_code;`) |
| Modify | `src/prettifier/pipeline.rs` (add Claude Code integration) |
| Modify | `src/prettifier/renderers/markdown.rs` (sub-rendering for nested formats) |

## Relevant Spec Sections

- **Lines 1194–1219**: Full Claude Code `Ctrl+O` integration specification
- **Lines 1199–1200**: Detection approach — process name, env var, output patterns
- **Lines 1200–1208**: Ctrl+O aware rendering — expand triggers pipeline, collapse shows preview
- **Lines 1209–1219**: Claude Code config YAML
- **Lines 1338**: Phase 1d — process detection, Ctrl+O awareness, format badges
- **Lines 1467**: Acceptance criteria — Ctrl+O expand triggers prettifier pipeline

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] Claude Code session detection works via `CLAUDE_CODE` env var
- [ ] Claude Code session detection works via process name
- [ ] `Ctrl+O` expand pattern is detected in terminal output
- [ ] Expanded content triggers the prettifier detection pipeline
- [ ] Collapsed blocks show format badge previews
- [ ] Rendered state is maintained across collapse/expand cycles
- [ ] Multi-format responses handle nested code blocks correctly
- [ ] `auto_render_on_expand` config is respected
- [ ] `show_format_badges` config is respected
- [ ] Integration with pipeline correctly processes expanded content
- [ ] Non-Claude Code sessions are unaffected
- [ ] Unit tests for session detection, expand/collapse patterns, preview generation
