//! Per-region identity + cache key + cached-texture marker.
//!
//! The "region" is the URX unit of independent rendering. A consumer
//! (tessera, mlc, app) declares regions; URX tracks their dirty state
//! and (optionally) keeps a cached rasterisation per region.
//!
//! Cache key carries DPR + logical size so a DPR change is a cache
//! miss (correct — pixels were rasterised at the wrong density).

use crate::math::Rect;

/// Stable, consumer-supplied region identity. Must be unique per
/// window. The consumer picks the bits (widget id hash, container id,
/// whatever). URX never invents new ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct RegionId(pub u64);

/// Cache lookup key for a `CachedRegion`. (RegionId + size + DPR.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub region:  RegionId,
    /// DPR in thousandths — 1000 = 1.0×, 2000 = 2.0×, 1500 = 1.5×.
    pub dpr_x1k: u32,
    /// Logical region size (DPR-independent, drives raster resolution).
    pub logical_w: u32,
    pub logical_h: u32,
}

/// Marker for a region that has been cached as a backend-owned texture.
///
/// URX itself does NOT hold the wgpu/cpu/hybrid handle — each backend
/// keeps its own per-key map of textures. `CachedRegion` here is the
/// metadata URX needs to drive cadence + LRU eviction policy at the
/// engine layer.
#[derive(Debug, Clone, Copy)]
pub struct CachedRegion {
    pub key:           CacheKey,
    pub last_used_us:  u64,    // monotonic timestamp, μs from engine start
    pub bytes:         u64,    // estimated texture memory cost
    pub bounds:        Rect,   // last-known logical bounds (for invalidation hit-test)
}
