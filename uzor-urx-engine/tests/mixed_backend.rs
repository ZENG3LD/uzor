//! Wave 9 — Mixed-backend MixDispatcher tests.
//!
//! Engine guarantees:
//!   - stable RegionId iteration order in dispatcher callbacks
//!   - per-region BackendHint is forwarded verbatim
//!   - all regions' DirtyState is cleared on a successful Mixed render

use uzor_urx_core::math::Rect;
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_engine::{
    Backend, BackendHint, MixDispatcher, RenderCadence, RenderTarget, UrxEngine,
};

struct RecordingDispatcher {
    calls: Vec<(RegionId, Rect, BackendHint)>,
}

impl MixDispatcher for RecordingDispatcher {
    fn dispatch(&mut self, id: RegionId, bounds: Rect, hint: BackendHint, _scene: &Scene) {
        self.calls.push((id, bounds, hint));
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x0: x, y0: y, x1: x + w, y1: y + h }
}

#[test]
fn mixed_backend_dispatches_per_region_in_id_order() {
    let mut e = UrxEngine::new_mixed(800, 600);
    assert_eq!(e.backend(), Backend::Mixed);

    // Insert three regions out of RegionId order — engine must
    // re-sort by RegionId for dispatcher callbacks.
    e.upsert_region_with_hint(
        RegionId(5), Scene::new(), rect(100.0, 100.0, 200.0, 50.0),
        RenderCadence::Static, BackendHint::Cpu,
    );
    e.upsert_region_with_hint(
        RegionId(1), Scene::new(), rect(0.0, 0.0, 800.0, 50.0),
        RenderCadence::Static, BackendHint::Cpu,
    );
    e.upsert_region_with_hint(
        RegionId(3), Scene::new(), rect(0.0, 50.0, 800.0, 550.0),
        RenderCadence::HighHz, BackendHint::FullGpu,
    );

    let mut rec = RecordingDispatcher { calls: Vec::new() };
    let stats = e
        .render(RenderTarget::Mixed { dispatcher: &mut rec })
        .expect("mixed render");

    assert_eq!(rec.calls.len(), 3);
    // RegionId order: 1, 3, 5
    assert_eq!(rec.calls[0].0, RegionId(1));
    assert_eq!(rec.calls[1].0, RegionId(3));
    assert_eq!(rec.calls[2].0, RegionId(5));

    // Hints forwarded verbatim.
    assert_eq!(rec.calls[0].2, BackendHint::Cpu);
    assert_eq!(rec.calls[1].2, BackendHint::FullGpu);
    assert_eq!(rec.calls[2].2, BackendHint::Cpu);

    // After successful render dirty must be cleared → needs_paint None
    assert!(e.needs_paint().is_none());
    assert_eq!(stats.regions_total, 3);
}

#[test]
fn mixed_backend_set_region_backend_hint_post_upsert() {
    let mut e = UrxEngine::new_mixed(800, 600);
    e.upsert_region(
        RegionId(10), Scene::new(), rect(0.0, 0.0, 100.0, 100.0),
        RenderCadence::Static,
    );
    // Default hint = Inherit
    assert_eq!(e.region_backend_hint(RegionId(10)), BackendHint::Inherit);

    e.set_region_backend_hint(RegionId(10), BackendHint::Hybrid);
    assert_eq!(e.region_backend_hint(RegionId(10)), BackendHint::Hybrid);

    let mut rec = RecordingDispatcher { calls: Vec::new() };
    e.render(RenderTarget::Mixed { dispatcher: &mut rec }).unwrap();
    assert_eq!(rec.calls[0].2, BackendHint::Hybrid);
}

#[test]
fn mixed_backend_rejects_non_mixed_target() {
    // Constructed as CPU — Mixed target must mismatch.
    let mut e = UrxEngine::new_cpu(100, 100);
    let mut rec = RecordingDispatcher { calls: Vec::new() };
    let r = e.render(RenderTarget::Mixed { dispatcher: &mut rec });
    assert!(r.is_err(), "Mixed target on CPU engine must error");
}

#[test]
fn regions_for_mix_iterator_returns_in_id_order() {
    let mut e = UrxEngine::new_mixed(100, 100);
    e.upsert_region_with_hint(
        RegionId(20), Scene::new(), rect(0.0, 0.0, 10.0, 10.0),
        RenderCadence::Static, BackendHint::FullGpu,
    );
    e.upsert_region_with_hint(
        RegionId(5), Scene::new(), rect(0.0, 0.0, 20.0, 20.0),
        RenderCadence::HighHz, BackendHint::Cpu,
    );
    let order: Vec<RegionId> = e.regions_for_mix().map(|t| t.0).collect();
    assert_eq!(order, vec![RegionId(5), RegionId(20)]);
}
