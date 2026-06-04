//! Wave 9b — RegionMixer routing tests.

use std::cell::RefCell;
use std::rc::Rc;
use uzor_urx_core::math::Rect;
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_engine::{BackendHint, RenderCadence, RenderTarget, UrxEngine};
use uzor_urx_region_mixer::RegionMixer;

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x0: x, y0: y, x1: x + w, y1: y + h }
}

#[test]
fn mixer_routes_each_hint_to_its_callback() {
    let mut e = UrxEngine::new_mixed(800, 600);
    e.upsert_region_with_hint(RegionId(1), Scene::new(), rect(0.0, 0.0, 100.0, 100.0), RenderCadence::Static, BackendHint::Cpu);
    e.upsert_region_with_hint(RegionId(2), Scene::new(), rect(100.0, 0.0, 100.0, 100.0), RenderCadence::HighHz, BackendHint::FullGpu);
    e.upsert_region_with_hint(RegionId(3), Scene::new(), rect(0.0, 100.0, 100.0, 100.0), RenderCadence::Static, BackendHint::Hybrid);

    let cpu_count = Rc::new(RefCell::new(0u32));
    let gpu_count = Rc::new(RefCell::new(0u32));
    let hyb_count = Rc::new(RefCell::new(0u32));
    let cpu_c = cpu_count.clone();
    let gpu_c = gpu_count.clone();
    let hyb_c = hyb_count.clone();

    let mut mixer = RegionMixer::new()
        .on_cpu(move |_, _, _| { *cpu_c.borrow_mut() += 1; })
        .on_full_gpu(move |_, _, _| { *gpu_c.borrow_mut() += 1; })
        .on_hybrid(move |_, _, _| { *hyb_c.borrow_mut() += 1; });

    mixer.begin_frame();
    let _stats = e.render(RenderTarget::Mixed { dispatcher: &mut mixer }).unwrap();
    assert_eq!(*cpu_count.borrow(), 1);
    assert_eq!(*gpu_count.borrow(), 1);
    assert_eq!(*hyb_count.borrow(), 1);
    assert_eq!(mixer.records().len(), 3);

    let counts = mixer.counts_by_hint();
    assert_eq!(counts.iter().find(|(h, _)| *h == BackendHint::Cpu).unwrap().1, 1);
    assert_eq!(counts.iter().find(|(h, _)| *h == BackendHint::FullGpu).unwrap().1, 1);
    assert_eq!(counts.iter().find(|(h, _)| *h == BackendHint::Hybrid).unwrap().1, 1);
}

#[test]
fn fallback_catches_unrouted_hints() {
    let mut e = UrxEngine::new_mixed(640, 480);
    e.upsert_region_with_hint(RegionId(10), Scene::new(), rect(0.0, 0.0, 50.0, 50.0), RenderCadence::Static, BackendHint::Cpu);
    e.upsert_region_with_hint(RegionId(11), Scene::new(), rect(50.0, 0.0, 50.0, 50.0), RenderCadence::Static, BackendHint::FullGpu);

    let fb_count = Rc::new(RefCell::new(0u32));
    let fb_c = fb_count.clone();
    let mut mixer = RegionMixer::new()
        .fallback(move |_, _, _| { *fb_c.borrow_mut() += 1; });
    mixer.begin_frame();
    e.render(RenderTarget::Mixed { dispatcher: &mut mixer }).unwrap();
    assert_eq!(*fb_count.borrow(), 2);
}

#[test]
fn lenient_silently_drops_unrouted_hints() {
    let mut e = UrxEngine::new_mixed(640, 480);
    e.upsert_region_with_hint(RegionId(1), Scene::new(), rect(0.0, 0.0, 50.0, 50.0), RenderCadence::Static, BackendHint::Cpu);

    let mut mixer = RegionMixer::new().lenient();
    mixer.begin_frame();
    e.render(RenderTarget::Mixed { dispatcher: &mut mixer }).unwrap();
    assert_eq!(mixer.records().len(), 1);
}

#[test]
#[should_panic(expected = "RegionMixer: no callback")]
fn strict_panics_on_unrouted_hints() {
    let mut e = UrxEngine::new_mixed(640, 480);
    e.upsert_region_with_hint(RegionId(1), Scene::new(), rect(0.0, 0.0, 50.0, 50.0), RenderCadence::Static, BackendHint::FullGpu);
    let mut mixer = RegionMixer::new();
    mixer.begin_frame();
    e.render(RenderTarget::Mixed { dispatcher: &mut mixer }).unwrap();
}
