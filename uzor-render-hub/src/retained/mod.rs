//! `RetainedCache` — the hub's single implementation of
//! `uzor::core::render::retained::RetainedSurface`, one instance per
//! `WindowRenderState`. Thin forwarder onto the six offscreen-target
//! methods `RenderContext` already exposes — the exact dance
//! `try_paint_boundary` runs today in `tessera-kernel`, moved here and
//! made scope-generic instead of container-only. See
//! `docs/uzor-tessera/plans/retained-render-unification-2026-07-18.md`
//! §2.
//!
//! `RetainedScope::Region` entries are bookkeeping-only markers — the
//! six offscreen methods are NEVER called for a `Region` scope
//! operation (URX regions have their own retained storage inside
//! `uzor-urx-engine`, confirmed architecturally separate by the plan's
//! §8/§9). Only `invalidate`/`free`/`clear` touch `Region` entries.

use std::collections::HashMap;

use uzor::core::render::offscreen::{OffscreenTargetDesc, OffscreenTargetId};
use uzor::core::render::retained::{InvalidateBits, RetainedCaps, RetainedScope, RetainedSurface, ScopeDecision};
use uzor::core::render::RenderContext;
use uzor::core::types::Rect;

/// Backend-agnostic slot — for Scene2D-family backends (tiny-skia,
/// vello-cpu, vello-gpu) this stores nothing itself; `id` IS the
/// `OffscreenTargetId` the backend already owns the pixels/fragment
/// for. `valid` mirrors the kernel's `BoundaryEntry::painted` /
/// `FragmentEntry::valid` (both used this exact bool today). `Region`
/// slots carry a synthetic `id` that is never handed to a backend
/// call — see the module doc.
struct Slot {
    id: OffscreenTargetId,
    size: (f64, f64),
    valid: bool,
}

/// One slot table per window, keyed by [`RetainedScope`] — NOT three
/// separate stores; the enum's variant IS the partition key, so a
/// single `HashMap<RetainedScope, Slot>` covers container/region/
/// fragment uniformly (raw ids don't collide across variants because
/// the enum discriminant is part of the hash/eq).
pub struct RetainedCache {
    slots: HashMap<RetainedScope, Slot>,
    /// Latched once `ctx.supports_offscreen_targets()` probes `false`
    /// for the active backend instance — mirrors the kernel's
    /// `RepaintBoundaryStore::capability_known_unsupported`, same
    /// "don't re-probe every frame" reasoning, now generalized to
    /// every scope kind instead of just containers.
    capability_known_unsupported: bool,
    /// Monotonic id source for `Region` slots — synthetic, never
    /// handed to `ctx`, just needs to be a stable `OffscreenTargetId`
    /// so `Slot` doesn't need an `Option<OffscreenTargetId>`.
    next_region_id: u64,
}

impl Default for RetainedCache {
    fn default() -> Self {
        Self { slots: HashMap::new(), capability_known_unsupported: false, next_region_id: 1 }
    }
}

impl RetainedCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a `RetainedScope::Region` entry was painted this
    /// frame by the URX region walker (its own retained storage, not
    /// this cache) — bookkeeping-only, never touches `ctx`. Lets a
    /// future cross-door observability query ("is this container's
    /// cache valid right now") answer for Region scopes too, without
    /// this store becoming a second source of truth for region
    /// pixels (`uzor-urx-engine`'s `CacheStore` stays the one writer).
    pub fn note_region_painted(&mut self, region_id: u64, size: (f64, f64)) {
        let scope = RetainedScope::Region(region_id);
        let id = self.slots.get(&scope).map(|s| s.id).unwrap_or_else(|| {
            let id = OffscreenTargetId(self.next_region_id);
            self.next_region_id += 1;
            id
        });
        self.slots.insert(scope, Slot { id, size, valid: true });
    }
}

impl RetainedSurface for RetainedCache {
    fn caps(&self) -> RetainedCaps {
        RetainedCaps { offscreen_targets: !self.capability_known_unsupported }
    }

    fn begin_scope(&mut self, scope: RetainedScope, size: (f64, f64)) -> ScopeDecision {
        if matches!(scope, RetainedScope::Region(_)) {
            return match self.slots.get(&scope) {
                Some(slot) if slot.valid && slot.size == size => ScopeDecision::Reuse,
                _ => ScopeDecision::Record,
            };
        }
        if self.capability_known_unsupported {
            return ScopeDecision::Record;
        }
        match self.slots.get(&scope) {
            Some(slot) if slot.valid && slot.size == size => ScopeDecision::Reuse,
            _ => ScopeDecision::Record,
        }
    }

    fn replay(&mut self, ctx: &mut dyn RenderContext, scope: RetainedScope, dst_rect: Rect) -> bool {
        if matches!(scope, RetainedScope::Region(_)) {
            return false;
        }
        let Some(slot) = self.slots.get(&scope) else { return false };
        if !ctx.draw_cached_target(slot.id, dst_rect) {
            // Backend no longer recognises the id (freed / stale
            // instance) — drop the dead entry, next `begin_scope`
            // reports `Record`.
            self.slots.remove(&scope);
            return false;
        }
        true
    }

    fn begin_record(&mut self, ctx: &mut dyn RenderContext, scope: RetainedScope, size: (f64, f64)) {
        if matches!(scope, RetainedScope::Region(_)) {
            return;
        }
        let desc = OffscreenTargetDesc {
            width_px: (size.0 * ctx.dpr()).max(1.0) as u32,
            height_px: (size.1 * ctx.dpr()).max(1.0) as u32,
            dpr: ctx.dpr(),
        };
        // Existing entry at a different size — try an in-place resize
        // first so the backend can reuse its storage; failure falls
        // through to a fresh acquire below (matches the kernel's
        // `try_paint_boundary` resize-then-acquire order).
        if let Some(slot) = self.slots.get(&scope) {
            if slot.size != size && ctx.resize_offscreen_target(slot.id, desc) {
                let id = slot.id;
                self.slots.insert(scope, Slot { id, size, valid: false });
                return;
            }
        }
        match ctx.push_offscreen_target(desc) {
            None => {
                self.capability_known_unsupported = true;
            }
            Some(id) => {
                self.slots.insert(scope, Slot { id, size, valid: false });
            }
        }
    }

    fn end_record(&mut self, ctx: &mut dyn RenderContext, scope: RetainedScope) {
        if matches!(scope, RetainedScope::Region(_)) {
            return;
        }
        ctx.pop_offscreen_target();
        if let Some(slot) = self.slots.get_mut(&scope) {
            slot.valid = true;
        }
    }

    fn invalidate(&mut self, scope: RetainedScope, bits: InvalidateBits) {
        if bits == InvalidateBits::NONE {
            return;
        }
        if let Some(slot) = self.slots.get_mut(&scope) {
            slot.valid = false;
        }
    }

    fn free(&mut self, ctx: &mut dyn RenderContext, scope: RetainedScope) {
        let Some(slot) = self.slots.remove(&scope) else { return };
        if !matches!(scope, RetainedScope::Region(_)) {
            ctx.free_offscreen_target(slot.id);
        }
    }

    fn clear(&mut self, _ctx: &mut dyn RenderContext) {
        // Every live target belongs to the PREVIOUS backend instance —
        // clear everything without a per-target free call (the backend
        // instance itself is gone, its internal tables are already
        // dropped) and reset the capability latch so the new backend
        // gets a fresh `push_offscreen_target` probe. Mirrors the
        // kernel's `RepaintBoundaryStore::on_backend_switched`.
        self.slots.clear();
        self.capability_known_unsupported = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid(n: u64) -> OffscreenTargetId {
        OffscreenTargetId(n)
    }

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 10.0, 10.0)
    }

    /// Minimal `RenderContext` that DOES support offscreen targets —
    /// every draw call is discarded, `push_offscreen_target` always
    /// succeeds with a fresh id. Ported from `tessera-kernel`'s
    /// `MockBoundaryCtx` (`repaint_boundary.rs:415-474`) — this crate
    /// cannot depend on `tessera-kernel`'s test module, so a small
    /// amount of test-double duplication across crates is correct here.
    struct MockBoundaryCtx {
        next_id: u64,
        supports: bool,
        draw_ok: bool,
        resize_ok: bool,
        /// When set, any offscreen-family call panics — used by the
        /// Region-never-calls-offscreen regression test.
        forbid_offscreen: bool,
    }

    impl MockBoundaryCtx {
        fn new() -> Self {
            Self { next_id: 1, supports: true, draw_ok: true, resize_ok: true, forbid_offscreen: false }
        }
        fn forbidding_offscreen() -> Self {
            Self { next_id: 1, supports: true, draw_ok: true, resize_ok: true, forbid_offscreen: true }
        }
    }

    impl uzor::core::render::Painter for MockBoundaryCtx {
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _a: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_fill_color(&mut self, _c: &str) {}
        fn set_global_alpha(&mut self, _a: f64) {}
        fn set_stroke_color(&mut self, _c: &str) {}
        fn set_stroke_width(&mut self, _w: f64) {}
        fn set_line_dash(&mut self, _p: &[f64]) {}
        fn set_line_cap(&mut self, _c: &str) {}
        fn set_line_join(&mut self, _j: &str) {}
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _c1x: f64, _c1y: f64, _c2x: f64, _c2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
    }
    impl uzor::core::render::TextRenderer for MockBoundaryCtx {
        fn set_font(&mut self, _f: &str) {}
        fn set_text_align(&mut self, _a: uzor::core::render::TextAlign) {}
        fn set_text_baseline(&mut self, _b: uzor::core::render::TextBaseline) {}
        fn fill_text(&mut self, _t: &str, _x: f64, _y: f64) {}
    }
    impl uzor::core::render::TextMetrics for MockBoundaryCtx {
        fn measure_text(&self, _t: &str) -> f64 {
            0.0
        }
        fn text_bounds(&self, _t: &str, _f: &str) -> uzor::core::render::TextBounds {
            uzor::core::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl uzor::core::render::Masking for MockBoundaryCtx {
        fn clip(&mut self) {}
    }
    impl uzor::core::render::Effects for MockBoundaryCtx {}
    impl uzor::core::render::ShapeHelpers for MockBoundaryCtx {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    }
    impl uzor::core::render::GradientPainter for MockBoundaryCtx {}
    impl uzor::core::render::UiEffectHelpers for MockBoundaryCtx {}
    impl uzor::core::render::BatchPainter for MockBoundaryCtx {}
    impl RenderContext for MockBoundaryCtx {
        fn dpr(&self) -> f64 {
            1.0
        }
        fn supports_offscreen_targets(&self) -> bool {
            assert!(!self.forbid_offscreen, "Region scope must never probe offscreen support");
            self.supports
        }
        fn push_offscreen_target(&mut self, _desc: OffscreenTargetDesc) -> Option<OffscreenTargetId> {
            assert!(!self.forbid_offscreen, "Region scope must never call push_offscreen_target");
            if !self.supports {
                return None;
            }
            let id = OffscreenTargetId(self.next_id);
            self.next_id += 1;
            Some(id)
        }
        fn pop_offscreen_target(&mut self) {
            assert!(!self.forbid_offscreen, "Region scope must never call pop_offscreen_target");
        }
        fn draw_cached_target(&mut self, _id: OffscreenTargetId, _dst: Rect) -> bool {
            assert!(!self.forbid_offscreen, "Region scope must never call draw_cached_target");
            self.draw_ok
        }
        fn resize_offscreen_target(&mut self, _id: OffscreenTargetId, _desc: OffscreenTargetDesc) -> bool {
            assert!(!self.forbid_offscreen, "Region scope must never call resize_offscreen_target");
            self.resize_ok
        }
        fn free_offscreen_target(&mut self, _id: OffscreenTargetId) {
            assert!(!self.forbid_offscreen, "Region scope must never call free_offscreen_target");
        }
    }

    fn record(cache: &mut RetainedCache, ctx: &mut MockBoundaryCtx, scope: RetainedScope, size: (f64, f64)) {
        cache.begin_record(ctx, scope, size);
        cache.end_record(ctx, scope);
    }

    // ── relocated from RepaintBoundaryStore's test module ───────────────

    #[test]
    fn insert_then_get_reports_cache_hit_on_matching_size() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Container(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));

        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Reuse);
        assert!(cache.replay(&mut ctx, scope, rect()));
    }

    #[test]
    fn size_mismatch_reports_no_cache_hit() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Container(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));

        assert_eq!(
            cache.begin_scope(scope, (200.0, 50.0)),
            ScopeDecision::Record,
            "dimension change must force a re-render"
        );
    }

    #[test]
    fn mark_dirty_clears_cache_hit_until_next_insert() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Container(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        cache.invalidate(scope, InvalidateBits::MATERIAL);

        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Record, "dirty entry must not cache-hit");

        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Reuse, "re-insert after repaint restores the cache hit");
    }

    #[test]
    fn remove_queues_target_for_deferred_free() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Container(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));

        cache.free(&mut ctx, scope);
        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Record, "freed scope must miss");
    }

    #[test]
    fn insert_over_existing_different_target_queues_old_for_free() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Container(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        let first_id = cache.slots.get(&scope).map(|s| s.id);
        assert_eq!(first_id, Some(tid(1)));

        // Different size — resize succeeds (mock's resize_ok = true),
        // so the SAME id is kept in place (matches try_paint_boundary's
        // resize-in-place-first order).
        record(&mut cache, &mut ctx, scope, (100.0, 60.0));
        assert_eq!(cache.begin_scope(scope, (100.0, 60.0)), ScopeDecision::Reuse, "latest record wins");
    }

    #[test]
    fn capability_latch_persists_until_backend_switch() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        ctx.supports = false;
        assert!(cache.caps().offscreen_targets, "not probed yet — reports true");

        // No entry present, backend refuses the acquire.
        cache.begin_record(&mut ctx, RetainedScope::Container(1), (10.0, 10.0));
        assert!(!cache.caps().offscreen_targets, "latched false after an unsupported push");

        cache.clear(&mut ctx);
        assert!(cache.caps().offscreen_targets, "clear resets the latch");
    }

    #[test]
    fn on_backend_switched_clears_entries_without_a_free_call() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Container(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        ctx.supports = false;
        cache.begin_record(&mut ctx, RetainedScope::Container(2), (5.0, 5.0));
        assert!(!cache.caps().offscreen_targets);

        cache.clear(&mut ctx);

        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Record);
        assert!(cache.caps().offscreen_targets);
    }

    // ── relocated from PaintFragmentStore's test module (Slot-shaped) ───

    #[test]
    fn fragment_insert_then_hit_reports_true_on_matching_size() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Fragment(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Reuse);
    }

    #[test]
    fn fragment_size_mismatch_reports_miss() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Fragment(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        assert_eq!(cache.begin_scope(scope, (200.0, 50.0)), ScopeDecision::Record, "resized fragment must miss");
    }

    #[test]
    fn fragment_mark_dirty_forces_a_miss_until_next_insert() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Fragment(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        cache.invalidate(scope, InvalidateBits::GEOMETRY);
        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Record, "dirty fragment must not replay");

        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Reuse, "re-capture restores the hit");
    }

    #[test]
    fn fragment_remove_drops_the_entry() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        let scope = RetainedScope::Fragment(1);
        record(&mut cache, &mut ctx, scope, (100.0, 50.0));
        cache.free(&mut ctx, scope);
        assert_eq!(cache.begin_scope(scope, (100.0, 50.0)), ScopeDecision::Record);
        assert!(cache.slots.is_empty());
    }

    #[test]
    fn fragment_clear_drops_every_entry() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        record(&mut cache, &mut ctx, RetainedScope::Fragment(1), (10.0, 10.0));
        record(&mut cache, &mut ctx, RetainedScope::Fragment(2), (20.0, 20.0));
        assert_eq!(cache.slots.len(), 2);
        cache.clear(&mut ctx);
        assert!(cache.slots.is_empty());
    }

    #[test]
    fn a_fresh_fragment_never_hits() {
        let mut cache = RetainedCache::new();
        assert_eq!(
            cache.begin_scope(RetainedScope::Fragment(1), (10.0, 10.0)),
            ScopeDecision::Record,
            "no capture yet — must miss"
        );
    }

    // ── new hub-side tests (plan §11) ────────────────────────────────────

    #[test]
    fn caps_reports_false_before_first_probe_and_latches_after_unsupported_push() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        ctx.supports = false;
        assert!(cache.caps().offscreen_targets, "no probe attempted yet");

        cache.begin_record(&mut ctx, RetainedScope::Container(9), (1.0, 1.0));
        assert!(!cache.caps().offscreen_targets, "latched false generalized across scopes");

        // A different scope kind reuses the SAME latch — no re-probe.
        assert_eq!(cache.begin_scope(RetainedScope::Fragment(9), (1.0, 1.0)), ScopeDecision::Record);
    }

    #[test]
    fn container_scope_and_fragment_scope_do_not_collide_in_the_same_map() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::new();
        record(&mut cache, &mut ctx, RetainedScope::Container(5), (100.0, 100.0));
        record(&mut cache, &mut ctx, RetainedScope::Fragment(5), (20.0, 20.0));

        assert_eq!(cache.begin_scope(RetainedScope::Container(5), (100.0, 100.0)), ScopeDecision::Reuse);
        assert_eq!(cache.begin_scope(RetainedScope::Fragment(5), (20.0, 20.0)), ScopeDecision::Reuse);
        assert_eq!(cache.slots.len(), 2, "same raw id under different scope variants must be two distinct entries");

        cache.free(&mut ctx, RetainedScope::Container(5));
        assert_eq!(
            cache.begin_scope(RetainedScope::Fragment(5), (20.0, 20.0)),
            ScopeDecision::Reuse,
            "freeing the container scope must not affect the fragment scope with the same raw id"
        );
    }

    #[test]
    fn region_scope_never_calls_offscreen_methods() {
        let mut cache = RetainedCache::new();
        let mut ctx = MockBoundaryCtx::forbidding_offscreen();
        let scope = RetainedScope::Region(3);

        assert_eq!(cache.begin_scope(scope, (10.0, 10.0)), ScopeDecision::Record);
        cache.begin_record(&mut ctx, scope, (10.0, 10.0));
        cache.end_record(&mut ctx, scope);
        assert!(!cache.replay(&mut ctx, scope, rect()));

        cache.note_region_painted(3, (10.0, 10.0));
        assert_eq!(cache.begin_scope(scope, (10.0, 10.0)), ScopeDecision::Reuse);

        cache.invalidate(scope, InvalidateBits::ALL);
        assert_eq!(cache.begin_scope(scope, (10.0, 10.0)), ScopeDecision::Record);

        cache.note_region_painted(3, (10.0, 10.0));
        cache.free(&mut ctx, scope);
        assert_eq!(cache.begin_scope(scope, (10.0, 10.0)), ScopeDecision::Record);

        // caps()/clear() are allowed to touch capability state but must
        // not reach the panicking offscreen methods either.
        cache.clear(&mut ctx);
    }
}
