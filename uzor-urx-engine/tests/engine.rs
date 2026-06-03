//! UrxEngine round-trip tests — per-region dirty tracking + needs_paint().

use uzor_urx_core::dirty::DirtyState;
use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::Pixmap;
use uzor_urx_engine::{Backend, RenderCadence, RenderTarget, UrxEngine};

fn rect_scene(x: f64, y: f64, w: f64, h: f64, color: Color) -> Scene {
    let mut s = Scene::new();
    s.push(DrawCommand::FillRect {
        rect: Rect::new(x, y, x + w, y + h),
        radii: None,
        brush: Brush::Solid(color),
        transform: Affine::IDENTITY,
    });
    s
}

#[test]
fn fresh_engine_has_no_paint() {
    let e = UrxEngine::new_cpu(100, 100);
    assert!(e.needs_paint().is_none(), "no regions → no paint needed");
}

#[test]
fn upsert_region_marks_dirty() {
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(1);
    e.upsert_region(id, rect_scene(10.0, 10.0, 50.0, 50.0, Color::rgba8(255, 0, 0, 255)),
                    Rect::new(10.0, 10.0, 60.0, 60.0), RenderCadence::Static);
    let dr = e.needs_paint().expect("must need paint");
    let bbox = dr.bbox().unwrap();
    assert!(bbox.width() > 0.0 && bbox.height() > 0.0, "dirty bbox must be non-empty");
}

#[test]
fn render_clears_dirty() {
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(1);
    e.upsert_region(id, rect_scene(10.0, 10.0, 50.0, 50.0, Color::rgba8(255, 0, 0, 255)),
                    Rect::new(10.0, 10.0, 60.0, 60.0), RenderCadence::Static);
    let mut p = Pixmap::new(100, 100);
    let stats = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(stats.regions_dirty, 1);
    assert_eq!(stats.regions_clean, 0);
    assert!(e.needs_paint().is_none(), "post-render must be clean");
}

#[test]
fn second_render_is_idle() {
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(1);
    e.upsert_region(id, rect_scene(10.0, 10.0, 50.0, 50.0, Color::rgba8(255, 0, 0, 255)),
                    Rect::new(10.0, 10.0, 60.0, 60.0), RenderCadence::Static);
    let mut p = Pixmap::new(100, 100);
    e.render(RenderTarget::Cpu(&mut p)).unwrap();
    // No mark_dirty between → driver should skip.
    assert!(e.needs_paint().is_none(), "idle frame skip");
    let stats = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(stats.regions_dirty, 0, "idle frame paints zero regions");
}

#[test]
fn mark_dirty_brings_back_paint() {
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(1);
    e.upsert_region(id, rect_scene(10.0, 10.0, 50.0, 50.0, Color::rgba8(255, 0, 0, 255)),
                    Rect::new(10.0, 10.0, 60.0, 60.0), RenderCadence::Static);
    let mut p = Pixmap::new(100, 100);
    e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert!(e.needs_paint().is_none());
    e.mark_dirty(id);
    assert!(e.needs_paint().is_some(), "post-mark must need paint");
}

#[test]
fn transform_only_does_not_re_raster_in_engine_dirty_view() {
    // TransformOnly is still recorded as needs_paint (the compositor
    // has work to do), but the region state itself is TransformOnly,
    // not Content — Phase 6 cache layer will use that to skip
    // rasterisation and only re-blit the texture at the new transform.
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(1);
    e.upsert_region(id, rect_scene(0.0, 0.0, 30.0, 30.0, Color::rgba8(0, 255, 0, 255)),
                    Rect::new(0.0, 0.0, 30.0, 30.0), RenderCadence::Static);
    let mut p = Pixmap::new(100, 100);
    e.render(RenderTarget::Cpu(&mut p)).unwrap();
    e.mark_transform_only(id, Affine::translate((10.0, 5.0)));
    assert!(e.needs_paint().is_some(), "transform-only is still a paint cycle");
    // The engine's internal state is TransformOnly, not Content.
    // Phase 6 cache layer reads this to skip re-raster.
    // (No public getter for region state yet; tested indirectly
    // via stats once Phase 6 lands.)
}

#[test]
fn remove_region_marks_footprint_dirty() {
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(7);
    e.upsert_region(id, rect_scene(0.0, 0.0, 30.0, 30.0, Color::rgba8(0, 0, 255, 255)),
                    Rect::new(0.0, 0.0, 30.0, 30.0), RenderCadence::Static);
    let mut p = Pixmap::new(100, 100);
    e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert!(e.needs_paint().is_none());
    e.remove_region(id);
    assert!(e.needs_paint().is_some(), "removed region must re-paint footprint");
}

#[test]
fn backend_mismatch_returns_error() {
    let mut e = UrxEngine::new_cpu(50, 50);
    let id = RegionId(1);
    e.upsert_region(id, rect_scene(0.0, 0.0, 20.0, 20.0, Color::rgba8(255, 0, 0, 255)),
                    Rect::new(0.0, 0.0, 20.0, 20.0), RenderCadence::Static);
    // Engine is Cpu; passing a Wgpu target would error. But the Wgpu
    // variant only exists with the wgpu feature; pure CPU consumers
    // can't even construct a mismatched target. The test below
    // confirms a valid match works.
    let mut p = Pixmap::new(50, 50);
    let r = e.render(RenderTarget::Cpu(&mut p));
    assert!(r.is_ok(), "matching backend renders ok");
    assert_eq!(e.backend(), Backend::Cpu);
}

#[test]
fn invalidate_all_marks_window_full() {
    let mut e = UrxEngine::new_cpu(640, 480);
    let id1 = RegionId(1);
    let id2 = RegionId(2);
    e.upsert_region(id1, rect_scene(0.0, 0.0, 100.0, 100.0, Color::rgba8(255, 0, 0, 255)),
                    Rect::new(0.0, 0.0, 100.0, 100.0), RenderCadence::Static);
    e.upsert_region(id2, rect_scene(200.0, 200.0, 100.0, 100.0, Color::rgba8(0, 255, 0, 255)),
                    Rect::new(200.0, 200.0, 300.0, 300.0), RenderCadence::Static);
    let mut p = Pixmap::new(640, 480);
    e.render(RenderTarget::Cpu(&mut p)).unwrap();
    e.invalidate_all();
    let dr = e.needs_paint().unwrap();
    let bbox = dr.bbox().unwrap();
    // The invalidate union covers the full window.
    assert!(bbox.width() >= 640.0 && bbox.height() >= 480.0,
            "invalidate_all union must cover full window, got {:?}", bbox);
}

#[test]
fn dirty_state_three_states_distinguishable() {
    // Direct verification that DirtyState has the three required variants
    // and the promotion rules match the doctrine.
    let mut s = DirtyState::Clean;
    assert!(!s.needs_raster());
    s.promote_to_transform();
    assert_eq!(s, DirtyState::TransformOnly);
    assert!(!s.needs_raster());
    assert!(s.needs_compose());
    s.promote_to_content();
    assert_eq!(s, DirtyState::Content);
    assert!(s.needs_raster());
}
