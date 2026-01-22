# Performance and Rendering Audit: par-term

## 1. Executive Summary
The current architecture of `par-term` provides a functional and feature-rich terminal emulator using `wgpu` and `swash`. However, the rendering pipeline suffers from significant $O(N)$ overhead where $N$ is the number of cells in the grid. The CPU spends a substantial amount of time rebuilding GPU data from scratch every frame, even when the terminal content is static.

## 2. Performance Bottlenecks

### 2.1. CPU-to-GPU Data Movement
- **Rebuilding Instance Buffers**: `CellRenderer::build_instance_buffers` iterates over every cell (Rows × Cols) on every frame to reconstruct the background and text instance buffers.
- **Full Buffer Uploads**: Every frame, the entire `bg_instance_buffer` and `text_instance_buffer` are written to the GPU via `queue.write_buffer`, regardless of how many cells actually changed.
- **Redundant Cell Conversions**: `TerminalManager::get_cells_with_scrollback` clones and converts every terminal cell to the renderer's `Cell` format on every state update, performing theme lookups and color blending repeatedly.

### 2.2. Text Shaping Overhead
- **Per-Frame Shaping**: When `enable_text_shaping` is true, `shape_line` is called for every visible row in the grid every frame.
- **Run Detection**: The logic to identify text runs (grouping by font index, style, and emoji sequences) is expensive and runs for every row, involving string allocations and complex conditional checks.
- **Mapping Overhead**: Mapping shaped glyph clusters back to cell columns involves `HashMap` lookups and cluster-to-char-index calculations that are redundant for static text.

### 2.3. Glyph Atlas & Cache Management
- **LRU Sorting**: The current LRU eviction mechanism sorts all dynamic glyphs by their `last_used_frame` whenever the atlas is 90% full ($O(G \log G)$ where $G$ is the number of glyphs). This can cause noticeable frame stutters during high-throughput output.
- **Cache Key Costs**: `GlyphCacheKey` uses a `String` grapheme. Hash lookups for every cell every frame involve string hashing, which adds up for large grids.

### 2.4. Event Loop & Polling
- **Fixed-Rate Polling**: The `refresh_task` polls the terminal state at a fixed FPS (default 60). While it uses a generation counter, it still triggers a full `render()` pass whenever any change is detected, rather than performing partial updates.

## 3. Rendering Pipeline Enhancements

### 3.1. Row-Level Dirty Tracking
- **Implementation**: Add a `dirty` bitset to the terminal grid. Only cells in dirty rows should be re-converted and re-uploaded to the GPU.
- **Impact**: For typical terminal usage (e.g., typing at a prompt), this would reduce processing from ~2000 cells to ~80 cells per frame.

### 3.2. Incremental Buffer Updates
- **Implementation**: Instead of overwriting the entire instance buffer, use `queue.write_buffer` with offsets and sizes corresponding only to changed rows or regions.
- **Impact**: Drastically reduces PCIe bandwidth usage and CPU time spent in memory copying.

### 3.3. Shaping Cache
- **Implementation**: Cache the results of `shape_line` at the row level. Only invalidate the cache when the row's content or the font configuration changes.
- **Impact**: Eliminates the vast majority of HarfBuzz shaping calls and run-detection logic during steady state.

### 3.4. Optimized Glyph Atlas
- **Data Structure**: Replace the sorting-based LRU with a doubly-linked list + HashMap structure for $O(1)$ eviction and $O(1)$ access.
- **Multi-Streaming**: Consider using multiple atlas textures or an array texture to reduce the need for frequent evictions in complex scripts (e.g., CJK).

### 3.5. Shader & Attribute Optimizations
- **Monochrome Flag**: Pass a bit-flag in the instance data to indicate whether a glyph is monochrome or colored. This avoids the current RGB-check branch in the fragment shader:
  ```wgsl
  // Current check
  if (glyph.r > 0.01 || glyph.g > 0.01 || glyph.b > 0.01) { ... }
  ```
- **Combined Buffers**: Background and text quads could potentially be combined into a single draw call with interleaved attributes if the blending modes are compatible.

## 4. Suggested Roadmap

1. **Done**: Implement row-level dirty tracking foundations (optimized cache invalidation).
2. **Done**: Cache shaped text results per row.
3. **Done**: Shader attribute optimization (`is_colored` flag to avoid branching).
4. **Done**: Refactor to CPU-side incremental updates (skip processing clean rows).
5. **Done**: Implement incremental GPU buffer updates using `write_buffer` with offsets.
6. **Done**: Achieve visual parity with iTerm2 (font metrics, baseline alignment, snapped pixels, padding).
7. **Done**: Restore and optimize custom background shader support.
8. **Done**: Replace the glyph atlas LRU logic with a more efficient $O(1)$ data structure (doubly-linked list + HashMap).
9. **Long-term**: Investigate combining background and text quads into a single draw call.
