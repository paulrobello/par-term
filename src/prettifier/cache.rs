//! Render cache for the Content Prettifier framework.
//!
//! `RenderCache` stores rendered content keyed by content hash and terminal width,
//! avoiding re-rendering unchanged blocks. Uses LRU eviction when the cache is full.

use std::collections::{HashMap, VecDeque};

use super::types::RenderedContent;

/// Caches rendered content to avoid re-rendering unchanged blocks.
/// Keyed by content hash + terminal width.
pub struct RenderCache {
    /// Cache entries: (content_hash, terminal_width) -> CacheEntry.
    entries: HashMap<(u64, usize), CacheEntry>,
    /// Maximum number of cached entries.
    max_entries: usize,
    /// LRU tracking: most recently accessed keys at the back.
    access_order: VecDeque<(u64, usize)>,
    /// Number of cache hits.
    hit_count: u64,
    /// Number of cache misses.
    miss_count: u64,
}

struct CacheEntry {
    rendered: RenderedContent,
    /// Stored for future diagnostics (e.g., cache inspection in settings UI).
    #[allow(dead_code)] // Retained for future cache diagnostics UI
    format_id: String,
}

impl RenderCache {
    /// Create a new render cache with the given maximum number of entries.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            access_order: VecDeque::new(),
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// Look up cached render result.
    pub fn get(&mut self, content_hash: u64, terminal_width: usize) -> Option<&RenderedContent> {
        let key = (content_hash, terminal_width);
        if self.entries.contains_key(&key) {
            self.hit_count += 1;
            self.touch(&key);
            // Re-borrow after touch to satisfy borrow checker.
            self.entries.get(&key).map(|e| &e.rendered)
        } else {
            self.miss_count += 1;
            None
        }
    }

    /// Store a render result.
    pub fn put(
        &mut self,
        content_hash: u64,
        terminal_width: usize,
        format_id: &str,
        rendered: RenderedContent,
    ) {
        let key = (content_hash, terminal_width);

        if self.entries.contains_key(&key) {
            // Update existing entry.
            self.entries.insert(
                key,
                CacheEntry {
                    rendered,
                    format_id: format_id.to_string(),
                },
            );
            self.touch(&key);
        } else {
            // Evict if full.
            if self.entries.len() >= self.max_entries {
                self.evict_lru();
            }
            self.entries.insert(
                key,
                CacheEntry {
                    rendered,
                    format_id: format_id.to_string(),
                },
            );
            self.access_order.push_back(key);
        }
    }

    /// Invalidate all entries for a specific content hash (any width).
    pub fn invalidate(&mut self, content_hash: u64) {
        self.entries.retain(|&(hash, _), _| hash != content_hash);
        self.access_order.retain(|&(hash, _)| hash != content_hash);
    }

    /// Clear all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
        self.hit_count = 0;
        self.miss_count = 0;
    }

    /// Get cache statistics (for diagnostics / settings UI).
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.entries.len(),
            max_entries: self.max_entries,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
        }
    }

    /// Move a key to the end of the access order (most recently used).
    fn touch(&mut self, key: &(u64, usize)) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            self.access_order.remove(pos);
        }
        self.access_order.push_back(*key);
    }

    /// Evict the least recently used entry.
    fn evict_lru(&mut self) {
        if let Some(oldest) = self.access_order.pop_front() {
            self.entries.remove(&oldest);
        }
    }
}

/// Cache statistics for diagnostics.
pub struct CacheStats {
    /// Current number of cached entries.
    pub entry_count: usize,
    /// Maximum number of cached entries.
    pub max_entries: usize,
    /// Number of cache hits.
    pub hit_count: u64,
    /// Number of cache misses.
    pub miss_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::types::*;

    fn make_rendered(text: &str) -> RenderedContent {
        RenderedContent {
            lines: vec![StyledLine::plain(text)],
            line_mapping: vec![],
            graphics: vec![],
            format_badge: "TEST".to_string(),
        }
    }

    #[test]
    fn test_cache_miss_then_hit() {
        let mut cache = RenderCache::new(10);

        // Miss.
        assert!(cache.get(123, 80).is_none());
        assert_eq!(cache.stats().miss_count, 1);
        assert_eq!(cache.stats().hit_count, 0);

        // Store.
        cache.put(123, 80, "md", make_rendered("hello"));

        // Hit.
        let result = cache.get(123, 80);
        assert!(result.is_some());
        assert_eq!(result.unwrap().lines[0].segments[0].text, "hello");
        assert_eq!(cache.stats().hit_count, 1);
    }

    #[test]
    fn test_different_widths_are_separate() {
        let mut cache = RenderCache::new(10);

        cache.put(100, 80, "md", make_rendered("80cols"));
        cache.put(100, 120, "md", make_rendered("120cols"));

        let r80 = cache.get(100, 80).unwrap();
        assert_eq!(r80.lines[0].segments[0].text, "80cols");

        let r120 = cache.get(100, 120).unwrap();
        assert_eq!(r120.lines[0].segments[0].text, "120cols");
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = RenderCache::new(2);

        cache.put(1, 80, "md", make_rendered("first"));
        cache.put(2, 80, "md", make_rendered("second"));

        // Cache is full. Adding a third should evict the LRU (key 1).
        cache.put(3, 80, "md", make_rendered("third"));

        assert!(cache.get(1, 80).is_none()); // Evicted.
        assert!(cache.get(2, 80).is_some());
        assert!(cache.get(3, 80).is_some());
    }

    #[test]
    fn test_lru_access_updates_order() {
        let mut cache = RenderCache::new(2);

        cache.put(1, 80, "md", make_rendered("first"));
        cache.put(2, 80, "md", make_rendered("second"));

        // Access key 1 to make it recently used.
        cache.get(1, 80);

        // Now key 2 is the LRU. Adding a third should evict key 2.
        cache.put(3, 80, "md", make_rendered("third"));

        assert!(cache.get(1, 80).is_some()); // Kept.
        assert!(cache.get(2, 80).is_none()); // Evicted.
        assert!(cache.get(3, 80).is_some());
    }

    #[test]
    fn test_invalidate_removes_all_widths() {
        let mut cache = RenderCache::new(10);

        cache.put(100, 80, "md", make_rendered("80"));
        cache.put(100, 120, "md", make_rendered("120"));
        cache.put(200, 80, "md", make_rendered("other"));

        cache.invalidate(100);

        assert!(cache.get(100, 80).is_none());
        assert!(cache.get(100, 120).is_none());
        assert!(cache.get(200, 80).is_some()); // Unrelated key kept.
    }

    #[test]
    fn test_clear() {
        let mut cache = RenderCache::new(10);

        cache.put(1, 80, "md", make_rendered("a"));
        cache.put(2, 80, "md", make_rendered("b"));
        cache.get(1, 80);

        cache.clear();

        assert_eq!(cache.stats().entry_count, 0);
        assert_eq!(cache.stats().hit_count, 0);
        assert_eq!(cache.stats().miss_count, 0);
        assert!(cache.get(1, 80).is_none());
    }

    #[test]
    fn test_stats() {
        let mut cache = RenderCache::new(5);

        cache.put(1, 80, "md", make_rendered("a"));
        cache.get(1, 80); // Hit.
        cache.get(2, 80); // Miss.

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.max_entries, 5);
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.miss_count, 1);
    }

    #[test]
    fn test_put_updates_existing() {
        let mut cache = RenderCache::new(10);

        cache.put(1, 80, "md", make_rendered("v1"));
        cache.put(1, 80, "md", make_rendered("v2"));

        let result = cache.get(1, 80).unwrap();
        assert_eq!(result.lines[0].segments[0].text, "v2");
        assert_eq!(cache.stats().entry_count, 1);
    }

    #[test]
    fn test_empty_cache() {
        let mut cache = RenderCache::new(10);
        assert!(cache.get(999, 80).is_none());

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 0);
        assert_eq!(stats.hit_count, 0);
        assert_eq!(stats.miss_count, 1);
    }
}
