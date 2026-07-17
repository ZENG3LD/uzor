//! Retained-cache surface — the L0 contract for the unification of the
//! three retained-render mechanisms (container boundary, URX region,
//! atom fragment) into one caller-facing trait.
//!
//! See `docs/uzor-tessera/plans/retained-render-unification-2026-07-18.md`
//! §1 for the full design rationale. This module adds ONLY the trait +
//! its vocabulary types — no existing `RenderContext` impl changes
//! shape, the six offscreen methods (`context.rs:62-113`) are reused
//! as-is by whatever implements [`RetainedSurface`].

use crate::core::types::Rect;

/// A retained-cache SCOPE — the granularity a paint pass wants to cache
/// at. Three scopes today; the unification's whole point is that ALL
/// THREE go through the SAME surface instead of three bespoke stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RetainedScope {
    /// A `TierId::Container` opted into `RepaintBoundaryBehavior` —
    /// today's Door A granularity.
    Container(u64),
    /// A `TierId::Container` realized as its own URX region — today's
    /// Door B granularity (same opt-in set, different channel).
    Region(u64),
    /// A `TierId::Atom` in `SkinChoice::Spec` regime — today's dormant
    /// Phase B granularity.
    Fragment(u64),
}

/// Per-scope invalidation verbs — what changed, so the surface can
/// decide keep-vs-evict without the caller re-deriving DirtyBits
/// semantics. Mirrors `tessera_kernel::core::cadence::DirtyBits`'
/// five bits by NAME (this crate cannot depend on tessera-kernel, so
/// the bits are re-declared here as the L0 contract's own vocabulary —
/// tessera's `DirtyBits` maps onto this 1:1 at the call site).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InvalidateBits(u8);

impl InvalidateBits {
    pub const NONE: Self = Self(0);
    pub const TRANSFORM: Self = Self(1 << 0);
    pub const GEOMETRY: Self = Self(1 << 1);
    pub const MATERIAL: Self = Self(1 << 2);
    pub const OPACITY: Self = Self(1 << 3);
    pub const STRUCTURE: Self = Self(1 << 4);
    pub const ALL: Self = Self(0b1_1111);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

/// Result of a `begin_scope` probe — tells the caller whether to skip
/// re-evaluation (`Reuse`) or paint fresh and capture (`Record`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeDecision {
    /// A valid cached entry exists — caller must call `replay(scope,
    /// dst_rect)` instead of re-running its paint logic.
    Reuse,
    /// No valid entry (cold, invalidated, or size mismatch) — caller
    /// paints fresh, wrapped in `begin_record`/`end_record`, which
    /// captures the content for next frame.
    Record,
}

/// Backend capability the caller may query BEFORE attempting any
/// scope — same "declare it, caller checks explicitly" convention as
/// `RenderContext::image_painter`/`BackdropBlur`
/// (`uzor/uzor/src/core/render/context.rs:49-58`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedCaps {
    /// Backend can retain `RetainedScope::Container`/`::Fragment` via
    /// the offscreen-target family (already true for tiny-skia,
    /// vello-cpu, vello-gpu).
    pub offscreen_targets: bool,
}

/// The single retained-cache surface every paint pass talks to,
/// regardless of scope. One instance per WINDOW (not per backend —
/// the hub owns exactly one per `WindowRenderState`, see
/// `uzor-render-hub::retained::RetainedCache`) — this trait is the L0
/// contract only, no default methods with real behavior (unlike
/// `RenderContext`'s draw methods, there is no sensible "no-op"
/// default that isn't just "always Record" — see [`NoRetainedCache`]
/// for that concrete no-op impl).
pub trait RetainedSurface {
    /// Capability query — call once per backend instance, not per node.
    fn caps(&self) -> RetainedCaps;

    /// Probe a scope BEFORE painting. `size` is the local (w, h) the
    /// content will occupy — a size mismatch against the stored entry
    /// is itself an invalidation signal (defense-in-depth, matches
    /// `PaintFragmentStore::is_hit`'s existing `rect_wh` check).
    fn begin_scope(&mut self, scope: RetainedScope, size: (f64, f64)) -> ScopeDecision;

    /// Replay a `Reuse`-decided scope into the currently active surface
    /// at `dst_rect` (window-local coordinates, same convention as
    /// `RenderContext::draw_cached_target`). Returns `false` if the
    /// entry vanished between `begin_scope` and this call (backend
    /// eviction under memory pressure) — caller must fall back to a
    /// fresh `Record` pass for this frame.
    fn replay(&mut self, ctx: &mut dyn super::RenderContext, scope: RetainedScope, dst_rect: Rect) -> bool;

    /// Begin capturing a `Record`-decided scope — every draw call the
    /// caller issues on `ctx` from this point until `end_record` is
    /// captured for replay. Mirrors `push_offscreen_target` exactly
    /// (same swap-current-surface convention) — because for the
    /// Scene2D-family backends this call forwards 1:1 to
    /// `ctx.push_offscreen_target`.
    fn begin_record(&mut self, ctx: &mut dyn super::RenderContext, scope: RetainedScope, size: (f64, f64));

    /// End capture, store the result keyed by `scope`.
    fn end_record(&mut self, ctx: &mut dyn super::RenderContext, scope: RetainedScope);

    /// Invalidate a scope (does not evict — next `begin_scope` for the
    /// same id returns `Record`, matching `PaintFragmentStore::
    /// mark_dirty`'s "invalidate without dropping" semantics).
    fn invalidate(&mut self, scope: RetainedScope, bits: InvalidateBits);

    /// Drop a scope's entry entirely (despawn fan-out).
    fn free(&mut self, ctx: &mut dyn super::RenderContext, scope: RetainedScope);

    /// Drop every entry for this window (backend switch, DPR change).
    fn clear(&mut self, ctx: &mut dyn super::RenderContext);
}

/// Honest no-op implementation — every scope always decides `Record`
/// and every replay/free/clear is a no-op. This is what a backend with
/// `caps().offscreen_targets == false` gets (vello-hybrid,
/// wgpu-instanced, canvas2d — the "honest None" backends stay honest
/// at THIS layer too, no silent behavior change). Also what the walker
/// uses for scopes the caller doesn't want cached at all (e.g. a plain
/// atom with no eligible cadence).
#[derive(Default)]
pub struct NoRetainedCache;

impl RetainedSurface for NoRetainedCache {
    fn caps(&self) -> RetainedCaps {
        RetainedCaps::default()
    }

    fn begin_scope(&mut self, _scope: RetainedScope, _size: (f64, f64)) -> ScopeDecision {
        ScopeDecision::Record
    }

    fn replay(&mut self, _ctx: &mut dyn super::RenderContext, _scope: RetainedScope, _dst_rect: Rect) -> bool {
        false
    }

    fn begin_record(&mut self, _ctx: &mut dyn super::RenderContext, _scope: RetainedScope, _size: (f64, f64)) {}

    fn end_record(&mut self, _ctx: &mut dyn super::RenderContext, _scope: RetainedScope) {}

    fn invalidate(&mut self, _scope: RetainedScope, _bits: InvalidateBits) {}

    fn free(&mut self, _ctx: &mut dyn super::RenderContext, _scope: RetainedScope) {}

    fn clear(&mut self, _ctx: &mut dyn super::RenderContext) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal `RenderContext` impl — same boilerplate-minimal pattern
    // `context.rs`'s own `NoImageContext` test uses; `NoRetainedCache`'s
    // mutating methods never touch `ctx` at all, so any valid
    // `RenderContext` impl works as the probe target here.
    struct NoImageContext;

    impl crate::render::Painter for NoImageContext {
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_fill_color(&mut self, _color: &str) {}
        fn set_global_alpha(&mut self, _alpha: f64) {}
        fn set_stroke_color(&mut self, _color: &str) {}
        fn set_stroke_width(&mut self, _width: f64) {}
        fn set_line_dash(&mut self, _pattern: &[f64]) {}
        fn set_line_cap(&mut self, _cap: &str) {}
        fn set_line_join(&mut self, _join: &str) {}
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
    }
    impl crate::render::TextRenderer for NoImageContext {
        fn set_font(&mut self, _font: &str) {}
        fn set_text_align(&mut self, _align: crate::render::TextAlign) {}
        fn set_text_baseline(&mut self, _baseline: crate::render::TextBaseline) {}
        fn fill_text(&mut self, _text: &str, _x: f64, _y: f64) {}
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    }
    impl crate::render::TextMetrics for NoImageContext {
        fn measure_text(&self, _text: &str) -> f64 {
            0.0
        }
        fn text_bounds(&self, _text: &str, _font: &str) -> crate::render::TextBounds {
            crate::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl crate::render::Masking for NoImageContext {
        fn clip(&mut self) {}
    }
    impl crate::render::Effects for NoImageContext {}
    impl crate::render::ShapeHelpers for NoImageContext {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    }
    impl crate::render::GradientPainter for NoImageContext {}
    impl crate::render::UiEffectHelpers for NoImageContext {}
    impl crate::render::BatchPainter for NoImageContext {}
    impl crate::render::RenderContext for NoImageContext {
        fn dpr(&self) -> f64 {
            1.0
        }
    }

    #[test]
    fn caps_default_is_false() {
        let cache = NoRetainedCache;
        assert!(!cache.caps().offscreen_targets);
    }

    #[test]
    fn begin_scope_always_decides_record() {
        let mut cache = NoRetainedCache;
        assert_eq!(
            cache.begin_scope(RetainedScope::Container(1), (10.0, 10.0)),
            ScopeDecision::Record
        );
        assert_eq!(
            cache.begin_scope(RetainedScope::Region(1), (10.0, 10.0)),
            ScopeDecision::Record
        );
        assert_eq!(
            cache.begin_scope(RetainedScope::Fragment(1), (10.0, 10.0)),
            ScopeDecision::Record
        );
    }

    #[test]
    fn replay_always_returns_false() {
        let mut cache = NoRetainedCache;
        let mut ctx = NoImageContext;
        let dst = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(!cache.replay(&mut ctx, RetainedScope::Container(1), dst));
    }

    #[test]
    fn mutating_calls_are_true_no_ops() {
        let mut cache = NoRetainedCache;
        let mut ctx = NoImageContext;
        let scope = RetainedScope::Fragment(7);

        // None of these have observable state to assert against beyond
        // "did not panic and did not change caps()/begin_scope's
        // always-Record answer" — the honest-no-op contract.
        cache.begin_record(&mut ctx, scope, (5.0, 5.0));
        cache.end_record(&mut ctx, scope);
        cache.invalidate(scope, InvalidateBits::ALL);
        cache.free(&mut ctx, scope);
        cache.clear(&mut ctx);

        assert!(!cache.caps().offscreen_targets);
        assert_eq!(cache.begin_scope(scope, (5.0, 5.0)), ScopeDecision::Record);
    }

    #[test]
    fn invalidate_bits_union_and_intersects() {
        let combo = InvalidateBits::GEOMETRY.union(InvalidateBits::MATERIAL);
        assert!(combo.intersects(InvalidateBits::GEOMETRY));
        assert!(combo.intersects(InvalidateBits::MATERIAL));
        assert!(!combo.intersects(InvalidateBits::TRANSFORM));
        assert!(!InvalidateBits::NONE.intersects(InvalidateBits::ALL));
        assert!(InvalidateBits::ALL.intersects(InvalidateBits::STRUCTURE));
    }
}
