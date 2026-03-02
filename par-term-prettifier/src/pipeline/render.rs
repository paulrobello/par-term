//! Render helpers for the prettifier pipeline.
//!
//! Contains the cache-aware rendering logic extracted from [`super::pipeline_impl`]:
//! - [`render_into_buffer`] — render a single block into a `DualViewBuffer`, using
//!   the render cache to avoid redundant work.
//! - [`re_render_blocks`] — iterate active blocks and re-render any that are stale
//!   (e.g., after a terminal-width change).

use std::collections::VecDeque;

use super::super::buffer::DualViewBuffer;
use super::super::cache::RenderCache;
use super::super::registry::RendererRegistry;
use super::super::traits::RendererConfig;
use super::block::PrettifiedBlock;

/// Render content in `buffer` using the given format, populating the render
/// cache on success.
///
/// Checks the cache first; on a miss, invokes the renderer and stores the
/// result. If no renderer is registered for `format_id`, logs an error and
/// leaves the buffer un-rendered.
pub(super) fn render_into_buffer(
    render_cache: &mut RenderCache,
    registry: &RendererRegistry,
    renderer_config: &RendererConfig,
    buffer: &mut DualViewBuffer,
    format_id: &str,
    terminal_width: usize,
) {
    let content_hash = buffer.content_hash();

    crate::debug_log!(
        "PRETTIFIER",
        "pipeline::render_into_buffer: format={}, hash={:#x}, width={}, source_lines={}",
        format_id,
        content_hash,
        terminal_width,
        buffer.source().lines.len()
    );

    // Check cache first.
    if let Some(cached) = render_cache.get(content_hash, terminal_width) {
        crate::debug_info!(
            "PRETTIFIER",
            "pipeline::render_into_buffer: CACHE HIT, {} rendered lines",
            cached.lines.len()
        );
        buffer.set_rendered(cached.clone(), terminal_width);
        return;
    }

    // Render and cache.
    if let Some(renderer) = registry.get_renderer(format_id) {
        match renderer.render(buffer.source(), renderer_config) {
            Ok(rendered) => {
                crate::debug_info!(
                    "PRETTIFIER",
                    "pipeline::render_into_buffer: RENDERED {} lines -> {} styled lines, badge={:?}",
                    buffer.source().lines.len(),
                    rendered.lines.len(),
                    rendered.format_badge
                );
                // Log first few rendered lines
                for (i, line) in rendered.lines.iter().take(3).enumerate() {
                    let text: String = line.segments.iter().map(|s| s.text.as_str()).collect();
                    crate::debug_log!(
                        "PRETTIFIER",
                        "pipeline::render_into_buffer: output[{}]={:?} (segs={})",
                        i,
                        &text[..text.floor_char_boundary(100)],
                        line.segments.len()
                    );
                }
                if rendered.lines.len() > 3 {
                    crate::debug_log!(
                        "PRETTIFIER",
                        "pipeline::render_into_buffer: ... ({} more rendered lines)",
                        rendered.lines.len() - 3
                    );
                }
                render_cache.put(content_hash, terminal_width, format_id, rendered.clone());
                buffer.set_rendered(rendered, terminal_width);
            }
            Err(e) => {
                crate::debug_error!(
                    "PRETTIFIER",
                    "pipeline::render_into_buffer: RENDER FAILED format={}: {:?}",
                    format_id,
                    e
                );
            }
        }
    } else {
        crate::debug_error!(
            "PRETTIFIER",
            "pipeline::render_into_buffer: NO RENDERER found for format={}",
            format_id
        );
    }
}

/// Re-render all active blocks that are stale (e.g., after a terminal-width change).
///
/// For each block that [`DualViewBuffer::needs_render`] returns `true` for,
/// attempts a cache hit first, then falls back to a full render.
pub(super) fn re_render_blocks(
    active_blocks: &mut VecDeque<PrettifiedBlock>,
    render_cache: &mut RenderCache,
    registry: &RendererRegistry,
    renderer_config: &RendererConfig,
    terminal_width: usize,
) {
    for block in active_blocks.iter_mut() {
        if block.buffer.needs_render(terminal_width) {
            let format_id = block.detection.format_id.clone();
            let content_hash = block.buffer.content_hash();

            // Check cache first.
            if let Some(cached) = render_cache.get(content_hash, terminal_width) {
                block.buffer.set_rendered(cached.clone(), terminal_width);
            } else if let Some(renderer) = registry.get_renderer(&format_id)
                && let Ok(rendered) = renderer.render(block.buffer.source(), renderer_config)
            {
                render_cache.put(content_hash, terminal_width, &format_id, rendered.clone());
                block.buffer.set_rendered(rendered, terminal_width);
            }
        }
    }
}
