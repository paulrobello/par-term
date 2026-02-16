# Step 5: Dual-View Buffer & Render Cache

## Summary

Implement the dual-view buffer system that maintains both source text and rendered output for each prettified block, along with a caching layer that avoids re-rendering unchanged content. This ensures users can always toggle back to source view and that re-renders are efficient.

## Dependencies

- **Step 1**: Core types (`ContentBlock`, `RenderedContent`, `ViewMode`, `SourceLineMapping`)
- **Step 4**: `PrettifiedBlock`, `PrettifierPipeline`

## What to Implement

### New File: `src/prettifier/buffer.rs`

The dual-view buffer manages the relationship between source and rendered content for a single prettified block.

```rust
/// Manages source text + rendered output for a single content block.
/// Supports toggling between views without re-rendering.
pub struct DualViewBuffer {
    /// The original source text (never modified)
    source: ContentBlock,
    /// The rendered output (computed lazily, cached)
    rendered: Option<RenderedContent>,
    /// Current view mode
    view_mode: ViewMode,
    /// Content hash for cache invalidation
    content_hash: u64,
    /// Terminal width at render time (re-render if width changes)
    rendered_width: Option<usize>,
}

impl DualViewBuffer {
    pub fn new(source: ContentBlock) -> Self { ... }

    /// Get the content to display based on current view mode.
    pub fn display_lines(&self) -> &[StyledLine] { ... }

    /// Set rendered content.
    pub fn set_rendered(&mut self, rendered: RenderedContent, terminal_width: usize) { ... }

    /// Check if re-rendering is needed (width changed, no cached render).
    pub fn needs_render(&self, terminal_width: usize) -> bool { ... }

    /// Toggle between source and rendered view.
    pub fn toggle_view(&mut self) { ... }

    /// Get current view mode.
    pub fn view_mode(&self) -> &ViewMode { ... }

    /// Get source text for copy operations.
    pub fn source_text(&self) -> &str { ... }

    /// Get rendered text for copy operations.
    pub fn rendered_text(&self) -> Option<&str> { ... }

    /// Map a rendered line number to the corresponding source line number.
    pub fn rendered_to_source_line(&self, rendered_line: usize) -> Option<usize> { ... }

    /// Map a source line number to rendered line number(s).
    pub fn source_to_rendered_lines(&self, source_line: usize) -> Vec<usize> { ... }

    /// Get the content hash (for cache keying).
    pub fn content_hash(&self) -> u64 { ... }

    /// Number of display lines in the current view mode.
    pub fn display_line_count(&self) -> usize { ... }
}
```

### New File: `src/prettifier/cache.rs`

```rust
use std::collections::HashMap;

/// Caches rendered content to avoid re-rendering unchanged blocks.
/// Keyed by content hash + terminal width.
pub struct RenderCache {
    /// Cache entries: (content_hash, terminal_width) -> RenderedContent
    entries: HashMap<(u64, usize), CacheEntry>,
    /// Maximum number of cached entries
    max_entries: usize,
    /// LRU tracking
    access_order: Vec<(u64, usize)>,
}

struct CacheEntry {
    rendered: RenderedContent,
    format_id: String,
    created_at: std::time::Instant,
}

impl RenderCache {
    pub fn new(max_entries: usize) -> Self { ... }

    /// Look up cached render result.
    pub fn get(&mut self, content_hash: u64, terminal_width: usize) -> Option<&RenderedContent> { ... }

    /// Store a render result.
    pub fn put(&mut self, content_hash: u64, terminal_width: usize, format_id: &str, rendered: RenderedContent) { ... }

    /// Invalidate a specific entry.
    pub fn invalidate(&mut self, content_hash: u64) { ... }

    /// Clear all cached entries.
    pub fn clear(&mut self) { ... }

    /// Evict oldest entries when cache is full.
    fn evict_lru(&mut self) { ... }

    /// Get cache statistics (for diagnostics / settings UI).
    pub fn stats(&self) -> CacheStats { ... }
}

pub struct CacheStats {
    pub entry_count: usize,
    pub max_entries: usize,
    pub hit_count: u64,
    pub miss_count: u64,
}
```

### Content Hashing

Implement a fast content hash for cache keying. Use a combination of:
- Number of lines
- First line content
- Last line content
- Total character count
- A fast hash (e.g., `ahash` or `std::hash`) of the full content

This should be fast enough to compute on every block without measurable overhead.

```rust
/// Compute a fast content hash for cache keying.
pub fn compute_content_hash(lines: &[String]) -> u64 { ... }
```

### Integration with PrettifierPipeline

Update `PrettifierPipeline` (from Step 4) to use `DualViewBuffer` and `RenderCache`:

- Each `PrettifiedBlock` wraps a `DualViewBuffer` instead of storing raw content/rendered separately
- Before rendering, check `RenderCache` for a cached result
- After rendering, store in `RenderCache`
- On terminal width change, mark blocks as needing re-render
- On copy operations, use `DualViewBuffer::source_text()` or `rendered_text()` based on copy mode

### Virtual Rendering for Large Blocks

For blocks exceeding 10K lines (spec line 1351), implement virtual rendering:

```rust
impl DualViewBuffer {
    /// For very large blocks, only render the visible portion.
    /// Returns styled lines for the visible range only.
    pub fn display_lines_range(&self, start: usize, count: usize) -> &[StyledLine] { ... }

    /// Whether this block uses virtual rendering (>10K lines).
    pub fn is_virtual(&self) -> bool { ... }
}
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/buffer.rs` |
| Create | `src/prettifier/cache.rs` |
| Modify | `src/prettifier/pipeline.rs` (use DualViewBuffer and RenderCache) |
| Modify | `src/prettifier/mod.rs` (add `pub mod buffer; pub mod cache;`) |

## Relevant Spec Sections

- **Lines 54**: "Source is always preserved — the raw source text is never discarded"
- **Lines 610–618**: Render & Cache Manager — async rendering, result caching, source ↔ rendered dual view + line mapping
- **Lines 1349–1351**: Performance — cache rendered output, invalidate on width change, virtual rendering for >10K lines
- **Lines 786–790**: Clipboard/copy behavior — rendered vs source copy modes
- **Lines 1446–1447**: Acceptance criteria — source/rendered dual-view maintained, copy operations provide both options
- **Lines 1303–1306**: Copy behavior — normal copy = rendered, Shift copy = source, Vi copy = source

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `DualViewBuffer` correctly stores source and rendered content
- [ ] `toggle_view()` switches between source and rendered display
- [ ] `source_text()` always returns the original text, never modified
- [ ] `rendered_to_source_line()` correctly maps rendered line numbers to source lines
- [ ] `needs_render()` returns true when terminal width changes
- [ ] `RenderCache::get()` returns cached content for matching hash + width
- [ ] `RenderCache` evicts LRU entries when full
- [ ] `content_hash()` produces different hashes for different content
- [ ] `content_hash()` produces same hash for identical content
- [ ] Virtual rendering for large blocks only renders the visible range
- [ ] Unit tests for buffer toggle, cache hit/miss, hash computation, line mapping
