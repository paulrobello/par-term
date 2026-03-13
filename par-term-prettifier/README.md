# par-term-prettifier

Content prettifier framework for the par-term terminal emulator.

This crate detects structured content in terminal output (Markdown, JSON, YAML, TOML, XML,
CSV, diffs, stack traces, SQL results, log lines, Mermaid diagrams, and more) and renders
it in a rich, human-readable form with syntax highlighting and formatted tables. The
architecture is pluggable: `ContentDetector` implementations identify formats and
`ContentRenderer` implementations handle display.

## What This Crate Provides

**Detection Layer**
- `detectors` — format-specific detectors: `JsonDetector`, `MarkdownDetector`, `DiffDetector`, etc.
- `regex_detector` — generic regex-based detector for user-configured patterns
- `boundary` — line boundary tracking for multi-line content detection
- `buffer` — output buffer used during progressive detection

**Rendering Layer**
- `renderers` — format-specific renderers for Markdown, JSON, YAML, diffs, stack traces, diagrams
- `custom_renderers` — user-configured external command renderers (`ExternalCommandRenderer`)
- `gutter` — left-margin gutter indicator manager for prettified content blocks

**Pipeline / Registry Layer**
- `PrettifierPipeline` — top-level coordinator driving per-line processing
- `RendererRegistry` — maps content type identifiers to renderer implementations at runtime
- `cache` — rendered output cache to avoid re-rendering unchanged content
- `config_bridge` — bridges `par-term-config` prettifier settings to live pipeline instances
- `claude_code` — specialized detector/renderer for Claude Code XML tool-call output

**Shared Types**
- `ContentDetector` / `ContentRenderer` — core trait definitions
- `ContentBlock`, `DetectionResult`, `RenderOutput` — shared data types

## Supported Content Formats

Markdown, JSON, YAML, TOML, XML, CSV, unified diff / git diff, stack traces (Rust, Python,
Node.js, Java, Go), SQL query results, log lines, Mermaid diagrams, tree output, Claude
Code XML tool calls, and user-configured external command renderers.

## Workspace Position

Layer 2 in the dependency graph. Depends on `par-term-config`. This is the largest
sub-crate (~23k lines) due to the breadth of format-specific rendering logic. Used by the
root `par-term` crate and re-exported as `par_term::prettifier`.

## Related Documentation

- [Content Prettifier](../docs/PRETTIFIER.md) — user-facing configuration and format reference
- [Config Reference](../docs/CONFIG_REFERENCE.md) — prettifier configuration options
- [Architecture Overview](../docs/ARCHITECTURE.md) — workspace structure
- [Crate Structure](../docs/CRATE_STRUCTURE.md) — dependency layers
