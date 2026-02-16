# Step 17: Log & Stack Trace Renderers

## Summary

Implement log file and stack trace detectors and renderers. The log renderer provides log-level coloring, timestamp dimming, and JSON-in-log expansion. The stack trace renderer highlights root errors, dims framework frames, makes file paths clickable, and supports collapsible trace folding.

## Dependencies

- **Step 1**: Core traits and types
- **Step 2**: `RegexDetector`
- **Step 4**: `RendererRegistry`
- **Step 14**: JSON renderer (for JSON-in-log expansion)

## What to Implement

### Log Detector

#### New File: `src/prettifier/detectors/log.rs`

Log detection rules (from spec lines 1146–1148 and the pattern described in lines 412–413):

```rust
pub fn create_log_detector() -> RegexDetector {
    RegexDetector::builder("log", "Log Output")
        .confidence_threshold(0.5)
        .min_matching_rules(2)
        .definitive_shortcircuit(false)
        // Timestamp + log level is the strongest signal
        .add_rule(DetectionRule {
            id: "log_timestamp_level".into(),
            pattern: Regex::new(r"^\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}.*?(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)").unwrap(),
            weight: 0.7,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // Log level at start of line
        .add_rule(DetectionRule {
            id: "log_level_prefix".into(),
            pattern: Regex::new(r"^\s*\[?(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)\]?\s").unwrap(),
            weight: 0.5,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // ISO timestamp
        .add_rule(DetectionRule {
            id: "log_iso_timestamp".into(),
            pattern: Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}").unwrap(),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            ..
        })
        // Syslog format
        .add_rule(DetectionRule {
            id: "log_syslog".into(),
            pattern: Regex::new(r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d+\s+\d{2}:\d{2}:\d{2}").unwrap(),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // JSON log lines (structured logging)
        .add_rule(DetectionRule {
            id: "log_json_line".into(),
            pattern: Regex::new(r#"^\{"(timestamp|time|ts|level|msg|message)":#).unwrap(),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        .build()
}
```

### Log Renderer

#### New File: `src/prettifier/renderers/log.rs`

```rust
pub struct LogRenderer {
    config: LogRendererConfig,
}

impl ContentRenderer for LogRenderer {
    fn format_id(&self) -> &str { "log" }
    fn display_name(&self) -> &str { "Log Output" }
    fn capabilities(&self) -> Vec<RendererCapability> { vec![RendererCapability::TextStyling] }
    fn supports_format(&self, format_id: &str) -> bool { format_id == "log" }

    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError> { ... }
}
```

**Log rendering features** (from spec lines 1146–1153):

1. **Log level coloring**:
   - TRACE: Gray/dim
   - DEBUG: Gray or light blue
   - INFO: Green
   - WARN: Yellow
   - ERROR: Red (bold)
   - FATAL: Red background or bright red bold

2. **Timestamp dimming**: Timestamps present but dimmed (not the visual focus)

3. **Error/exception highlighting**: ERROR and FATAL lines get prominent styling (bold, bright color, optional background)

4. **Stack trace folding**: When a stack trace follows an error line, fold it and show "▸ N stack frames" (expand on click)

5. **JSON-in-log expansion**: Detect JSON payloads within log lines and offer expansion/prettification:
   ```
   2024-01-15T10:30:00Z INFO Response: {"status":200,"data":{"id":42}}
                                       ↑ detected JSON → expand/highlight
   ```

```rust
fn parse_log_line(line: &str) -> LogLine {
    LogLine {
        timestamp: extract_timestamp(line),
        level: extract_log_level(line),
        source: extract_source_info(line),  // logger name, file:line, etc.
        message: extract_message(line),
        json_payload: extract_json_payload(line),
    }
}

fn style_log_level(level: &LogLevel, theme: &ThemeColors) -> StyledSegment {
    match level {
        LogLevel::Trace => StyledSegment { fg: Some(theme.dim_color), .. },
        LogLevel::Debug => StyledSegment { fg: Some(theme.dim_color), .. },
        LogLevel::Info => StyledSegment { fg: Some(theme.green), bold: false, .. },
        LogLevel::Warn => StyledSegment { fg: Some(theme.yellow), bold: true, .. },
        LogLevel::Error => StyledSegment { fg: Some(theme.red), bold: true, .. },
        LogLevel::Fatal => StyledSegment { fg: Some(theme.red), bold: true, bg: Some(theme.error_bg), .. },
    }
}
```

### Stack Trace Detector

#### New File: `src/prettifier/detectors/stack_trace.rs`

```rust
pub fn create_stack_trace_detector() -> RegexDetector {
    RegexDetector::builder("stack_trace", "Stack Trace")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_shortcircuit(true)
        // Java/JVM stack trace
        .add_rule(DetectionRule {
            id: "stacktrace_java".into(),
            pattern: Regex::new(r"^\s+at\s+[\w.$]+\([\w.]+:\d+\)").unwrap(),
            weight: 0.7,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            ..
        })
        // Python traceback
        .add_rule(DetectionRule {
            id: "stacktrace_python_header".into(),
            pattern: Regex::new(r"^Traceback \(most recent call last\):").unwrap(),
            weight: 0.9,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            ..
        })
        .add_rule(DetectionRule {
            id: "stacktrace_python_frame".into(),
            pattern: Regex::new(r#"^\s+File ".*", line \d+"#).unwrap(),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // Rust panic
        .add_rule(DetectionRule {
            id: "stacktrace_rust_panic".into(),
            pattern: Regex::new(r"^thread '.*' panicked at").unwrap(),
            weight: 0.9,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            ..
        })
        // Node.js/JavaScript
        .add_rule(DetectionRule {
            id: "stacktrace_js".into(),
            pattern: Regex::new(r"^\s+at\s+\S+\s+\(.*:\d+:\d+\)").unwrap(),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // Generic "Error:" followed by indented "at" lines
        .add_rule(DetectionRule {
            id: "stacktrace_generic_error".into(),
            pattern: Regex::new(r"^(\w+Error|Exception|Caused by):").unwrap(),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // Go panic
        .add_rule(DetectionRule {
            id: "stacktrace_go_panic".into(),
            pattern: Regex::new(r"^goroutine \d+ \[").unwrap(),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            ..
        })
        .build()
}
```

### Stack Trace Renderer

#### New File: `src/prettifier/renderers/stack_trace.rs`

```rust
pub struct StackTraceRenderer {
    config: StackTraceRendererConfig,
}
```

**Stack trace rendering features** (from spec lines 1164–1172):

1. **Root error highlighting**: The "caused by" or root error message is prominently styled (bold red, larger visual weight)

2. **Frame classification**:
   - **Application frames**: Bright/normal color — these are the user's code
   - **Framework/library frames**: Dimmed — less relevant to debugging
   - Classification heuristic: frames containing user-configured package names are "application"

3. **Clickable file paths**: File paths with line numbers (`File.java:42`, `script.py:42`) rendered as clickable links via par-term's semantic history / Ctrl+Click feature

4. **Collapsible long traces**: Show first N frames and "... N more frames" (click to expand)
   - Keep first 3 frames visible
   - Collapse middle frames
   - Keep last "caused by" frame visible

```rust
fn classify_frame(frame: &str, app_packages: &[String]) -> FrameType {
    if app_packages.iter().any(|pkg| frame.contains(pkg)) {
        FrameType::Application
    } else {
        FrameType::Framework
    }
}

fn extract_file_path(frame: &str) -> Option<FilePath> {
    // Extract file:line patterns for clickable links
    // Java: (FileName.java:42)
    // Python: File "path/to/file.py", line 42
    // Rust: src/main.rs:42
    // JS: (file.js:42:10)
    ...
}
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/detectors/log.rs` |
| Create | `src/prettifier/detectors/stack_trace.rs` |
| Create | `src/prettifier/renderers/log.rs` |
| Create | `src/prettifier/renderers/stack_trace.rs` |
| Modify | `src/prettifier/detectors/mod.rs` (add modules) |
| Modify | `src/prettifier/renderers/mod.rs` (add modules) |

## Relevant Spec Sections

- **Lines 412–413**: Additional built-in rule sets (log, stack_trace mentioned)
- **Lines 1146–1153**: Log file renderer features
- **Lines 1164–1172**: Stack trace renderer features
- **Lines 1341**: Phase 2c — log level coloring, stack trace folding, framework frame dimming
- **Lines 1365**: Shared clickable paths infrastructure (file:line → editor)
- **Lines 1328**: Semantic history integration for file path clicking
- **Lines 1473**: Acceptance criteria — log output detected with log-level coloring

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] Log output with timestamps and log levels is detected
- [ ] Log levels (TRACE through FATAL) render with distinct colors
- [ ] Timestamps are dimmed in rendered output
- [ ] ERROR/FATAL lines are prominently highlighted
- [ ] JSON payloads within log lines are detected and expandable
- [ ] Syslog format is detected
- [ ] JSON structured logs are detected
- [ ] Java stack traces are detected and rendered
- [ ] Python tracebacks are detected and rendered
- [ ] Rust panics are detected and rendered
- [ ] Go panics are detected and rendered
- [ ] Root error / "caused by" is highlighted prominently
- [ ] Framework frames are dimmed, application frames are bright
- [ ] File paths with line numbers are rendered as clickable links
- [ ] Long traces are collapsible with "... N more frames"
- [ ] Unit tests for detection, rendering, frame classification, file path extraction
