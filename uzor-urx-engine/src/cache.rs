//! Retained region cache (Phase 6).
//!
//! For regions whose `RenderCadence::allows_cache()` returns true
//! (Static / LowHz), the engine keeps a CPU pixmap of the region's
//! last-rasterised pixels. On subsequent frames:
//!
//!   - `DirtyState::Clean`         → blit cached pixmap (1 memcpy)
//!   - `DirtyState::TransformOnly` → blit cached pixmap at new offset
//!   - `DirtyState::Content`       → re-rasterise into cache + blit
//!
//! For `HighHz` regions cache is bypassed entirely (cache turnover
//! cost > savings — Unreal UMG calls this "Volatile").
//!
//! LRU eviction triggers when total cached bytes exceed `BUDGET`.
//! Budget is configurable per engine (default 64 MB — research-03
//! "desktop 1080p" tier).

use std::collections::BTreeMap;

use uzor_urx_core::region::{CacheKey, RegionId};
use uzor_urx_cpu::Pixmap;

/// Default cache budget: 64 MB worth of pixel data, before eviction
/// triggers. Phase 5 default; consumers tune via `set_budget_bytes`.
pub const DEFAULT_CACHE_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(crate) struct CachedEntry {
    /// Cache key used for re-validation. Read by hybrid integration
    /// (Phase 7.5) to detect DPR / size mismatches and trigger
    /// resize-then-recreate.
    #[allow(dead_code)]
    pub key:          CacheKey,
    pub pixmap:       Pixmap,
    pub bytes:        u64,
    pub last_used_us: u64,
}

#[derive(Debug)]
pub(crate) struct CacheStore {
    entries:      BTreeMap<RegionId, CachedEntry>,
    total_bytes:  u64,
    budget_bytes: u64,
}

impl CacheStore {
    pub fn new() -> Self {
        Self {
            entries:      BTreeMap::new(),
            total_bytes:  0,
            budget_bytes: DEFAULT_CACHE_BUDGET_BYTES,
        }
    }

    pub fn set_budget(&mut self, bytes: u64) { self.budget_bytes = bytes; }
    #[allow(dead_code)]
    pub fn budget(&self) -> u64 { self.budget_bytes }
    pub fn total_bytes(&self) -> u64 { self.total_bytes }
    pub fn count(&self) -> usize { self.entries.len() }

    /// Look up a cache entry without touching its LRU timestamp.
    pub fn get(&self, id: RegionId) -> Option<&CachedEntry> {
        self.entries.get(&id)
    }

    /// Mark an entry as used now (advances its LRU position).
    pub fn touch(&mut self, id: RegionId, now_us: u64) {
        if let Some(e) = self.entries.get_mut(&id) {
            e.last_used_us = now_us;
        }
    }

    /// Insert or replace a cached entry. Triggers LRU eviction if
    /// the new total exceeds budget.
    pub fn insert(&mut self, id: RegionId, key: CacheKey, pixmap: Pixmap, now_us: u64) {
        let bytes = (pixmap.width() as u64) * (pixmap.height() as u64) * 4;
        // Drop old entry's bytes if any.
        if let Some(old) = self.entries.remove(&id) {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
        }
        self.entries.insert(id, CachedEntry {
            key, pixmap, bytes, last_used_us: now_us,
        });
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.evict_if_over_budget();
        emit_metrics(self);
    }

    /// Drop an entry by id (region removed by consumer).
    pub fn remove(&mut self, id: RegionId) {
        if let Some(e) = self.entries.remove(&id) {
            self.total_bytes = self.total_bytes.saturating_sub(e.bytes);
            metrics::counter!(uzor_urx_core::metrics_keys::KEY_CACHE_EVICT).increment(1);
        }
        emit_metrics(self);
    }

    /// Drop all entries (DPR change / full invalidation).
    pub fn clear(&mut self) {
        let n = self.entries.len() as u64;
        self.entries.clear();
        self.total_bytes = 0;
        if n > 0 {
            metrics::counter!(uzor_urx_core::metrics_keys::KEY_CACHE_EVICT).increment(n);
        }
        emit_metrics(self);
    }

    /// LRU eviction loop — pop oldest until total ≤ budget.
    fn evict_if_over_budget(&mut self) {
        while self.total_bytes > self.budget_bytes && !self.entries.is_empty() {
            // Find oldest entry.
            let oldest_id = self.entries.iter()
                .min_by_key(|(_, e)| e.last_used_us)
                .map(|(id, _)| *id);
            let Some(id) = oldest_id else { break };
            if let Some(e) = self.entries.remove(&id) {
                self.total_bytes = self.total_bytes.saturating_sub(e.bytes);
                metrics::counter!(uzor_urx_core::metrics_keys::KEY_CACHE_EVICT).increment(1);
            }
        }
    }
}

fn emit_metrics(store: &CacheStore) {
    use uzor_urx_core::metrics_keys::{KEY_CACHE_BYTES, KEY_CACHE_COUNT};
    metrics::gauge!(KEY_CACHE_BYTES).set(store.total_bytes as f64);
    metrics::gauge!(KEY_CACHE_COUNT).set(store.entries.len() as f64);
}

/// Blit a cached pixmap into the target pixmap at the given top-left
/// position. Source-over premultiplied blend per-pixel.
pub(crate) fn blit_cached(
    src: &Pixmap,
    dst: &mut Pixmap,
    dst_x: i64,
    dst_y: i64,
) {
    let sw = src.width()  as i64;
    let sh = src.height() as i64;
    let dw = dst.width()  as i64;
    let dh = dst.height() as i64;

    // Visible region of the source after clipping to the dst bounds.
    let x0 = dst_x.max(0);
    let y0 = dst_y.max(0);
    let x1 = (dst_x + sw).min(dw);
    let y1 = (dst_y + sh).min(dh);
    if x0 >= x1 || y0 >= y1 { return; }

    for py in y0 .. y1 {
        for px in x0 .. x1 {
            let sx = (px - dst_x) as u32;
            let sy = (py - dst_y) as u32;
            let p = src.get_pixel(sx, sy);
            if p[3] == 0 { continue; }
            dst.blend_pixel(px as u32, py as u32, p);
        }
    }
}
