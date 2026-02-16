# Step 12: Diagram Renderer

## Summary

Implement the diagram renderer that converts fenced code blocks tagged with diagram language identifiers (Mermaid, PlantUML, GraphViz, D2, etc.) into inline images rendered via par-term's existing Sixel/iTerm2/Kitty graphics pipeline. Supports local CLI tools, Kroki.io API, and self-hosted Kroki instances, with async rendering, caching, and graceful fallback.

## Dependencies

- **Step 1**: `ContentRenderer` trait, `RenderedContent`, `InlineGraphic`, `RendererCapability`
- **Step 4**: `RendererRegistry` (to register the diagram renderer)
- **Step 5**: `RenderCache` (for diagram image caching)
- **Step 6**: Diagram config types (`DiagramRendererConfig`)

## What to Implement

### New File: `src/prettifier/renderers/diagrams.rs`

```rust
/// Renders fenced code blocks with diagram language tags as inline images.
pub struct DiagramRenderer {
    config: DiagramRendererConfig,
    /// Cache for rendered diagrams (content hash → image data)
    image_cache: DiagramCache,
    /// Registry of supported diagram languages
    languages: Vec<DiagramLanguage>,
}

pub struct DiagramLanguage {
    /// The fenced code block tag (e.g., "mermaid", "plantuml", "dot")
    pub tag: String,
    /// Display name (e.g., "Mermaid", "PlantUML")
    pub display_name: String,
    /// Kroki API type identifier (if supported by Kroki)
    pub kroki_type: Option<String>,
    /// Local CLI command to render this language
    pub local_command: Option<String>,
    /// Arguments for local CLI command
    pub local_args: Vec<String>,
}

impl DiagramRenderer {
    pub fn new(config: DiagramRendererConfig) -> Self {
        let mut languages = default_diagram_languages();
        // Merge user-defined custom_languages from config
        for custom in &config.custom_languages {
            languages.push(custom.clone().into());
        }
        Self { config, image_cache: DiagramCache::new(), languages }
    }

    /// Check if a language tag is a known diagram language.
    pub fn is_diagram_language(&self, tag: &str) -> bool { ... }

    /// Render a diagram from source code.
    /// Returns image data (RGBA) or falls back to syntax-highlighted source.
    pub async fn render_diagram(
        &self,
        language: &str,
        source: &str,
    ) -> Result<DiagramResult, RenderError> { ... }
}

pub enum DiagramResult {
    /// Successfully rendered to an image
    Image {
        data: Vec<u8>,  // RGBA pixel data
        width: u32,
        height: u32,
    },
    /// Fell back to syntax-highlighted source text
    Fallback(Vec<StyledLine>),
}
```

**Default diagram languages** (from spec lines 964–978):

| Tag | Kroki Type | Display Name |
|-----|-----------|--------------|
| `mermaid` | `mermaid` | Mermaid |
| `plantuml` | `plantuml` | PlantUML |
| `graphviz` / `dot` | `graphviz` | GraphViz |
| `d2` | `d2` | D2 |
| `ditaa` | `ditaa` | Ditaa |
| `svgbob` | `svgbob` | SvgBob |
| `erd` | `erd` | Erd |
| `vegalite` | `vegalite` | Vega-Lite |
| `wavedrom` | `wavedrom` | WaveDrom |
| `excalidraw` | `excalidraw` | Excalidraw |

### Rendering Pipeline (Priority Order)

From spec lines 979–983:

```rust
impl DiagramRenderer {
    async fn render_diagram(&self, language: &str, source: &str) -> Result<DiagramResult, RenderError> {
        // 1. Check cache
        let hash = compute_content_hash_str(source);
        if let Some(cached) = self.image_cache.get(hash) {
            return Ok(cached.clone());
        }

        // 2. Try local CLI tool (if configured and prefer_local_tools is true)
        if self.config.prefer_local_tools {
            if let Some(result) = self.try_local_render(language, source).await? {
                self.image_cache.put(hash, &result);
                return Ok(result);
            }
        }

        // 3. Try Kroki API (if backend allows)
        if self.config.backend != "local" {
            if let Some(result) = self.try_kroki_render(language, source).await? {
                self.image_cache.put(hash, &result);
                return Ok(result);
            }
        }

        // 4. Try local CLI tool (if not tried above and backend is "auto")
        if !self.config.prefer_local_tools && self.config.backend == "auto" {
            if let Some(result) = self.try_local_render(language, source).await? {
                self.image_cache.put(hash, &result);
                return Ok(result);
            }
        }

        // 5. Fallback
        match self.config.fallback_on_error.as_str() {
            "source" => Ok(DiagramResult::Fallback(highlight_source(language, source))),
            "error_message" => Err(RenderError::RenderFailed("No rendering backend available".into())),
            _ => Ok(DiagramResult::Fallback(placeholder_lines("diagram", language))),
        }
    }
}
```

#### Local CLI Rendering

```rust
impl DiagramRenderer {
    async fn try_local_render(&self, language: &str, source: &str) -> Result<Option<DiagramResult>, RenderError> {
        let lang_config = self.get_language_config(language)?;
        let command = lang_config.local_command.as_ref()?;

        // Check if command exists on PATH
        if !command_exists(command) {
            return Ok(None);
        }

        // Execute: pipe source to stdin, capture stdout (SVG/PNG)
        let output = tokio::process::Command::new(command)
            .args(&lang_config.local_args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?
            .wait_with_output()
            .await?;

        // Convert SVG/PNG to RGBA pixel data
        let image = decode_image_output(&output.stdout)?;
        Ok(Some(DiagramResult::Image { data: image.data, width: image.width, height: image.height }))
    }
}
```

#### Kroki API Rendering

```rust
impl DiagramRenderer {
    async fn try_kroki_render(&self, language: &str, source: &str) -> Result<Option<DiagramResult>, RenderError> {
        let lang_config = self.get_language_config(language)?;
        let kroki_type = lang_config.kroki_type.as_ref()?;

        let url = format!("{}/{}/png", self.config.kroki_url, kroki_type);

        // POST the diagram source, receive PNG
        let client = reqwest::Client::new();
        let response = client.post(&url)
            .body(source.to_string())
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let bytes = response.bytes().await?;
        let image = decode_png(&bytes)?;
        Ok(Some(DiagramResult::Image { data: image.data, width: image.width, height: image.height }))
    }
}
```

### Diagram Detection

Create a detector for diagram fenced code blocks:

```rust
/// Detector that identifies fenced code blocks with diagram language tags.
pub fn create_diagram_detector(languages: &[DiagramLanguage]) -> RegexDetector {
    // Build a regex that matches ```<diagram_tag>
    let tags: Vec<&str> = languages.iter().map(|l| l.tag.as_str()).collect();
    let pattern = format!(r"^```({})\s*$", tags.join("|"));

    RegexDetector::builder("diagrams", "Diagrams")
        .confidence_threshold(0.8)
        .min_matching_rules(1)
        .definitive_shortcircuit(true)
        .add_rule(DetectionRule {
            id: "diagram_fenced_block".into(),
            pattern: Regex::new(&pattern).unwrap(),
            weight: 1.0,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Fenced code block with diagram language tag".into(),
            enabled: true,
        })
        .build()
}
```

### Async Rendering & Placeholder

Since diagram rendering is slow (network calls, CLI tools), show a placeholder while rendering:

```rust
/// Placeholder shown while diagram is being rendered.
fn placeholder_content(language: &str) -> RenderedContent {
    RenderedContent {
        styled_lines: vec![StyledLine {
            segments: vec![StyledSegment {
                text: format!("⏳ Rendering [{}] diagram...", language),
                fg: Some([128, 128, 128]),
                ..Default::default()
            }],
        }],
        line_mapping: vec![],
        graphics: vec![],
        format_badge: "📊".to_string(),
    }
}
```

The pipeline should:
1. Show placeholder immediately
2. Spawn async render task
3. When render completes, update the block with the actual image
4. Trigger a re-render of the viewport

### Diagram Cache

Disk-based cache at `~/.config/par-term/prettifier-cache/diagrams/`:

```rust
pub struct DiagramCache {
    memory_cache: HashMap<u64, DiagramResult>,
    cache_dir: PathBuf,
    max_size_mb: usize,
}
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/renderers/diagrams.rs` |
| Create | `src/prettifier/detectors/diagrams.rs` |
| Modify | `src/prettifier/renderers/mod.rs` (add `pub mod diagrams;`) |
| Modify | `src/prettifier/detectors/mod.rs` (add `pub mod diagrams;`) |

## Relevant Spec Sections

- **Lines 958–1044**: Full diagram renderer specification
- **Lines 964–978**: Supported diagram languages table
- **Lines 979–998**: Rendering pipeline (local → Kroki → fallback), async flow
- **Lines 999–998**: Interactive features (click to zoom, Ctrl+Click copy, right-click menu)
- **Lines 1000–1044**: Diagram-specific config YAML
- **Lines 1323**: Inline graphics (Sixel/iTerm2/Kitty) — diagram renderers use existing pipeline
- **Lines 1337**: Phase 1c — Mermaid first, then PlantUML/GraphViz/D2 via Kroki
- **Lines 1350**: Diagrams always async with placeholder and content-hash disk cache

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `DiagramRenderer` implements `ContentRenderer` trait
- [ ] `capabilities()` includes `InlineGraphics` and `NetworkAccess`
- [ ] All 10 default diagram languages are registered
- [ ] Fenced code blocks with diagram tags are detected (definitive match)
- [ ] Local CLI rendering works when the tool is available
- [ ] Kroki API rendering works for diagram types with `kroki_type`
- [ ] Fallback to syntax-highlighted source works when no backend is available
- [ ] Placeholder is shown immediately while rendering is in progress
- [ ] Rendered images are cached (same source produces cache hit)
- [ ] Cache respects `max_size_mb` limit
- [ ] Custom diagram languages from config are registered
- [ ] `backend: "local"` skips Kroki; `backend: "kroki"` skips local
- [ ] Unit tests for language detection, cache hit/miss, fallback behavior
