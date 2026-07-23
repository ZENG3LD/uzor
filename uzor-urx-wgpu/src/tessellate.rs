//! `BezPath` → lyon-tessellated triangle mesh, with a hand-rolled LRU
//! cache keyed on LOCAL (pre-transform) geometry.
//!
//! ## Lyon API finding (design §9 risk 3)
//!
//! `lyon_path::Path::builder()` returns `lyon_path::path::Builder`,
//! which exposes `quadratic_bezier_to`/`cubic_bezier_to` DIRECTLY
//! (confirmed by reading `lyon_path-1.0.19/src/path.rs:93,708,721` in
//! the local registry cache) — curves are fed to lyon as-is, no
//! `kurbo::flatten` pre-pass needed. The legacy crate's own adapter
//! (`adapter.rs:278-286`) pre-flattens, but that's because ITS OWN
//! CPU-side `PathCmd` enum (`uzor-render-wgpu-instanced/src/context.rs`)
//! has no curve variant at all (only `MoveTo`/`LineTo`/`Close`/`Rect`/
//! `Arc`) — a limitation of that legacy representation, not of lyon.
//! This module builds the lyon path directly from `BezPath`'s
//! `PathEl::{MoveTo, LineTo, QuadTo, CurveTo, ClosePath}`, so the
//! fallback flatten path described in the design's risk section is
//! NOT needed.
//!
//! ## Cache design (design §6)
//!
//! `TessKey` is a bare FNV-1a `u64` hash — same accepted-collision-risk
//! convention already used by this exact crate family's other content-
//! addressed caches (`uzor-urx-cpu`'s `rounded::MaskKey` and
//! `gradient::hash_stops`, both plain hashed keys with `==` compare on
//! the *derived* key, not the original geometry). Storing full raw
//! path geometry per cache slot for a secondary exact-compare (the
//! `GlyphLru`-style defensive pattern design §9 risk 4 mentions) isn't
//! practical for arbitrary-length paths the way it is for `GlyphLru`'s
//! small POD key — FNV-64 collision probability is astronomically low
//! for the path counts a UI produces, and it's the same trade-off this
//! crate family already accepts elsewhere.
//!
//! `TessMesh` stores triangles in LOCAL (pre-transform) space —
//! `encode.rs` re-projects through the current frame's translate+scale
//! decomposition every time a cached mesh is replayed. See
//! `encode.rs`'s module doc for the honest write-up of what this
//! caching choice costs relative to `uzor-urx-cpu`'s per-call
//! transform-then-stroke semantics (stroke width + curve flattening
//! tolerance both behave differently under non-uniform/non-identity
//! transforms — irrelevant to every Wave 1 fixture, which is
//! identity-transform only, but a real, documented Wave 1 architecture
//! limitation).

use std::sync::Arc;

use kurbo::PathEl;
use lyon_path::math::point;
use lyon_path::Path as LyonPath;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule as LyonFillRule, FillTessellator, FillVertex, LineCap as LyonLineCap,
    LineJoin as LyonLineJoin, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};

use uzor_urx_core::math::BezPath;
use uzor_urx_core::scene::{FillRule, LineCap, LineJoin, Stroke};

/// Local-space (pre-transform) tessellation tolerance — matches the
/// `uzor-urx-cpu` + legacy-adapter convention
/// (`uzor-urx-cpu/src/path.rs:33` `FLATTEN_TOLERANCE_PX`,
/// `uzor-urx-wgpu/src/adapter.rs:277`). Folded into every `TessKey` so
/// a future tolerance bump invalidates old cache entries automatically
/// (design §6).
pub(crate) const TESS_TOLERANCE_PX: f64 = 0.25;

/// Hand-rolled FNV-1a-64 accumulator — same algorithm `uzor-urx-cpu`'s
/// `gradient::hash_stops` uses, spelled out locally so this module
/// doesn't need a cross-crate helper for a 10-line hash loop.
struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn feed_byte(&mut self, b: u8) {
        self.0 ^= b as u64;
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn feed_bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.feed_byte(*b);
        }
    }

    fn feed_f64(&mut self, v: f64) {
        self.feed_bytes(&v.to_bits().to_le_bytes());
    }

    fn feed_f32(&mut self, v: f32) {
        self.feed_bytes(&v.to_bits().to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

fn feed_path(h: &mut Fnv1a, path: &BezPath) {
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                h.feed_byte(0);
                h.feed_f64(p.x);
                h.feed_f64(p.y);
            }
            PathEl::LineTo(p) => {
                h.feed_byte(1);
                h.feed_f64(p.x);
                h.feed_f64(p.y);
            }
            PathEl::QuadTo(c, p) => {
                h.feed_byte(2);
                h.feed_f64(c.x);
                h.feed_f64(c.y);
                h.feed_f64(p.x);
                h.feed_f64(p.y);
            }
            PathEl::CurveTo(c1, c2, p) => {
                h.feed_byte(3);
                h.feed_f64(c1.x);
                h.feed_f64(c1.y);
                h.feed_f64(c2.x);
                h.feed_f64(c2.y);
                h.feed_f64(p.x);
                h.feed_f64(p.y);
            }
            PathEl::ClosePath => h.feed_byte(4),
        }
    }
}

/// Content hash of the LOCAL (pre-transform) path geometry + fill/
/// stroke params + the tessellation tolerance (design §6). Transform
/// and colour are NOT part of the key — they vary per-instance and are
/// applied when re-projecting the cached mesh, not when tessellating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TessKey(u64);

impl TessKey {
    pub(crate) fn for_fill(path: &BezPath, rule: FillRule) -> Self {
        let mut h = Fnv1a::new();
        feed_path(&mut h, path);
        h.feed_byte(match rule {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        });
        h.feed_f64(TESS_TOLERANCE_PX);
        Self(h.finish())
    }

    pub(crate) fn for_stroke(path: &BezPath, stroke: &Stroke) -> Self {
        let mut h = Fnv1a::new();
        feed_path(&mut h, path);
        h.feed_f32(stroke.width);
        h.feed_f32(stroke.miter_limit);
        h.feed_byte(match stroke.join {
            LineJoin::Miter => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        });
        h.feed_byte(match stroke.cap {
            LineCap::Butt => 0,
            LineCap::Round => 1,
            LineCap::Square => 2,
        });
        h.feed_f64(TESS_TOLERANCE_PX);
        Self(h.finish())
    }
}

/// Local-space (pre-transform) triangle soup — re-projected through
/// the current-frame transform + colour every time it's replayed.
#[derive(Debug, Default)]
pub(crate) struct TessMesh {
    pub(crate) triangles: Vec<[[f32; 2]; 3]>,
}

/// Read-only cache telemetry (design §2) — hit/miss/entry counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TessCacheStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
}

/// Hardcoded this commit (design §6) — the `UrxConfig` knob
/// (`path_tess_cache_cap`) lands in Commit 5.
const TESS_CACHE_CAP: usize = 256;

/// Hand-rolled LRU — same shape as `uzor-urx-glyph::GlyphLru`:
/// `Vec<(key, value, tick)>` + linear scan + `swap_remove` of the
/// min-tick entry on overflow.
pub(crate) struct TessCache {
    entries: Vec<(TessKey, Arc<TessMesh>, u64)>,
    tick: u64,
    cap: usize,
    hits: u64,
    misses: u64,
}

impl TessCache {
    pub(crate) fn new() -> Self {
        Self { entries: Vec::new(), tick: 0, cap: TESS_CACHE_CAP, hits: 0, misses: 0 }
    }

    fn get(&mut self, key: TessKey) -> Option<Arc<TessMesh>> {
        self.tick = self.tick.wrapping_add(1);
        for entry in self.entries.iter_mut() {
            if entry.0 == key {
                entry.2 = self.tick;
                self.hits += 1;
                return Some(entry.1.clone());
            }
        }
        self.misses += 1;
        None
    }

    fn insert(&mut self, key: TessKey, mesh: Arc<TessMesh>) {
        self.tick = self.tick.wrapping_add(1);
        if self.entries.len() >= self.cap {
            if let Some((idx, _)) = self.entries.iter().enumerate().min_by_key(|(_, e)| e.2) {
                self.entries.swap_remove(idx);
            }
        }
        self.entries.push((key, mesh, self.tick));
    }

    pub(crate) fn get_or_insert_fill(&mut self, path: &BezPath, rule: FillRule) -> Arc<TessMesh> {
        let key = TessKey::for_fill(path, rule);
        if let Some(mesh) = self.get(key) {
            return mesh;
        }
        let mesh = Arc::new(tessellate_fill(path, rule));
        self.insert(key, mesh.clone());
        mesh
    }

    pub(crate) fn get_or_insert_stroke(&mut self, path: &BezPath, stroke: &Stroke) -> Arc<TessMesh> {
        let key = TessKey::for_stroke(path, stroke);
        if let Some(mesh) = self.get(key) {
            return mesh;
        }
        let mesh = Arc::new(tessellate_stroke(path, stroke));
        self.insert(key, mesh.clone());
        mesh
    }

    pub(crate) fn stats(&self) -> TessCacheStats {
        TessCacheStats { entries: self.entries.len(), hits: self.hits, misses: self.misses }
    }
}

/// Build a `lyon_path::Path` directly from `BezPath`'s elements — no
/// pre-flatten (see this module's doc comment, lyon API finding).
fn build_lyon_path(path: &BezPath) -> LyonPath {
    let mut builder = LyonPath::builder();
    let mut in_subpath = false;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                if in_subpath {
                    builder.end(false);
                }
                builder.begin(point(p.x as f32, p.y as f32));
                in_subpath = true;
            }
            PathEl::LineTo(p) => {
                if in_subpath {
                    builder.line_to(point(p.x as f32, p.y as f32));
                } else {
                    builder.begin(point(p.x as f32, p.y as f32));
                    in_subpath = true;
                }
            }
            PathEl::QuadTo(c, p) => {
                if in_subpath {
                    builder.quadratic_bezier_to(point(c.x as f32, c.y as f32), point(p.x as f32, p.y as f32));
                }
                // A curve command with no preceding `MoveTo` is a
                // malformed `BezPath` — silently dropped rather than
                // guessing a start point, matching this crate's
                // "never panic on scene content" policy.
            }
            PathEl::CurveTo(c1, c2, p) => {
                if in_subpath {
                    builder.cubic_bezier_to(
                        point(c1.x as f32, c1.y as f32),
                        point(c2.x as f32, c2.y as f32),
                        point(p.x as f32, p.y as f32),
                    );
                }
            }
            PathEl::ClosePath => {
                if in_subpath {
                    builder.end(true);
                    in_subpath = false;
                }
            }
        }
    }
    if in_subpath {
        builder.end(false);
    }
    builder.build()
}

fn map_fill_rule(rule: FillRule) -> LyonFillRule {
    match rule {
        FillRule::NonZero => LyonFillRule::NonZero,
        FillRule::EvenOdd => LyonFillRule::EvenOdd,
    }
}

fn map_line_cap(cap: LineCap) -> LyonLineCap {
    match cap {
        LineCap::Butt => LyonLineCap::Butt,
        LineCap::Round => LyonLineCap::Round,
        LineCap::Square => LyonLineCap::Square,
    }
}

fn map_line_join(join: LineJoin) -> LyonLineJoin {
    match join {
        LineJoin::Miter => LyonLineJoin::Miter,
        LineJoin::Round => LyonLineJoin::Round,
        LineJoin::Bevel => LyonLineJoin::Bevel,
    }
}

fn triangles_from_geometry(geometry: &VertexBuffers<[f32; 2], u16>) -> TessMesh {
    let mut triangles = Vec::with_capacity(geometry.indices.len() / 3);
    for tri in geometry.indices.chunks_exact(3) {
        triangles.push([
            geometry.vertices[tri[0] as usize],
            geometry.vertices[tri[1] as usize],
            geometry.vertices[tri[2] as usize],
        ]);
    }
    TessMesh { triangles }
}

fn tessellate_fill(path: &BezPath, rule: FillRule) -> TessMesh {
    let lyon_path = build_lyon_path(path);
    let mut geometry: VertexBuffers<[f32; 2], u16> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    let options = FillOptions::tolerance(TESS_TOLERANCE_PX as f32).with_fill_rule(map_fill_rule(rule));
    // Malformed/self-intersecting-beyond-lyon's-handling paths degrade
    // to an empty mesh (nothing drawn) rather than panicking — the
    // `Result` is intentionally not propagated further, matching this
    // crate's "never panic on scene content" policy.
    let _ = tessellator.tessellate_path(
        &lyon_path,
        &options,
        &mut BuffersBuilder::new(&mut geometry, |v: FillVertex| v.position().to_array()),
    );
    triangles_from_geometry(&geometry)
}

/// Tessellates in LOCAL (pre-transform) units — `stroke.width` is fed
/// to lyon exactly as given, i.e. the geometric stroke width baked
/// into the cached mesh is a LOCAL-space quantity. At replay
/// (`encode.rs::emit_solid_mesh`) the whole mesh — including this
/// baked-in width — is re-projected through the frame's translate+
/// scale, so under a non-1.0 scale the apparent on-screen stroke width
/// scales with it. See this module's doc comment + `encode.rs`'s
/// module doc for how this compares to `uzor-urx-cpu::stroke_path_aa`.
fn tessellate_stroke(path: &BezPath, stroke: &Stroke) -> TessMesh {
    let lyon_path = build_lyon_path(path);
    let mut geometry: VertexBuffers<[f32; 2], u16> = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();
    let options = StrokeOptions::tolerance(TESS_TOLERANCE_PX as f32)
        .with_line_width(stroke.width)
        .with_line_cap(map_line_cap(stroke.cap))
        .with_line_join(map_line_join(stroke.join))
        .with_miter_limit(stroke.miter_limit.max(1.0));
    let _ = tessellator.tessellate_path(
        &lyon_path,
        &options,
        &mut BuffersBuilder::new(&mut geometry, |v: StrokeVertex| v.position().to_array()),
    );
    triangles_from_geometry(&geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_urx_core::math::Point;

    fn triangle_path() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(5.0, 10.0));
        p.close_path();
        p
    }

    #[test]
    fn tess_key_stable_for_the_same_path_and_rule() {
        let a = TessKey::for_fill(&triangle_path(), FillRule::NonZero);
        let b = TessKey::for_fill(&triangle_path(), FillRule::NonZero);
        assert_eq!(a, b);
    }

    #[test]
    fn tess_key_differs_by_fill_rule() {
        let a = TessKey::for_fill(&triangle_path(), FillRule::NonZero);
        let b = TessKey::for_fill(&triangle_path(), FillRule::EvenOdd);
        assert_ne!(a, b);
    }

    #[test]
    fn tess_key_differs_by_stroke_width() {
        let a = TessKey::for_stroke(&triangle_path(), &Stroke { width: 2.0, ..Stroke::default() });
        let b = TessKey::for_stroke(&triangle_path(), &Stroke { width: 3.0, ..Stroke::default() });
        assert_ne!(a, b);
    }

    #[test]
    fn tess_key_differs_by_geometry() {
        let mut other = triangle_path();
        other.line_to(Point::new(6.0, 11.0));
        let a = TessKey::for_fill(&triangle_path(), FillRule::NonZero);
        let b = TessKey::for_fill(&other, FillRule::NonZero);
        assert_ne!(a, b);
    }

    #[test]
    fn fill_triangle_tessellates_to_at_least_one_triangle() {
        let mesh = tessellate_fill(&triangle_path(), FillRule::NonZero);
        assert!(!mesh.triangles.is_empty());
    }

    #[test]
    fn cache_hit_after_first_insert() {
        let mut cache = TessCache::new();
        let path = triangle_path();
        let _m1 = cache.get_or_insert_fill(&path, FillRule::NonZero);
        let stats1 = cache.stats();
        assert_eq!(stats1.misses, 1);
        assert_eq!(stats1.hits, 0);

        let _m2 = cache.get_or_insert_fill(&path, FillRule::NonZero);
        let stats2 = cache.stats();
        assert_eq!(stats2.misses, 1);
        assert_eq!(stats2.hits, 1);
        assert_eq!(stats2.entries, 1);
    }

    #[test]
    fn cache_evicts_min_tick_entry_on_overflow() {
        let mut cache = TessCache::new();
        cache.cap = 2;
        let mut p0 = BezPath::new();
        p0.move_to(Point::new(0.0, 0.0));
        p0.line_to(Point::new(1.0, 0.0));
        p0.line_to(Point::new(0.0, 1.0));
        p0.close_path();

        let mut p1 = p0.clone();
        p1.line_to(Point::new(1.0, 1.0));

        let mut p2 = p0.clone();
        p2.line_to(Point::new(2.0, 2.0));

        let _ = cache.get_or_insert_fill(&p0, FillRule::NonZero); // miss, insert p0
        let _ = cache.get_or_insert_fill(&p1, FillRule::NonZero); // miss, insert p1 (cap reached)
        let _ = cache.get_or_insert_fill(&p0, FillRule::NonZero); // hit — bumps p0's tick above p1's
        let _ = cache.get_or_insert_fill(&p2, FillRule::NonZero); // miss, must evict p1 (min tick), not p0

        // p0 must still be a hit (it was NOT evicted).
        let before = cache.stats();
        let _ = cache.get_or_insert_fill(&p0, FillRule::NonZero);
        let after = cache.stats();
        assert_eq!(after.hits, before.hits + 1, "p0 should still be cached (most recently used)");
        assert_eq!(after.entries, 2, "cap of 2 must never be exceeded");
    }
}
