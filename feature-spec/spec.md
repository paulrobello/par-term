# Feature Request: Extensible Content Prettifier System with Markdown & Mermaid as First-Class Renderers

## Summary

Add an extensible **Content Prettifier** framework to par-term that automatically detects structured content in terminal output and renders it in a rich, human-readable form. The system should be built as a **pluggable renderer architecture** where Markdown is the first renderer implemented, but the design makes it trivial to add prettifiers for JSON, YAML, TOML, XML, CSV, LaTeX/math, log files, diff/patch output, and diagram-as-code languages (Mermaid, PlantUML, GraphViz, D2, etc.) in the future.

The system should integrate with Claude Code's `Ctrl+O` expand/collapse workflow and leverage par-term's existing inline graphics pipeline (Sixel, iTerm2, Kitty) for visual renderers.

---

## Prior Art Check

**This feature does NOT currently exist in par-term.** After reviewing:

- The full README and feature list through v0.16.0
- The existing rendering pipeline (GPU text rasterization, inline graphics via Sixel/iTerm2/Kitty)
- The trigger/action system, shell integration (OSC 133), and progress bar rendering (OSC 9;4)
- All documented keyboard shortcuts, settings, and configuration options
- The existing `ideas.md` file listing

**No content auto-detection, markdown rendering, structured data prettifying, or diagram rendering exists.** The closest related features are:
- Regex-based URL detection and OSC 8 hyperlinks (pattern matching in terminal output)
- Inline graphics protocols (the rendering target for diagram renderers)
- Shell integration OSC markers (potential hook points for content boundary detection)
- Command separator lines (visual demarcation between output blocks)
- The trigger/action system (regex-based output processing — the natural extension point)

**Related ecosystem context:**
- Warp terminal has an open issue for Mermaid rendering in their markdown previewer (#7115)
- Claude Code has an open feature request for improved terminal markdown rendering (#13600) and known rendering bugs (#14755)
- External tools exist for individual formats (`glow`/`mdcat` for Markdown, `jq` for JSON, `yq` for YAML, `bat` for syntax highlighting) but no terminal provides unified built-in prettifying
- kroki.io provides a unified API for 25+ diagram languages — an ideal backend for the diagram renderer

---

## Problem Statement

Terminal emulators display all output as flat character grids, regardless of whether the content is structured data (JSON, YAML, XML), rich documents (Markdown), diagrams (Mermaid, PlantUML), or plain text. This is a growing problem as AI coding tools like Claude Code generate increasingly structured output that would benefit from rich rendering.

Today's workarounds require users to pipe output through external tools (`jq`, `glow`, `bat`) — a manual step that breaks flow. A terminal that understands content types and renders them appropriately would be a category-defining feature.

**Key opportunities:**
1. **Markdown readability**: Render proper visual hierarchy instead of raw `#`, `**`, `` ` `` syntax
2. **Structured data clarity**: Syntax-highlight and tree-fold JSON/YAML/TOML/XML instead of displaying walls of text
3. **Diagrams as images**: Render Mermaid/PlantUML/GraphViz definitions as actual inline graphics
4. **Log file readability**: Color-code log levels, timestamps, and highlight errors
5. **Diff visualization**: Render unified diffs with proper +/- coloring and side-by-side views

---

## Architecture: The Content Prettifier Framework

### Core Design Principles

1. **Trait-based plugin system**: Each prettifier implements a common `ContentRenderer` trait, making it trivial to add new formats
2. **Detection is separate from rendering**: A `ContentDetector` identifies the format; a `ContentRenderer` handles display — they are independently extensible
3. **Regex-driven detection as the common substrate**: All built-in detectors are powered by configurable regex rule sets under the hood. This means every detector can be inspected, tuned, extended, or overridden by users via config — no Rust code required to add detection for a new format. A `RegexDetector` is the standard implementation of the `ContentDetector` trait, and most formats simply declare their regex rules rather than implementing custom detection logic
4. **Source is always preserved**: The raw source text is never discarded — users can always toggle back to source view
5. **Renderers declare their capabilities**: Each renderer advertises what it needs (text-only styling? inline graphics? external tools?) so the framework can gracefully degrade
6. **Lazy and async**: Detection runs at content boundaries (not per-byte), rendering is async and cached, expensive renderers (diagrams) show placeholders while working
7. **User-extensible at every layer**: Users can (a) add regex rules to existing detectors, (b) create entirely new detectors from regex patterns alone via config, (c) register custom renderers that map to external commands, (d) hook into par-term's existing trigger system to invoke prettifiers on specific patterns
8. **Trigger system integration**: The prettifier registers as a new action type in par-term's existing trigger/action system (v0.11.0), so users can also invoke prettifiers manually via trigger rules alongside the auto-detection path

### Trait Definitions (Conceptual Rust)

```rust
/// Identifies whether a content block matches a specific format
trait ContentDetector: Send + Sync {
    /// Unique identifier for this format (e.g., "markdown", "json", "mermaid")
    fn format_id(&self) -> &str;
    
    /// Human-readable name for settings UI
    fn display_name(&self) -> &str;
    
    /// Analyze a content block and return a confidence score (0.0 - 1.0)
    /// Returns None if this detector cannot handle the content at all
    fn detect(&self, content: &ContentBlock) -> Option<DetectionResult>;
    
    /// Quick check — can this detector potentially match this content?
    /// Used for fast filtering before running full detection
    fn quick_match(&self, first_lines: &[&str]) -> bool;
    
    /// Return the regex rules powering this detector (for UI inspection/editing)
    /// Built-in detectors expose their rules; custom detectors always expose theirs
    fn detection_rules(&self) -> &[DetectionRule];
    
    /// Whether this detector allows user-added regex rules via config
    fn accepts_custom_rules(&self) -> bool { true }
}

// ──────────────────────────────────────────────────────────────────
// Regex-Driven Detection: The standard ContentDetector implementation
// ──────────────────────────────────────────────────────────────────

/// A single regex rule contributing to format detection
struct DetectionRule {
    /// Unique ID for this rule (for enable/disable and override)
    id: String,
    /// The regex pattern (compiled at startup)
    pattern: Regex,
    /// How much confidence this rule contributes when matched (0.0 - 1.0)
    weight: f32,
    /// Where in the content to apply this pattern
    scope: RuleScope,
    /// Whether this rule is a strong signal (can trigger detection alone)
    /// or a supporting signal (needs other rules to also match)
    strength: RuleStrength,
    /// Is this rule built-in or user-added?
    source: RuleSource,
    /// Optional: rule only applies after a specific command (e.g., "curl", "cat *.json")
    command_context: Option<Regex>,
    /// Human-readable description (shown in Settings UI)
    description: String,
}

enum RuleScope {
    /// Match against any line in the content block
    AnyLine,
    /// Match only the first N lines (fast path for format headers)
    FirstLines(usize),
    /// Match only the last N lines (footers, closing brackets)
    LastLines(usize),
    /// Match against the entire content block as a single string (multi-line regex)
    FullBlock,
    /// Match against the preceding command that generated this output
    PrecedingCommand,
}

enum RuleStrength {
    /// A definitive signal — this pattern alone is sufficient to identify the format
    /// (e.g., `^```mermaid` for Mermaid, `<?xml` for XML, `^diff --git` for diff)
    Definitive,
    /// A strong signal — high confidence when matched, but benefits from corroboration
    /// (e.g., `^#{1,6}\s` for Markdown headers, `^\{` for JSON)
    Strong,
    /// A supporting signal — only contributes when combined with other matches
    /// (e.g., `\*\*bold\*\*` for Markdown, indentation patterns for YAML)
    Supporting,
}

enum RuleSource {
    /// Shipped with par-term, can be disabled but not deleted
    BuiltIn,
    /// Added by the user via config, can be edited or removed
    UserDefined,
}

/// The standard regex-based ContentDetector implementation
/// Most formats use this rather than implementing ContentDetector from scratch
struct RegexDetector {
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

impl ContentDetector for RegexDetector {
    fn detect(&self, content: &ContentBlock) -> Option<DetectionResult> {
        let mut total_confidence = 0.0;
        let mut match_count = 0;
        
        for rule in &self.rules {
            let text = match rule.scope {
                RuleScope::FirstLines(n) => first_n_lines(content, n),
                RuleScope::PrecedingCommand => content.preceding_command.as_deref()?,
                // ... other scopes
            };
            
            if rule.pattern.is_match(text) {
                total_confidence += rule.weight;
                match_count += 1;
                
                // A Definitive rule can short-circuit detection
                if rule.strength == RuleStrength::Definitive 
                   && self.definitive_rule_shortcircuit {
                    return Some(DetectionResult {
                        format_id: self.format_id.clone(),
                        confidence: 1.0,
                        // ...
                    });
                }
            }
        }
        
        if match_count >= self.min_matching_rules 
           && total_confidence >= self.confidence_threshold {
            Some(DetectionResult {
                format_id: self.format_id.clone(),
                confidence: total_confidence.min(1.0),
                // ...
            })
        } else {
            None
        }
    }
    
    fn detection_rules(&self) -> &[DetectionRule] { &self.rules }
}
```

### Built-In Regex Rule Sets

Every built-in detector ships with a default set of regex rules that users can inspect, disable, or augment. Here are the initial rule sets:

```yaml
# ──────────────────────────────────────────────────────────────────
# Built-in detection rules (loaded at startup, user-overridable)
# These live in the par-term binary but are exposed in Settings UI
# and can be disabled or supplemented via config.yaml
# ──────────────────────────────────────────────────────────────────

content_prettifier:
  detection_rules:

    markdown:
      # Definitive rules — any one of these is enough
      - id: md_fenced_code
        pattern: '^```\w*\s*$'
        weight: 0.8
        scope: any_line
        strength: definitive
        description: "Fenced code block opening (``` or ```language)"
      - id: md_fenced_tilde
        pattern: '^~~~\w*\s*$'
        weight: 0.8
        scope: any_line
        strength: definitive
        description: "Tilde-style fenced code block"
      # Strong rules
      - id: md_atx_header
        pattern: '^#{1,6}\s+\S'
        weight: 0.5
        scope: any_line
        strength: strong
        description: "ATX-style header (# through ######)"
      - id: md_table
        pattern: '^\|.*\|.*\|'
        weight: 0.4
        scope: any_line
        strength: strong
        description: "Markdown table row with pipe delimiters"
      - id: md_table_separator
        pattern: '^\|[\s\-:|]+\|'
        weight: 0.3
        scope: any_line
        strength: supporting
        description: "Markdown table separator row"
      # Supporting rules
      - id: md_bold
        pattern: '\*\*[^*]+\*\*'
        weight: 0.2
        scope: any_line
        strength: supporting
        description: "Bold emphasis markers"
      - id: md_italic
        pattern: '(?<!\*)\*[^*]+\*(?!\*)'
        weight: 0.15
        scope: any_line
        strength: supporting
        description: "Italic emphasis markers"
      - id: md_link
        pattern: '\[([^\]]+)\]\(([^)]+)\)'
        weight: 0.2
        scope: any_line
        strength: supporting
        description: "Inline link [text](url)"
      - id: md_list_bullet
        pattern: '^\s*[-*+]\s+\S'
        weight: 0.15
        scope: any_line
        strength: supporting
        description: "Unordered list item"
      - id: md_list_ordered
        pattern: '^\s*\d+[.)]\s+\S'
        weight: 0.15
        scope: any_line
        strength: supporting
        description: "Ordered list item"
      - id: md_blockquote
        pattern: '^>\s+'
        weight: 0.15
        scope: any_line
        strength: supporting
        description: "Blockquote line"
      - id: md_inline_code
        pattern: '`[^`]+`'
        weight: 0.1
        scope: any_line
        strength: supporting
        description: "Inline code span"
      - id: md_horizontal_rule
        pattern: '^([-*_])\s*\1\s*\1[\s\1]*$'
        weight: 0.15
        scope: any_line
        strength: supporting
        description: "Horizontal rule (---, ***, ___)"
      # Command context — boost confidence when following AI tools
      - id: md_claude_code_context
        pattern: '(claude|cc|claude-code)'
        weight: 0.2
        scope: preceding_command
        strength: supporting
        description: "Output follows a Claude Code command"

    json:
      - id: json_open_brace
        pattern: '^\s*\{\s*$'
        weight: 0.4
        scope: first_lines:3
        strength: strong
        description: "Opening brace on its own line"
      - id: json_open_bracket
        pattern: '^\s*\[\s*$'
        weight: 0.35
        scope: first_lines:3
        strength: strong
        description: "Opening bracket on its own line"
      - id: json_key_value
        pattern: '^\s*"[^"]+"\s*:\s*'
        weight: 0.3
        scope: any_line
        strength: strong
        description: "JSON key-value pair with quoted key"
      - id: json_close_brace
        pattern: '^\s*\}\s*,?\s*$'
        weight: 0.2
        scope: last_lines:3
        strength: supporting
        description: "Closing brace"
      - id: json_curl_context
        pattern: '^(curl|http|httpie|wget)\s+'
        weight: 0.3
        scope: preceding_command
        strength: supporting
        description: "Output follows an HTTP client command"
      - id: json_jq_context
        pattern: '^(jq|gron|fx)\s+'
        weight: 0.3
        scope: preceding_command
        strength: supporting
        description: "Output follows a JSON processor command"

    yaml:
      - id: yaml_doc_start
        pattern: '^---\s*$'
        weight: 0.5
        scope: first_lines:3
        strength: definitive
        description: "YAML document start marker"
      - id: yaml_key_value
        pattern: '^[a-zA-Z_][a-zA-Z0-9_]*:\s+'
        weight: 0.3
        scope: any_line
        strength: strong
        description: "Top-level key-value pair"
      - id: yaml_nested
        pattern: '^\s{2,}[a-zA-Z_][a-zA-Z0-9_]*:\s+'
        weight: 0.2
        scope: any_line
        strength: supporting
        description: "Indented nested key-value pair"
      - id: yaml_list
        pattern: '^\s*-\s+[a-zA-Z_]'
        weight: 0.15
        scope: any_line
        strength: supporting
        description: "YAML list item"

    diff:
      - id: diff_git_header
        pattern: '^diff --git\s+'
        weight: 0.9
        scope: first_lines:5
        strength: definitive
        description: "Git diff header"
      - id: diff_unified_header
        pattern: '^---\s+\S+.*\n\+\+\+\s+\S+'
        weight: 0.9
        scope: first_lines:10
        strength: definitive
        description: "Unified diff file headers (--- / +++)"
      - id: diff_hunk
        pattern: '^@@\s+-\d+,?\d*\s+\+\d+,?\d*\s+@@'
        weight: 0.8
        scope: any_line
        strength: definitive
        description: "Diff hunk header (@@ -n,m +n,m @@)"
      - id: diff_add_line
        pattern: '^\+[^+]'
        weight: 0.1
        scope: any_line
        strength: supporting
        description: "Added line (starts with single +)"
      - id: diff_remove_line
        pattern: '^-[^-]'
        weight: 0.1
        scope: any_line
        strength: supporting
        description: "Removed line (starts with single -)"
      - id: diff_git_context
        pattern: '^git\s+(diff|log|show)'
        weight: 0.3
        scope: preceding_command
        strength: supporting
        description: "Output follows a git diff/log/show command"

    # Additional built-in rule sets: toml, xml, csv, log, sql_results,
    # stack_trace follow the same pattern...
```

### Trigger System Integration

The prettifier integrates with par-term's existing trigger/action system (v0.11.0) as a **new action type**: `Prettify`. This allows users to create trigger rules that invoke the prettifier on demand for patterns that the auto-detection pipeline might miss or for highly specific use cases.

```
┌─────────────────────────────────────────────────────────────────┐
│                    par-term Trigger System                       │
│                                                                 │
│  Existing action types (v0.11.0):                               │
│    1. Highlight Line     5. Send Text                           │
│    2. Highlight Text     6. Run Command                         │
│    3. Post Notification  7. Run Coprocess                       │
│    4. Set Mark                                                  │
│                                                                 │
│  NEW action type:                                               │
│    8. Prettify — invoke a specific renderer on matched content  │
│                                                                 │
│  The Prettify action bridges the trigger system and the         │
│  content prettifier framework, enabling regex-triggered         │
│  rendering without going through the auto-detection pipeline.   │
└─────────────────────────────────────────────────────────────────┘
```

**How trigger-based prettifying works:**

A user creates a trigger (in Settings > Automation or via config) with:
- A **regex pattern** that matches terminal output lines
- The **Prettify action** specifying which renderer to invoke
- Optional **scope** — apply to the matched line, the matched block, or the surrounding command output

This bypasses the confidence-scoring auto-detection pipeline entirely. When the trigger regex matches, the specified renderer is invoked directly with confidence 1.0.

**Trigger → Prettify configuration:**

```yaml
# In par-term's existing triggers configuration
triggers:
  # Example: Force JSON prettifying for output of a custom API tool
  - name: "Prettify myapi output"
    regex: '^\{"api_version":'
    action: prettify
    prettify_format: "json"            # Which renderer to invoke
    prettify_scope: "command_output"   # "line" | "block" | "command_output"
    enabled: true

  # Example: Detect and prettify custom log format
  - name: "Prettify app logs"
    regex: '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z\s+(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)'
    action: prettify
    prettify_format: "log"
    prettify_scope: "command_output"
    enabled: true

  # Example: Force Markdown rendering for a specific tool's output
  - name: "Prettify llm-cli output"
    regex: '.'                         # Match any output...
    command_filter: '^llm\s+'          # ...but only from the `llm` CLI
    action: prettify
    prettify_format: "markdown"
    prettify_scope: "command_output"
    enabled: true

  # Example: Render PlantUML from a custom code fence your team uses
  - name: "Prettify @startuml blocks"
    regex: '^@startuml'
    action: prettify
    prettify_format: "diagrams"
    prettify_sub_format: "plantuml"
    prettify_scope: "block"            # Scope to the @startuml...@enduml block
    prettify_block_end: '^@enduml'     # Regex for block terminator
    enabled: true

  # Example: Render any output from `bat` as-is (disable prettifier to avoid double-rendering)
  - name: "Skip prettifier for bat"
    regex: '.'
    command_filter: '^bat\s+'
    action: prettify
    prettify_format: "none"            # Special value: suppress auto-detection
    prettify_scope: "command_output"
    enabled: true
```

**Key integration points:**

- **Trigger → Prettify** is a one-way bridge: triggers can invoke prettifiers, but the auto-detection pipeline does NOT create triggers. They are independent entry points to the same renderer registry
- **`prettify_format: "none"`** is a special value that suppresses auto-detection for matched content — useful for tools that already produce styled output (e.g., `bat`, `delta`, `glow`) where prettifying would double-render
- **`command_filter`** uses the existing trigger system's command context awareness (from shell integration) to scope triggers to output from specific commands
- **`prettify_scope: "block"`** + **`prettify_block_end`** enables block-scoped rendering for formats with explicit start/end markers (like `@startuml`/`@enduml`) that don't use standard fenced code block syntax
- Trigger-based prettifying respects the same `enable_prettifier` master toggle and profile overrides — if prettifying is disabled, trigger actions are also suppressed
- Triggers appear in the Settings UI alongside existing triggers, with a "Prettify" action type in the dropdown and a format selector

---

## Configuration: `enable_prettifier` — Global & Profile Toggle

> **Setting name**: `enable_prettifier`
> **Default**: `true` (enabled out of the box)
> **Scope**: Global config (`config.yaml`) + per-profile override (`profiles.yaml`)
> **Override rule**: Profile setting wins. If the active profile specifies `enable_prettifier`, that value is used. If the profile omits it, the global value is inherited.

This is the single master switch for the entire Content Prettifier system. It is intentionally named **`enable_prettifier`** — not `enable_markdown` or `enable_content_detection` — because it controls a general-purpose framework that starts with Markdown but will grow to cover many structured formats over time.

**In the Settings UI**, this appears as:

> **Enable Prettifier** · `toggle: ON`
> *Automatically detects and renders structured content in terminal output including Markdown, diagrams (Mermaid, PlantUML, GraphViz), JSON, YAML, and more. Additional format support is added regularly.*

The subtitle text should be **dynamically generated** from the renderer registry so it always reflects the currently available renderers without manual updates.

**In the Profile editor**, this appears as a **tri-state toggle**:
- **On** — Prettifier is enabled for this profile regardless of global setting
- **Off** — Prettifier is disabled for this profile regardless of global setting
- **Inherit** *(default)* — Use whatever the global config says

A small **scope badge** next to the toggle in the Settings UI indicates the source of the current value: `[Global]` or `[Profile: Claude Code]`, with a "Reset to global" link when profile-overridden.

### Full Override Chain

All settings under `content_prettifier:` follow the same precedence:

```
1. Profile-level setting (if the active profile specifies it)    ← wins
2. Global config-level setting (config.yaml)                     ← fallback
3. Built-in default                                              ← last resort
```

This applies to every sub-setting — `enable_prettifier`, `detection.confidence_threshold`, `renderers.json.enabled`, `renderers.diagrams.backend`, `claude_code_integration.enabled`, etc. Profiles can selectively override just the settings they care about without repeating the entire configuration block.

**Example scenarios:**

| Global | Profile | Effective | Why |
|---|---|---|---|
| `enable_prettifier: true` | *(omitted)* | **enabled** | Profile inherits global |
| `enable_prettifier: true` | `enable_prettifier: false` | **disabled** | Profile override wins |
| `enable_prettifier: false` | `enable_prettifier: true` | **enabled** | Profile override wins |
| `renderers.json.enabled: false` | `renderers.json.enabled: true` | **json enabled** | Profile enables JSON for this context |
| `renderers.diagrams.backend: "kroki"` | `renderers.diagrams.backend: "local"` | **local** | Air-gapped profile uses local tools |

### Runtime Behavior

- **Profile switching** (automatic or manual) immediately applies the new profile's prettifier settings
- Already-rendered blocks in scrollback are **not** re-processed on profile switch — only new output uses the new settings
- The `Cmd+Shift+M` / `Ctrl+Shift+M` keybinding acts as a **session-level** toggle that overrides both global and profile settings until the session ends or the key is pressed again. It does not persist to config.

---

### Detection Pipeline (Updated with Regex & Trigger Integration)

```
Terminal Output Stream
        │
        ├──────────────────────────────────────────────┐
        │                                              │
        ▼                                              ▼
┌─────────────────────────┐               ┌──────────────────────────┐
│   Trigger Engine         │               │   Content Boundary        │
│   (existing v0.11.0)     │               │   Detector                │
│                          │               │                           │
│   Regex match per-line   │               │   OSC 133 markers,        │
│   against trigger rules  │               │   alt-screen transitions, │
│                          │               │   blank line heuristics   │
│   If action = "prettify" │               └──────────┬────────────────┘
│   → direct dispatch to   │                          │ ContentBlock
│     renderer registry    │                          ▼
│   (bypasses confidence   │               ┌──────────────────────────┐
│    scoring entirely)     │               │   Regex-Based Format      │
│                          │               │   Detection Pipeline      │
└──────────┬───────────────┘               │                           │
           │                               │   For each registered     │
           │ PrettifyAction                │   RegexDetector:          │
           │ (format_id,                   │                           │
           │  confidence: 1.0)             │   1. quick_match() on     │
           │                               │      first lines          │
           │                               │   2. Run regex rules by   │
           │                               │      scope & strength     │
           │                               │   3. Sum weighted scores  │
           │                               │   4. Apply Definitive     │
           │                               │      short-circuit        │
           │                               │   5. Check threshold      │
           │                               │                           │
           │                               │   Built-in rule sets +    │
           │                               │   user-added rules merged │
           │                               └──────────┬────────────────┘
           │                                          │ DetectionResult
           │                                          │ (format_id, confidence)
           ▼                                          ▼
     ┌─────────────────────────────────────────────────────┐
     │                  Renderer Registry                   │
     │                                                     │
     │   Maps format_id → ContentRenderer                  │
     │   Checks renderer capabilities vs terminal          │
     │   Applies profile overrides                         │
     │   Falls back gracefully if requirements not met     │
     └──────────────────────┬──────────────────────────────┘
                            │
                            ▼
     ┌─────────────────────────────────────────────────────┐
     │              Render & Cache Manager                  │
     │                                                     │
     │   Async rendering, result caching,                  │
     │   placeholder display for slow renders              │
     │   Source ←→ Rendered dual view + line mapping        │
     └──────────────────────┬──────────────────────────────┘
                            │ RenderedContent
                            ▼
     ┌─────────────────────────────────────────────────────┐
     │   GPU Text Rasterizer + Inline Graphics             │
     │   (existing par-term rendering pipeline)            │
     └─────────────────────────────────────────────────────┘
```

### User-Extensible Regex Rules

Users can add custom regex rules to any built-in detector, or create entirely new detectors from regex alone, without writing any Rust code:

**Adding rules to a built-in detector:**

```yaml
content_prettifier:
  detection_rules:
    markdown:
      # User-added rules are merged with built-in rules at startup
      - id: my_custom_md_signal
        pattern: '^:::note'
        weight: 0.6
        scope: any_line
        strength: strong
        source: user_defined
        description: "Custom admonition syntax used by our docs"

    json:
      - id: my_api_envelope
        pattern: '^\{"status":\s*\d+,\s*"data":'
        weight: 0.7
        scope: first_lines:1
        strength: definitive
        source: user_defined
        description: "Our API always wraps responses in {status, data}"
```

**Creating a new detector entirely from regex (no Rust code):**

```yaml
content_prettifier:
  custom_renderers:
    - format_id: "graphql"
      display_name: "GraphQL"
      detection_rules:
        - id: gql_query
          pattern: '^\s*(query|mutation|subscription)\s+\w+'
          weight: 0.7
          scope: first_lines:5
          strength: definitive
          description: "GraphQL operation keyword"
        - id: gql_type
          pattern: '^\s*type\s+\w+\s*\{'
          weight: 0.5
          scope: any_line
          strength: strong
          description: "GraphQL type definition"
        - id: gql_field
          pattern: '^\s+\w+(\(.*\))?\s*:\s*\[?\w+\]?!?'
          weight: 0.2
          scope: any_line
          strength: supporting
          description: "GraphQL field with type annotation"
      min_matching_rules: 2
      render_command: "prettier --parser graphql --color"
      render_type: "text"
      cache: true

    - format_id: "nginx_conf"
      display_name: "Nginx Config"
      detection_rules:
        - id: nginx_server_block
          pattern: '^\s*server\s*\{'
          weight: 0.8
          scope: any_line
          strength: definitive
          description: "Nginx server block"
        - id: nginx_location
          pattern: '^\s*location\s+[/~]'
          weight: 0.5
          scope: any_line
          strength: strong
          description: "Nginx location directive"
        - id: nginx_directive
          pattern: '^\s*(listen|server_name|root|index|proxy_pass)\s+'
          weight: 0.3
          scope: any_line
          strength: supporting
          description: "Common Nginx directives"
      render_command: "bat --language nginx --color=always --style=plain"
      render_type: "text"
```

**Disabling or overriding built-in rules:**

```yaml
content_prettifier:
  detection_rules:
    markdown:
      overrides:
        # Disable a noisy built-in rule
        - id: md_bold
          enabled: false
        # Increase weight of a rule that matters for your workflow
        - id: md_fenced_code
          weight: 1.0
        # Change the scope of a rule
        - id: md_atx_header
          scope: first_lines:10     # Only check first 10 lines for headers
```

### Configuration: Global vs Profile Override

The prettifier follows the same override pattern used by par-term's existing profile system (badges, directory patterns, shell selection, etc.):

**Resolution order** (first non-null wins):
1. Profile-level setting (if the active profile specifies it)
2. Global config-level setting
3. Built-in default

This means:
- The **global config** (`config.yaml`) sets the baseline for all sessions
- Any **profile** can override any prettifier setting — enable it, disable it, change thresholds, enable/disable specific renderers
- If a profile doesn't specify a prettifier setting, the global value is inherited
- Profiles can selectively override just the settings they care about without repeating the entire block

**Example use cases:**
- Global: prettifier enabled. Your "Production SSH" profile overrides `enable_prettifier: false` because you want raw output on prod servers
- Global: JSON renderer disabled. Your "API Development" profile overrides `renderers.json.enabled: true` because you're constantly reading API responses
- Global: diagram backend set to "kroki". Your "Air-Gapped" profile overrides `renderers.diagrams.backend: "local"` because there's no network

### Global Configuration (config.yaml)

```yaml
# ──────────────────────────────────────────────────────────────────
# Content Prettifier
# Master toggle + detailed settings for par-term's content
# prettifier system. Automatically detects and renders structured
# content in terminal output — starting with Markdown and
# expanding to JSON, YAML, diagrams, diffs, and more.
#
# NAMING: The setting is called "enable_prettifier" (not
# "enable_markdown") because it's a general-purpose framework.
# Markdown is the first supported format, with many more to come.
#
# DEFAULT: true (enabled out of the box)
#
# OVERRIDE: This can be overridden per-profile in profiles.yaml.
# Profile settings take precedence over global settings.
# ──────────────────────────────────────────────────────────────────
enable_prettifier: true                # Master switch (default: true)
                                       # Set to false to disable all content
                                       # prettifying globally. Profiles can
                                       # override this per-profile.

content_prettifier:
  respect_alternate_screen: true       # Disable in alt screen (vim, etc.)
  global_toggle_key: "Cmd+Shift+M"    # Toggle all prettifying on/off
  per_block_toggle: true               # Click gutter indicator to toggle individual blocks
  
  # Detection settings
  detection:
    scope: "command_output"            # "command_output" | "all" | "manual_only"
    confidence_threshold: 0.6          # 0.0-1.0, higher = more conservative
    max_scan_lines: 500                # Don't scan blocks larger than this for detection
    debounce_ms: 100                   # Wait for output to settle before detecting

  # Copy behavior
  clipboard:
    default_copy: "rendered"           # "rendered" | "source"
    source_copy_modifier: "Shift"      # Hold Shift+Cmd+C to copy source
    vi_copy_mode: "source"             # Vi copy mode operates on source text
  
  # Per-renderer enable/disable and priority
  renderers:
    markdown:
      enabled: true
      priority: 100                    # Higher = checked first during detection
    json:
      enabled: true
      priority: 90
    yaml:
      enabled: true
      priority: 85
    toml:
      enabled: true
      priority: 80
    xml:
      enabled: true
      priority: 75
    csv:
      enabled: true
      priority: 70
    diff:
      enabled: true
      priority: 95                     # High priority — diffs are very recognizable
    log:
      enabled: true
      priority: 60
    diagrams:
      enabled: true
      priority: 110                    # Highest — diagram blocks inside markdown
    sql_results:
      enabled: true
      priority: 65
    stack_trace:
      enabled: true
      priority: 85

  # Custom/user-defined renderers
  custom_renderers: []
    # Example: register a custom renderer for a fenced code block language
    # - format_id: "asciimath"
    #   detect_pattern: "```asciimath"
    #   render_command: "katex-cli --format=sixel"
    #   capabilities: ["external_command"]
```

### Profile-Level Overrides (profiles.yaml)

Profiles can override any prettifier setting. **Profile values always win over global config.** Omitted fields inherit from the global config — profiles only need to specify the settings they want to change.

The `enable_prettifier` field in a profile is optional. When omitted, the global value is inherited. When present, it overrides the global value for sessions using that profile.

```yaml
profiles:
  - name: "Default"
    emoji: "🖥️"
    # enable_prettifier is NOT specified here — inherits global (true by default)
    # All content_prettifier sub-settings also inherit from global

  - name: "API Development"
    emoji: "🔌"
    enable_prettifier: true            # Explicit — ensure prettifier is on
    content_prettifier:
      renderers:
        json:
          enabled: true
          priority: 100                # Boost JSON to highest priority
          max_depth_expanded: 5        # Expand deeper for API debugging
        yaml:
          enabled: true
        markdown:
          enabled: false               # Less useful in API work, reduce noise

  - name: "Production SSH"
    emoji: "🔒"
    enable_prettifier: false           # Disable entirely — raw output on prod

  - name: "Claude Code"
    emoji: "🤖"
    enable_prettifier: true
    content_prettifier:
      renderers:
        markdown:
          enabled: true
          priority: 100
        diagrams:
          enabled: true
          backend: "kroki"             # Use Kroki for diagram rendering
      claude_code_integration:
        enabled: true
        auto_render_on_expand: true

  - name: "Air-Gapped Dev"
    emoji: "✈️"
    enable_prettifier: true
    content_prettifier:
      renderers:
        diagrams:
          enabled: true
          backend: "local"             # No network — local tools only
          kroki_url: null
```

### Setting Naming Rationale

The master toggle is deliberately named **`enable_prettifier`** — not `enable_markdown_rendering`, `enable_content_detection`, or any format-specific name. This is intentional:

1. **Future-proof**: The name doesn't need to change as new renderers are added. When JSON, YAML, diff, and diagram prettifying ship, the setting name still makes sense
2. **Discoverable**: Users searching settings for "prettify" or "pretty" will find it regardless of which specific format they're interested in
3. **Honest**: The setting controls a general-purpose framework, not just one format. Calling it `enable_markdown` would be misleading once other renderers exist
4. **Consistent with par-term conventions**: Par-term uses `enable_` prefix for boolean feature flags (similar to `custom_shader_enabled`, `reduce_flicker`, etc.)

The **Settings UI label** reads:

> **Enable Prettifier** · `toggle: ON`
> *Automatically detects and renders structured content in terminal output including Markdown, diagrams (Mermaid, PlantUML, GraphViz), JSON, YAML, and more. Additional format support is added regularly.*

This subtitle text should be **dynamically generated** from the renderer registry — listing the `display_name` of each enabled renderer — so it always reflects currently available renderers without manual documentation updates. When Markdown is the only shipped renderer (Phase 1), the subtitle would read:

> *Automatically detects and renders structured content in terminal output. Currently supports Markdown and diagrams (Mermaid, PlantUML, GraphViz). Additional format support is coming soon.*

---

## Phase 1 Renderers (Ship First)

### 1.1 Markdown Renderer

The flagship renderer — auto-detects and renders Markdown with full formatting.

**Detection heuristics (combine for confidence scoring):**
- Fenced code blocks: ```` ``` ```` or ```` ~~~ ```` patterns
- ATX headers: Lines starting with `# `, `## `, `### `, etc.
- Markdown tables: Lines with `|` delimiters and `---` separator rows
- Lists: Lines starting with `- `, `* `, `1. ` patterns
- Emphasis: `**bold**`, `_italic_`, `` `inline code` ``
- Links: `[text](url)` patterns
- Context signals: Output following known AI tool prompts (Claude Code, etc.)

**Rendered elements:**
- **Headers (H1-H6)**: Bold + color differentiation from the theme palette, with visual weight scaling
- **Code blocks**: Background shading, syntax highlighting via tree-sitter or ANSI color mapping, language label in gutter
- **Inline code**: Subtle background highlight
- **Tables**: Unicode box-drawing characters (par-term's geometric box drawing), proper column alignment and padding
- **Bold/Italic**: Leverage existing styled font variant support (Bold, Italic, Bold-Italic families)
- **Lists**: Proper indentation with styled bullets/numbers
- **Blockquotes**: Left border line with dimmed/colored text
- **Horizontal rules**: Rendered via the command separator line infrastructure
- **Links**: Converted to OSC 8 hyperlinks (already supported) with underline + link color
- **Images** (URL references): Fetch and render inline via graphics protocols

**Markdown-specific config:**
```yaml
content_prettifier:
  renderers:
    markdown:
      enabled: true
      priority: 100
      render_mode: "pretty"            # "pretty" | "source" | "hybrid"
      header_style: "colored"          # "colored" | "bold" | "underlined"
      code_block_theme: "monokai"      # Syntax highlighting theme name
      code_block_background: true
      table_style: "unicode"           # "unicode" | "ascii" | "rounded"
      table_border_color: "dim"
      horizontal_rule_style: "thin"    # "thin" | "thick" | "dashed"
      link_style: "underline_color"    # "underline_color" | "inline_url" | "footnote"
```

### 1.2 Diagram Renderer (Mermaid + Extensible)

Renders fenced code blocks tagged with diagram language identifiers as inline images. **Designed from day one to support all diagram-as-code languages**, not just Mermaid.

**Detection**: Triggered by fenced code block language tags — the detector maintains a registry of known diagram language identifiers.

**Supported diagram languages (initial):**

| Language Tag | Format | Notes |
|---|---|---|
| `mermaid` | Mermaid | Flowcharts, sequence, gantt, class, state, ER, pie, git |
| `plantuml` | PlantUML | UML, C4, wireframes, mindmaps |
| `graphviz` / `dot` | GraphViz | Graph/network diagrams |
| `d2` | D2 | Modern diagram language |
| `ditaa` | Ditaa | ASCII art → diagrams |
| `svgbob` | SvgBob | ASCII art → SVG |
| `erd` | Erd | Entity-relationship diagrams |
| `vegalite` | Vega-Lite | Data visualization |
| `wavedrom` | WaveDrom | Digital timing diagrams |
| `excalidraw` | Excalidraw | Hand-drawn style diagrams |

**Rendering pipeline (priority order):**
1. **Local CLI tool**: If the user has the diagram tool installed locally (e.g., `mmdc` for Mermaid, `dot` for GraphViz, `d2` for D2), use it directly — fastest, no network needed
2. **Kroki.io API**: Unified API supporting 25+ diagram languages — single endpoint, consistent interface (user opt-in for network access)
3. **Self-hosted Kroki**: User can configure a self-hosted Kroki instance URL for air-gapped environments
4. **Fallback**: Show syntax-highlighted source code with a "diagram" gutter indicator

**Diagram rendering flow:**
1. Detect fenced code block with known diagram language tag
2. Extract diagram source
3. Check cache (content-hash keyed) — return cached image if available
4. Show placeholder with spinner: `⏳ Rendering [mermaid] diagram...`
5. Render asynchronously via local tool or Kroki API → SVG/PNG
6. Display inline via par-term's Sixel/iTerm2/Kitty graphics pipeline
7. Cache the result

**Interactive features:**
- Click to zoom/expand diagram in an overlay window
- `Ctrl+Click` to copy diagram source to clipboard
- Hover tooltip shows diagram type and render time
- Right-click context menu: "Copy source", "Copy image", "Open in browser", "Re-render"

**Diagram-specific config:**
```yaml
content_prettifier:
  renderers:
    diagrams:
      enabled: true
      priority: 110
      
      # Rendering backend
      backend: "auto"                  # "auto" | "local" | "kroki" | "self_hosted_kroki"
      kroki_url: "https://kroki.io"    # Or self-hosted URL
      prefer_local_tools: true         # Try local CLI tools before Kroki
      
      # Appearance
      theme: "dark"                    # "dark" | "default" | "forest" | "neutral"
      background: "transparent"        # Match terminal background
      max_width: 800                   # Max render width in pixels
      max_height: 600                  # Max render height in pixels
      
      # Caching
      cache_enabled: true
      cache_dir: "~/.config/par-term/prettifier-cache/diagrams/"
      cache_max_size_mb: 100
      
      # Error handling
      fallback_on_error: "source"      # "source" | "error_message" | "placeholder"
      
      # Per-language overrides
      language_overrides:
        mermaid:
          local_command: "mmdc"
          local_args: ["-t", "dark", "-b", "transparent"]
        graphviz:
          local_command: "dot"
          local_args: ["-Tsvg"]
        plantuml:
          local_command: "plantuml"
          local_args: ["-tsvg", "-darkmode"]
      
      # User-registered diagram languages
      custom_languages: []
        # - tag: "structurizr"
        #   kroki_type: "structurizr"
        #   local_command: null         # No local tool, Kroki only
```

---

## Phase 2 Renderers (Fast Follow)

### 2.1 JSON Renderer

**Detection**: Opening `{` or `[` on first non-whitespace line, valid JSON structure, often following `curl`, `http`, `jq`, or API-related commands.

**Rendering:**
- Syntax highlighting (keys, strings, numbers, booleans, null in distinct colors)
- Proper indentation with tree-drawing guide lines
- Collapsible nodes — click `{` or `[` to fold/unfold nested objects/arrays
- Value type indicators (string, number, boolean, null, array length, object key count)
- Large array truncation with "... and N more items" indicator

```yaml
content_prettifier:
  renderers:
    json:
      enabled: true
      priority: 90
      indent: 2
      max_depth_expanded: 3            # Auto-collapse beyond this depth
      max_string_length: 200           # Truncate long strings with "..."
      show_array_length: true          # Show [5 items] next to arrays
      show_types: false                # Show type annotations
      sort_keys: false
      highlight_nulls: true            # Visually distinguish null values
      clickable_urls: true             # Detect URLs in string values → OSC 8
```

### 2.2 YAML Renderer

**Detection**: Lines matching `key: value` patterns, `---` document separators, indentation-based nesting. Distinguish from Markdown by absence of Markdown-specific markers.

**Rendering:**
- Syntax highlighting (keys, values, anchors, aliases, tags)
- Indentation guide lines
- Collapsible sections
- Anchor/alias resolution indicators (show what `*alias` resolves to on hover)

### 2.3 TOML Renderer

**Detection**: `[section]` headers, `key = "value"` patterns, `[[array]]` tables.

**Rendering:**
- Section headers styled prominently
- Key-value alignment
- Inline table expansion
- Type-aware value coloring

### 2.4 XML/HTML Renderer

**Detection**: `<?xml` declaration, `<tag>` patterns, `<!DOCTYPE`.

**Rendering:**
- Tag hierarchy with indentation and guide lines
- Attribute highlighting
- Collapsible elements
- Namespace coloring
- CDATA/comment distinction

### 2.5 CSV/TSV Renderer

**Detection**: Consistent delimiter patterns across multiple lines, header row heuristics.

**Rendering:**
- Tabular display using box-drawing characters (reuse Markdown table infrastructure)
- Column alignment (right-align numbers, left-align text)
- Header row styling
- Row striping for readability
- Column width auto-sizing

### 2.6 Diff/Patch Renderer

**Detection**: Lines starting with `+++`, `---`, `@@`, `diff --git`, unified diff format.

**Rendering:**
- Green/red coloring for additions/deletions
- Line number gutter
- File header styling
- Hunk header with range info
- Word-level diff highlighting within changed lines (not just line-level)
- Optional side-by-side mode (if terminal is wide enough)

```yaml
content_prettifier:
  renderers:
    diff:
      enabled: true
      priority: 95
      style: "inline"                  # "inline" | "side_by_side" | "auto"
      side_by_side_min_width: 160      # Min terminal cols for side-by-side
      word_diff: true                  # Highlight word-level changes
      show_line_numbers: true
      context_lines: 3
```

### 2.7 Log File Renderer

**Detection**: Timestamp patterns, log level keywords (INFO, WARN, ERROR, DEBUG, TRACE, FATAL), common log formats (syslog, JSON logs, Apache/Nginx).

**Rendering:**
- Log level coloring (green=INFO, yellow=WARN, red=ERROR, gray=DEBUG)
- Timestamp dimming (present but not visually dominant)
- Error/exception highlighting with stack trace folding
- JSON-in-log expansion (detect and pretty-print JSON payloads within log lines)

### 2.8 SQL Result Set Renderer

**Detection**: Output following SQL commands, column-header + separator + data row patterns from tools like `psql`, `mysql`, `sqlite3`.

**Rendering:**
- Clean table rendering with box-drawing
- NULL value highlighting
- Numeric column right-alignment
- Row count footer

### 2.9 Stack Trace Renderer

**Detection**: Language-specific patterns — `at com.example.Class.method(File.java:42)`, `File "script.py", line 42`, `thread 'main' panicked at`, `Error:` + indented `at` lines.

**Rendering:**
- Highlight the "caused by" / root error prominently
- Dim framework/library frames, highlight application code frames
- Clickable file paths via semantic history (par-term already has this)
- Collapsible "... N more frames" for long traces

---

## Phase 3: Future Renderers (Roadmap)

These renderers follow naturally from the architecture and can be contributed by the community:

| Renderer | Detection | Rendering |
|---|---|---|
| **LaTeX/Math** | `$...$`, `$$...$$`, `\begin{equation}` | Render to image via KaTeX/MathJax, display inline |
| **Protocol Buffers** | `message`, `service`, `syntax = "proto3"` | Syntax highlight + structure formatting |
| **INI/Config** | `[section]`, `key=value` without TOML features | Section headers + key-value formatting |
| **HTTP Request/Response** | `HTTP/1.1 200 OK`, `GET /path`, curl verbose output | Method coloring, header formatting, body prettifying (recursive — detect body format) |
| **Docker Compose** | YAML subset with `services:`, `volumes:`, `networks:` | Service-aware tree with port/volume mapping visualization |
| **Terraform/HCL** | `resource`, `variable`, `module`, `{` block syntax | Block-aware formatting with resource type coloring |
| **Regex** | `/pattern/flags`, complex regex in grep/sed output | Annotated breakdown of regex components |
| **Base64 blobs** | Long strings of `[A-Za-z0-9+/=]` | Decode and detect inner format, then render that |
| **Color swatches** | Hex color codes `#FF5733`, `rgb(255,87,51)` | Show inline color swatch next to the value |

---

## Claude Code `Ctrl+O` Integration

Claude Code uses `Ctrl+O` to expand/collapse detailed output (transcript view). par-term should detect and respect this interaction pattern, applied to the entire prettifier system — not just Markdown.

**Integration approach:**
- **Detect Claude Code context**: Identify via process name, `CLAUDE_CODE` environment variable, or characteristic output patterns
- **`Ctrl+O` aware rendering**: When expanded content is revealed:
  - Run the full detection pipeline on newly visible content
  - Apply appropriate prettifiers (Markdown body, JSON in tool results, diffs in file changes, etc.)
  - Maintain rendered state across collapse/expand cycles
- **Collapsed state handling**: When Claude Code shows `(ctrl+o to expand)`:
  - Display a compact rendered preview (first header + content type badge)
  - Show format indicator: `📝 Markdown` / `{} JSON` / `📊 Diagram` etc.
- **Source toggle coordination**: If the user toggles prettifying globally, respect that preference across expand/collapse
- **Multi-format awareness**: A single Claude Code response may contain Markdown with embedded JSON code blocks, Mermaid diagrams, and diff output — the prettifier should handle the nesting correctly

**Configuration:**
```yaml
content_prettifier:
  claude_code_integration:
    enabled: true
    auto_render_on_expand: true        # Run prettifiers when Ctrl+O expands
    preview_collapsed: true            # Show rendered preview for collapsed blocks
    respect_output_style: true         # Honor Claude Code's output style settings
    show_format_badges: true           # Show format type badges on collapsed blocks
```

---

## User-Defined Custom Renderers

Advanced users can register custom renderers via configuration without modifying par-term source:

### External Command Renderer

Map a detection pattern to an external command that transforms content:

```yaml
content_prettifier:
  custom_renderers:
    - format_id: "protobuf"
      display_name: "Protocol Buffers"
      detect_patterns:
        - "^syntax = \"proto[23]\";"
        - "^message\\s+\\w+\\s*\\{"
      render_command: "buf format --diff"
      render_type: "text"              # "text" | "image"
      cache: true
    
    - format_id: "ndjson"
      display_name: "Newline-Delimited JSON"
      detect_patterns:
        - "^\\{.*\\}$"                 # Every line is a JSON object
      min_matching_lines: 3
      render_command: "jq -C ."
      render_type: "text"
      
    - format_id: "openapi"
      display_name: "OpenAPI Spec"
      detect_patterns:
        - "\"openapi\":\\s*\"3\\."
        - "openapi:\\s*\"?3\\."
      render_command: "swagger-cli validate --json"
      render_type: "text"
```

### Fenced Code Block Language Renderer

Register additional fenced code block languages for diagram/visualization rendering:

```yaml
content_prettifier:
  renderers:
    diagrams:
      custom_languages:
        - tag: "tikz"
          display_name: "TikZ"
          kroki_type: "tikz"           # Use Kroki backend
          local_command: null

        - tag: "bytefield"
          display_name: "Bytefield Diagram"
          kroki_type: "bytefield"
          local_command: null
          
        - tag: "gnuplot"
          display_name: "Gnuplot Chart"
          kroki_type: null             # No Kroki support
          local_command: "gnuplot"
          local_args: ["-e", "set terminal svg; plot '-'"]
          render_type: "image"
```

---

## User Interaction & Toggle Controls

- **Global toggle**: `Cmd+Shift+M` (macOS) / `Ctrl+Shift+M` (Linux/Windows) toggles all prettifying on/off
- **Per-block toggle**: Click on a rendered block's gutter indicator to toggle that specific block between rendered and source
- **Format indicator gutter**: Small icon/badge next to prettified blocks showing the detected format:
  - `📝` Markdown
  - `{}` JSON
  - `📊` Diagram (with language name)
  - `📋` YAML / TOML
  - `📄` XML
  - `📉` CSV
  - `±` Diff
  - `📜` Log
  - `⚠️` Stack trace
- **Copy behavior**:
  - Normal copy (`Cmd+C`) copies the rendered/formatted text
  - `Cmd+Shift+C` copies the raw source
  - Vi copy mode operates on source text for accurate selection
- **Right-click context menu** on prettified blocks:
  - "Toggle source/pretty"
  - "Copy as [format]"
  - "Copy as image" (for diagrams)
  - "Open in browser" (for diagrams)
  - "Disable [format] detection" (quick way to turn off a noisy detector)

---

## Implementation Notes

### Architecture Fit

This feature aligns with and extends par-term's existing architecture:

- **Trigger system (v0.11.0)**: The prettifier registers as a new **8th action type** (`Prettify`) in the existing trigger/action system, alongside the existing 7 actions (Highlight Line, Highlight Text, Post Notification, Set Mark, Send Text, Run Command, Run Coprocess). This provides two entry points to the renderer registry: (a) auto-detection via the regex-based confidence pipeline, and (b) explicit trigger-based invocation where a user's regex match directly dispatches to a named renderer. The trigger path also supports `prettify_format: "none"` to suppress auto-detection for tools that already produce styled output
- **Inline graphics (Sixel/iTerm2/Kitty)**: Diagram renderers use the existing graphics pipeline — no new image display code needed
- **Shell integration (OSC 133)**: Command boundaries provide natural content block scoping
- **Styled font variants**: Bold, italic, bold-italic families power emphasis rendering
- **Box drawing**: Geometric box drawing powers tables in Markdown, JSON, CSV, SQL results
- **Command separators (v0.14.0)**: Horizontal rule infrastructure powers Markdown `---` and content block dividers
- **Semantic history (v0.11.0)**: File path clicking in stack traces leverages the existing Ctrl+Click → editor integration
- **Themes**: All prettifier colors derive from the active color scheme
- **Profiles**: The `enable_prettifier` setting and all `content_prettifier` sub-settings follow par-term's existing profile override pattern — profile values override global config, omitted profile fields inherit global. This is the same pattern used by `shell`, `login_shell`, badge appearance, `directory_patterns`, and `tmux_session_patterns`

### Suggested Implementation Order

1. **Phase 0 — Framework**: Build the `ContentDetector`/`ContentRenderer` trait system, the `RegexDetector` standard implementation, the `DetectionRule` model, the renderer registry, the dual-view (source/rendered) buffer management, the global toggle, per-block toggle, and gutter indicators. Load built-in regex rule sets and merge with user-defined rules from config. Register the `Prettify` action type in the existing trigger system. *This is the foundation — get it right.*
2. **Phase 1a — Markdown renderer**: Headers, bold/italic, inline code, horizontal rules, lists, blockquotes, links (text-attribute-only changes, no line count changes)
3. **Phase 1b — Markdown complex**: Tables, fenced code blocks with syntax highlighting (line count may change)
4. **Phase 1c — Diagram renderer**: Mermaid first, then PlantUML/GraphViz/D2 via Kroki integration, async rendering with placeholders
5. **Phase 1d — Claude Code integration**: Process detection, `Ctrl+O` awareness, format badges
6. **Phase 2a — Structured data**: JSON prettifier with collapsible nodes, then YAML, TOML, XML (share tree-rendering infrastructure)
7. **Phase 2b — Diff renderer**: Unified diff detection and coloring with word-level highlighting
8. **Phase 2c — Log & Stack trace**: Log level coloring, stack trace folding, framework frame dimming
9. **Phase 2d — Tabular data**: CSV/TSV and SQL result set rendering (share table infrastructure with Markdown tables)
10. **Phase 3 — User extensibility**: Custom renderer registration via config, fenced block language registration, user-defined regex rules for existing detectors, custom regex-only detectors, trigger-based prettifier invocation via Settings > Automation UI
11. **Phase 4 — Settings UI**: Full prettifier settings section with per-renderer configuration, preview, and detection tuning

### Performance Considerations

- **Detection**: Runs only at content boundaries (OSC 133 command end, alt-screen exit, process change) — never per-byte. Use `quick_match()` for fast rejection before full analysis
- **Rendering**: Cache rendered output alongside the scrollback buffer, keyed by content hash. Invalidate only when terminal width changes (triggers re-render)
- **Diagrams**: Always async — show placeholder immediately, render in background thread, update display when ready. Content-hash-based disk cache in `~/.config/par-term/prettifier-cache/`
- **Memory**: Store source + rendered + line mapping. For very large blocks (>10K lines), only render the visible portion (virtual rendering)
- **Syntax highlighting**: Use tree-sitter grammars where available (already widely used in Rust ecosystem), fall back to regex-based highlighting

### Shared Infrastructure Between Renderers

Several rendering capabilities should be built once and shared:

| Infrastructure | Used By |
|---|---|
| **Table renderer** (box-drawing, column alignment, header styling) | Markdown tables, CSV, SQL results, YAML/JSON tabular views |
| **Syntax highlighter** (tree-sitter or regex-based) | Markdown code blocks, standalone code detection, JSON/YAML/TOML/XML |
| **Tree renderer** (collapsible nodes, indentation guides) | JSON, YAML, TOML, XML |
| **Inline image display** (async render → graphics protocol) | All diagram languages, LaTeX math, image URL preview |
| **Diff coloring** (line-level + word-level) | Diff/patch output, Git output |
| **Clickable paths** (file:line → editor) | Stack traces, compiler errors, grep output |
| **Gutter system** (format badges, fold indicators, line numbers) | All renderers |

---

## Settings UI Integration

Add a new **"Content Prettifier"** section (🎨) in the Settings window with:

**Top-level controls:**
- **Enable Prettifier** master toggle — maps to `enable_prettifier` in config
  - Subtitle text dynamically generated from the renderer registry: *"Automatically detects and renders structured content in terminal output including Markdown, diagrams (Mermaid, PlantUML, GraphViz), JSON, YAML, and more. Additional format support is added regularly."*
  - **Scope badge** next to the toggle: Shows `[Global]` or `[Profile: {name}]` indicating where the current value comes from
  - When profile-overridden, a small **"Reset to global"** link appears to clear the profile override
- **Detection scope** dropdown: Command Output / All / Manual Only
- **Confidence threshold** slider (0.0 - 1.0)
- **Global toggle keybinding** display

**Per-renderer tabs/cards** (one for each enabled renderer):
- Enable/disable toggle (with **"overridden by profile"** indicator badge if applicable)
- Priority slider
- Renderer-specific settings (expandable)
- "Test detection" button — paste sample content to verify detection works
- Detection confidence preview — shows what confidence score sample content gets

**Profile override section** (within the Profile editor, under each profile):
- **Enable Prettifier** toggle with **tri-state control**:
  - **On** — Force prettifier enabled for this profile
  - **Off** — Force prettifier disabled for this profile
  - **Inherit from global** *(default)* — Use the global config value
- Collapsible **"Prettifier Overrides"** panel showing only the settings this profile changes relative to global
- **"Clear all overrides"** button to reset profile to fully inheriting global
- **Visual diff indicators** (colored dots or badges) next to any setting that differs from global, making it obvious at a glance what a profile customizes

**Diagram settings subsection:**
- Backend selection (Auto / Local / Kroki / Self-hosted)
- Kroki URL input
- Theme dropdown
- Max dimensions
- Cache management (size display, clear button)

**Claude Code subsection:**
- Integration enable/disable
- Auto-render on expand toggle
- Format badges toggle

**Custom renderers section:**
- List of user-defined renderers
- Add/edit/remove UI
- Import/export as YAML

**Detection rules section** (per-renderer expandable):
- Table of all regex rules (built-in + user-defined) with columns: ID, Pattern, Weight, Scope, Strength, Source, Enabled
- Built-in rules are read-only but can be disabled via checkbox
- User-defined rules can be edited inline or removed
- "Add rule" button with regex pattern input, weight slider, scope dropdown, strength dropdown
- "Test rules" button — paste sample content and see which rules fire, their weights, and the resulting confidence score
- Visual confidence meter showing the aggregated score for test content

**Trigger integration section** (within Settings > Automation):
- Existing trigger list now includes `Prettify` as an action type in the action dropdown
- When `Prettify` action is selected, show additional fields: format selector dropdown (populated from renderer registry), scope dropdown (`line` / `block` / `command_output`), optional block-end regex, optional command filter
- `prettify_format: "none"` option labeled as "Suppress auto-detection" for tools that already style their output

---

## Acceptance Criteria

### Framework
- [ ] Content prettifier framework with `ContentDetector` and `ContentRenderer` traits is implemented and documented
- [ ] `RegexDetector` standard implementation with weighted confidence scoring, rule scoping, and definitive-rule short-circuit is implemented
- [ ] All built-in detectors are powered by `RegexDetector` with inspectable/overridable regex rule sets
- [ ] Built-in regex rule sets are loaded at startup and merged with user-defined rules from config
- [ ] Users can add, disable, or override regex rules for any built-in detector via `config.yaml`
- [ ] Users can create entirely new detectors from regex rules alone via `config.yaml` (no Rust code required)
- [ ] `Prettify` action type is registered in par-term's existing trigger/action system
- [ ] Trigger-based prettifying bypasses confidence scoring and dispatches directly to the named renderer
- [ ] `prettify_format: "none"` suppresses auto-detection for matched content (anti-double-render)
- [ ] Trigger `command_filter` correctly scopes trigger rules to output from specific commands
- [ ] Block-scoped triggers (`prettify_scope: "block"` + `prettify_block_end`) correctly identify and render delimited blocks
- [ ] Renderer registry supports dynamic registration at startup (built-in + user-defined)
- [ ] Source/rendered dual-view is maintained for all prettified content blocks
- [ ] Global toggle and per-block toggle work correctly
- [ ] Gutter format indicators display for all prettified blocks
- [ ] Copy operations provide both rendered and source options
- [ ] Performance: zero measurable impact on non-prettified output

### Configuration & Profiles
- [ ] `enable_prettifier` setting exists in global config (`config.yaml`) with default `true`
- [ ] `enable_prettifier` can be overridden per-profile in `profiles.yaml`
- [ ] Profile-level overrides take precedence over global config; omitted profile fields inherit global values
- [ ] All `content_prettifier` sub-settings (detection, renderers, clipboard, etc.) follow the same global → profile override chain
- [ ] Settings UI shows the `enable_prettifier` toggle with dynamic subtitle listing currently supported formats
- [ ] Settings UI indicates when a value is inherited from global vs overridden by the active profile
- [ ] Profile editor includes a tri-state prettifier toggle (On / Off / Inherit) and a collapsible overrides panel
- [ ] Switching profiles at runtime immediately applies the new profile's prettifier settings (or inherits global)

### Phase 1 — Markdown & Diagrams
- [ ] Markdown is auto-detected and rendered with styled headers, code blocks, tables, emphasis, lists, blockquotes, links
- [ ] Mermaid fenced code blocks are rendered as inline graphics via the terminal's supported graphics protocol
- [ ] At least 3 additional diagram languages work via Kroki integration (PlantUML, GraphViz, D2)
- [ ] Diagram rendering is async with placeholder display and caching
- [ ] `Ctrl+O` expand in Claude Code triggers prettifier pipeline on newly visible content

### Phase 2 — Structured Data & Diffs
- [ ] JSON is auto-detected and rendered with syntax highlighting, indentation, and collapsible nodes
- [ ] YAML and TOML are auto-detected and rendered with syntax highlighting
- [ ] Unified diff output is detected and rendered with green/red coloring and word-level diff highlighting
- [ ] Log output is detected with log-level coloring
- [ ] CSV/TSV data is rendered as formatted tables

### Extensibility
- [ ] User-defined custom renderers can be registered via `config.yaml` with regex-only detection rules
- [ ] Custom fenced code block diagram languages can be registered and rendered via Kroki
- [ ] Users can add regex rules to existing built-in detectors via `config.yaml`
- [ ] Users can disable or override built-in regex rules via `config.yaml`
- [ ] Trigger-based prettifying works in Settings > Automation alongside existing trigger actions
- [ ] Settings UI shows all regex rules (built-in + user-defined) per detector with enable/disable toggles
- [ ] Settings UI includes a "Test rules" feature for validating regex patterns against sample content
- [ ] All rendering respects the active color scheme/theme
- [ ] Prettifier settings are fully per-profile-capable with proper inheritance
- [ ] Settings UI provides access to all prettifier configuration with global/profile scope indicators
- [ ] Adding a new built-in renderer requires only defining a regex rule set + implementing `ContentRenderer` — no pipeline changes