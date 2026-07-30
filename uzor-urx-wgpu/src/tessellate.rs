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
//! `encode.rs`'s `project_local` re-projects through the current
//! frame's FULL 6-coefficient affine transform every time a cached mesh
//! is replayed (Wave 4 Commit 4, design §5.4 — was a translate+scale-
//! only decomposition through Wave 1-3). Curve-flattening tolerance is
//! derived per command from the affine's largest singular value, keeping
//! the lyon error bound at `TESS_TOLERANCE_PX` in screen space without
//! over-tessellating fit-to-screen world geometry. Stroke width is also unified
//! (design §0.2): `TessKey::for_stroke` doesn't change shape at all —
//! it still just hashes whatever `Stroke.width` it's handed — but
//! `encode.rs`'s callers (`tess_stroke_scaled`) now feed it a width
//! ALREADY pre-divided by the transform's own average scale magnitude,
//! so that after this cache's mesh is reprojected through the full
//! transform at replay, the resulting on-screen stroke width comes out
//! DEVICE-CONSTANT, matching CPU's own semantic. This makes the cache
//! re-key per distinct effective scale for stroked content — a
//! disclosed, bounded cost (see `encode.rs`'s `tess_stroke_scaled` doc
//! comment), not a change to this module's own API.

use std::collections::{HashMap, VecDeque};
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

/// Maximum screen-space tessellation error. Callers convert this into
/// local-space units using the affine's largest singular value before
/// asking lyon to tessellate.
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
/// stroke params + the resolved local tessellation tolerance (design
/// §6). Colour and transform coefficients are not stored directly; the
/// transform-derived tolerance is keyed because it changes mesh density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TessKey(u64);

impl TessKey {
    #[cfg(test)]
    pub(crate) fn for_fill(path: &BezPath, rule: FillRule) -> Self {
        Self::for_fill_with_tolerance(path, rule, TESS_TOLERANCE_PX as f32)
    }

    pub(crate) fn for_fill_with_tolerance(path: &BezPath, rule: FillRule, tolerance: f32) -> Self {
        let mut h = Fnv1a::new();
        feed_path(&mut h, path);
        h.feed_byte(match rule {
            FillRule::NonZero => 0,
            FillRule::EvenOdd => 1,
        });
        h.feed_f32(tolerance);
        Self(h.finish())
    }

    #[cfg(test)]
    pub(crate) fn for_stroke(path: &BezPath, stroke: &Stroke) -> Self {
        Self::for_stroke_with_tolerance(path, stroke, TESS_TOLERANCE_PX as f32)
    }

    pub(crate) fn for_stroke_with_tolerance(path: &BezPath, stroke: &Stroke, tolerance: f32) -> Self {
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
        h.feed_f32(tolerance);
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

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TessFrameProfile {
    pub(crate) tessellate_calls: u64,
    pub(crate) tessellated_triangles: usize,
    pub(crate) tessellate_us: u128,
    pub(crate) cache_growths: u64,
    pub(crate) cache_reserved_entries: usize,
    pub(crate) cache_reserve_us: u128,
}

/// Default cap (entries) for the test-only `TessCache::new` — same
/// value as `UrxConfig::default().path_tess_cache_cap` (Commit 5 wires
/// the real config knob through `NativeUrxRenderer::with_config`;
/// `NativeUrxRenderer::new`/`with_sample_count` go through
/// `UrxConfig::default()`, so this test-only constant and that default
/// can never silently drift apart in practice).
#[cfg(test)]
const TESS_CACHE_DEFAULT_CAP: usize = 256;

struct TessCacheEntry {
    mesh: Arc<TessMesh>,
    tick: u64,
    frame: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TessCacheMode {
    Fixed,
    Adaptive,
}

/// Hash-indexed LRU. The queue may contain stale access records; their
/// tick is checked against the current map entry before eviction, making
/// lookup and ordinary eviction O(1) amortized without intrusive links.
pub(crate) struct TessCache {
    entries: HashMap<TessKey, TessCacheEntry>,
    lru: VecDeque<(TessKey, u64)>,
    tick: u64,
    frame: u64,
    current_frame_entries: usize,
    largest_profiled_mesh_triangles: usize,
    cap: usize,
    mode: TessCacheMode,
    hits: u64,
    misses: u64,
    frame_profile: TessFrameProfile,
}

impl TessCache {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_cap(TESS_CACHE_DEFAULT_CAP)
    }

    /// Construct with an explicit cap — what
    /// `NativeUrxRenderer::with_config` uses, reading
    /// `UrxConfig::path_tess_cache_cap`. The cap is read once here and
    /// never revisited: this cache is NOT hot-swappable mid-session.
    pub(crate) fn with_cap(cap: usize) -> Self {
        Self::with_mode(cap, TessCacheMode::Fixed)
    }

    /// Default-renderer mode: `cap` is the initial retained working-set
    /// size. It grows only when every resident entry has already been
    /// used in the current frame, so a scene larger than the default
    /// does not thrash by evicting geometry that same frame still needs.
    pub(crate) fn with_adaptive_cap(cap: usize) -> Self {
        Self::with_mode(cap, TessCacheMode::Adaptive)
    }

    fn with_mode(cap: usize, mode: TessCacheMode) -> Self {
        let cap = cap.max(1);
        Self {
            entries: HashMap::with_capacity(cap),
            lru: VecDeque::with_capacity(cap),
            tick: 0,
            frame: 0,
            current_frame_entries: 0,
            largest_profiled_mesh_triangles: 0,
            cap,
            mode,
            hits: 0,
            misses: 0,
            frame_profile: TessFrameProfile::default(),
        }
    }

    /// Start a renderer frame. Adaptive mode protects every entry used
    /// after this point from same-frame eviction. Stale access records
    /// are compacted occasionally in one linear pass.
    pub(crate) fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.current_frame_entries = 0;
        self.largest_profiled_mesh_triangles = 0;
        self.frame_profile = TessFrameProfile::default();
        if self.lru.len() > self.entries.len().saturating_mul(2).max(1024) {
            self.lru.retain(|(key, tick)| {
                self.entries.get(key).is_some_and(|entry| entry.tick == *tick)
            });
        }
    }

    fn get(&mut self, key: TessKey) -> Option<Arc<TessMesh>> {
        self.tick = self.tick.wrapping_add(1);
        let entry = self.entries.get_mut(&key)?;
        if entry.frame != self.frame {
            entry.frame = self.frame;
            self.current_frame_entries += 1;
        }
        entry.tick = self.tick;
        let mesh = entry.mesh.clone();
        self.lru.push_back((key, self.tick));
        self.hits += 1;
        Some(mesh)
    }

    fn insert(&mut self, key: TessKey, mesh: Arc<TessMesh>) {
        self.tick = self.tick.wrapping_add(1);
        if self.entries.len() >= self.cap {
            let adaptive_working_set_full =
                self.mode == TessCacheMode::Adaptive && self.current_frame_entries >= self.cap;
            if adaptive_working_set_full || !self.evict_oldest() {
                let old_cap = self.cap;
                self.cap = self.cap.saturating_mul(2).max(self.entries.len() + 1);
                let reserve_t0 = crate::profile::enabled().then(std::time::Instant::now);
                self.entries.reserve(self.cap - self.entries.len());
                self.lru.reserve(self.cap - self.lru.len().min(self.cap));
                self.frame_profile.cache_growths += 1;
                self.frame_profile.cache_reserved_entries = self
                    .frame_profile
                    .cache_reserved_entries
                    .saturating_add(self.cap.saturating_sub(old_cap));
                self.frame_profile.cache_reserve_us +=
                    reserve_t0.map_or(0, |started| started.elapsed().as_micros());
            }
        }
        self.entries.insert(key, TessCacheEntry { mesh, tick: self.tick, frame: self.frame });
        self.lru.push_back((key, self.tick));
        self.current_frame_entries += 1;
    }

    fn evict_oldest(&mut self) -> bool {
        while let Some((key, tick)) = self.lru.pop_front() {
            let Some(entry) = self.entries.get(&key) else {
                continue;
            };
            if entry.tick != tick {
                continue;
            }
            if self.mode == TessCacheMode::Adaptive && entry.frame == self.frame {
                self.lru.push_front((key, tick));
                return false;
            }
            self.entries.remove(&key);
            return true;
        }
        false
    }

    #[cfg(test)]
    pub(crate) fn get_or_insert_fill(&mut self, path: &BezPath, rule: FillRule) -> Arc<TessMesh> {
        self.get_or_insert_fill_with_tolerance(path, rule, TESS_TOLERANCE_PX as f32)
    }

    pub(crate) fn get_or_insert_fill_with_tolerance(
        &mut self,
        path: &BezPath,
        rule: FillRule,
        tolerance: f32,
    ) -> Arc<TessMesh> {
        let key = TessKey::for_fill_with_tolerance(path, rule, tolerance);
        if let Some(mesh) = self.get(key) {
            return mesh;
        }
        self.misses += 1;
        let tessellate_t0 = crate::profile::enabled().then(std::time::Instant::now);
        let mesh = Arc::new(tessellate_fill_with_tolerance(path, rule, tolerance));
        self.frame_profile.tessellate_calls += 1;
        self.frame_profile.tessellated_triangles = self
            .frame_profile
            .tessellated_triangles
            .saturating_add(mesh.triangles.len());
        self.frame_profile.tessellate_us +=
            tessellate_t0.map_or(0, |started| started.elapsed().as_micros());
        self.insert(key, mesh.clone());
        mesh
    }

    pub(crate) fn get_or_insert_stroke_with_tolerance(
        &mut self,
        path: &BezPath,
        stroke: &Stroke,
        tolerance: f32,
    ) -> Arc<TessMesh> {
        let key = TessKey::for_stroke_with_tolerance(path, stroke, tolerance);
        if let Some(mesh) = self.get(key) {
            return mesh;
        }
        self.misses += 1;
        let tessellate_t0 = crate::profile::enabled().then(std::time::Instant::now);
        let mesh = Arc::new(tessellate_stroke_with_tolerance(path, stroke, tolerance));
        self.frame_profile.tessellate_calls += 1;
        self.frame_profile.tessellated_triangles = self
            .frame_profile
            .tessellated_triangles
            .saturating_add(mesh.triangles.len());
        self.frame_profile.tessellate_us +=
            tessellate_t0.map_or(0, |started| started.elapsed().as_micros());
        self.insert(key, mesh.clone());
        mesh
    }

    pub(crate) fn profile_mesh(
        &mut self,
        kind: &'static str,
        path: &BezPath,
        transform: [f64; 6],
        local_tolerance: f32,
        triangles: usize,
    ) {
        if !crate::profile::enabled() || triangles <= self.largest_profiled_mesh_triangles {
            return;
        }
        self.largest_profiled_mesh_triangles = triangles;
        crate::profile::stage(
            "tess_largest_mesh",
            format_args!(
                "kind={kind} path_elements={} \
                 transform={transform:?} local_tolerance={local_tolerance:.9} triangles={triangles}",
                path.elements().len(),
            ),
        );
    }

    pub(crate) fn stats(&self) -> TessCacheStats {
        TessCacheStats { entries: self.entries.len(), hits: self.hits, misses: self.misses }
    }

    pub(crate) fn frame_profile(&self) -> TessFrameProfile {
        self.frame_profile
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

fn triangles_from_geometry(geometry: &VertexBuffers<[f32; 2], u32>) -> TessMesh {
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

#[cfg(test)]
fn tessellate_fill(path: &BezPath, rule: FillRule) -> TessMesh {
    tessellate_fill_with_tolerance(path, rule, TESS_TOLERANCE_PX as f32)
}

fn tessellate_fill_with_tolerance(path: &BezPath, rule: FillRule, tolerance: f32) -> TessMesh {
    let lyon_path = build_lyon_path(path);
    let mut geometry: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    let options = FillOptions::tolerance(tolerance).with_fill_rule(map_fill_rule(rule));
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
fn tessellate_stroke_with_tolerance(path: &BezPath, stroke: &Stroke, tolerance: f32) -> TessMesh {
    let lyon_path = build_lyon_path(path);
    let mut geometry: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let mut tessellator = StrokeTessellator::new();
    let options = StrokeOptions::tolerance(tolerance)
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
    fn tess_key_differs_by_local_tolerance() {
        let path = triangle_path();
        let a = TessKey::for_fill_with_tolerance(&path, FillRule::NonZero, 0.25);
        let b = TessKey::for_fill_with_tolerance(&path, FillRule::NonZero, 2_500.0);
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

    /// Commit 5 item 5: a custom `UrxConfig` with a small
    /// `path_tess_cache_cap` must actually cap the cache — this is the
    /// exact `TessCache::with_cap(cfg.path_tess_cache_cap)` call
    /// `NativeUrxRenderer::with_config` makes, just without the GPU
    /// device that constructor also needs (this cache is pure CPU-side
    /// data, so the config-to-cap wiring is fully testable here).
    #[test]
    fn tess_cache_respects_a_custom_config_cap() {
        // `UrxConfig` is `#[non_exhaustive]` — the builder is the only
        // way to construct a non-default config from outside `uzor-urx-core`.
        let cfg = uzor_urx_core::config::UrxConfig::builder().path_tess_cache_cap(2).build_unchecked();
        let mut cache = TessCache::with_cap(cfg.path_tess_cache_cap);

        let mut p0 = BezPath::new();
        p0.move_to(Point::new(0.0, 0.0));
        p0.line_to(Point::new(1.0, 0.0));
        p0.line_to(Point::new(0.0, 1.0));
        p0.close_path();
        let mut p1 = p0.clone();
        p1.line_to(Point::new(1.0, 1.0));
        let mut p2 = p0.clone();
        p2.line_to(Point::new(2.0, 2.0));

        let _ = cache.get_or_insert_fill(&p0, FillRule::NonZero); // miss 1
        let _ = cache.get_or_insert_fill(&p1, FillRule::NonZero); // miss 2 — cap (2) reached
        assert_eq!(cache.stats().entries, 2);

        // 3rd distinct path — must evict rather than grow past the cap.
        let _ = cache.get_or_insert_fill(&p2, FillRule::NonZero);
        let stats = cache.stats();
        assert_eq!(stats.entries, 2, "cap from UrxConfig::path_tess_cache_cap=2 must never be exceeded");
        assert_eq!(stats.misses, 3);
    }

    #[test]
    fn adaptive_cache_retains_a_working_set_larger_than_the_initial_cap() {
        const PATHS: usize = 300;
        let path = |i: usize| {
            let x = i as f64 * 2.0;
            let mut path = BezPath::new();
            path.move_to(Point::new(x, 0.0));
            path.line_to(Point::new(x + 1.0, 0.0));
            path.line_to(Point::new(x, 1.0));
            path.close_path();
            path
        };

        let mut cache = TessCache::with_adaptive_cap(256);
        cache.begin_frame();
        for i in 0..PATHS {
            let _ = cache.get_or_insert_fill(&path(i), FillRule::NonZero);
        }
        let first = cache.stats();
        assert_eq!(first.entries, PATHS);
        assert_eq!(first.hits, 0);
        assert_eq!(first.misses, PATHS as u64);

        cache.begin_frame();
        for i in 0..PATHS {
            let _ = cache.get_or_insert_fill(&path(i), FillRule::NonZero);
        }
        let second = cache.stats();
        assert_eq!(second.entries, PATHS);
        assert_eq!(second.hits - first.hits, PATHS as u64);
        assert_eq!(second.misses - first.misses, 0);
    }
}
