//! Retained cache (Phase 6) — round-trip + LRU eviction.

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::Pixmap;
use uzor_urx_engine::{RenderCadence, RenderTarget, UrxEngine};

fn rect_scene(x: f64, y: f64, w: f64, h: f64, c: Color) -> Scene {
    let mut s = Scene::new();
    s.push(DrawCommand::FillRect {
        rect: Rect::new(x, y, x + w, y + h), radii: None,
        brush: Brush::Solid(c), transform: Affine::IDENTITY,
    });
    s
}

#[test]
fn first_render_is_miss_second_is_hit_after_transform_only() {
    let mut e = UrxEngine::new_cpu(200, 200);
    let id = RegionId(1);
    e.upsert_region(id,
        rect_scene(20.0, 20.0, 60.0, 60.0, Color::rgba8(255, 0, 0, 255)),
        Rect::new(20.0, 20.0, 80.0, 80.0),
        RenderCadence::Static);

    let mut p = Pixmap::new(200, 200);

    let s1 = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(s1.cache_misses, 1, "first render must be a miss");
    assert_eq!(s1.cache_hits, 0);
    assert_eq!(e.cache_count(), 1, "cache should now hold 1 entry");

    // Transform-only invalidation — should hit the cache, no re-raster.
    e.mark_transform_only(id, Affine::translate((50.0, 30.0)));
    let s2 = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(s2.cache_hits, 1, "second render (transform only) must hit");
    assert_eq!(s2.cache_misses, 0);
}

#[test]
fn high_hz_region_bypasses_cache() {
    let mut e = UrxEngine::new_cpu(200, 200);
    let id = RegionId(1);
    e.upsert_region(id,
        rect_scene(0.0, 0.0, 20.0, 20.0, Color::rgba8(0, 255, 0, 255)),
        Rect::new(0.0, 0.0, 20.0, 20.0),
        RenderCadence::HighHz);

    let mut p = Pixmap::new(200, 200);
    let s = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(s.cache_hits,   0, "HighHz must not hit cache");
    assert_eq!(s.cache_misses, 0, "HighHz must not store cache");
    assert_eq!(e.cache_count(), 0, "HighHz region never enters cache");
}

#[test]
fn cache_eviction_when_over_budget() {
    let mut e = UrxEngine::new_cpu(2000, 2000);
    // Tight budget — anything bigger than 1 small region triggers
    // eviction of the oldest.
    e.set_cache_budget(20_000); // ~110×45 pixels worth

    let mut p = Pixmap::new(2000, 2000);
    for i in 0..5u64 {
        let id = RegionId(i);
        let x = (i as f64) * 200.0;
        e.upsert_region(id,
            rect_scene(x, 0.0, 100.0, 100.0, Color::rgba8(255, 255, 255, 255)),
            Rect::new(x, 0.0, x + 100.0, 100.0),
            RenderCadence::Static);
        e.render(RenderTarget::Cpu(&mut p)).unwrap();
    }
    // After inserting 5 × 100×100 = 200KB > 20KB budget; LRU evicts.
    assert!(e.cache_count() < 5,
            "cache must have evicted oldest, got {}", e.cache_count());
    assert!(e.cache_bytes() <= 20_000,
            "cache total bytes must respect budget, got {}", e.cache_bytes());
}

#[test]
fn content_dirty_re_rasters_and_replaces_cache() {
    let mut e = UrxEngine::new_cpu(100, 100);
    let id = RegionId(1);
    e.upsert_region(id,
        rect_scene(10.0, 10.0, 30.0, 30.0, Color::rgba8(255, 0, 0, 255)),
        Rect::new(10.0, 10.0, 40.0, 40.0),
        RenderCadence::Static);

    let mut p = Pixmap::new(100, 100);
    let s1 = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(s1.cache_misses, 1, "first paint = miss");

    // Now upsert a NEW scene for the same region — should re-raster
    // (cache miss again) and replace the cache entry.
    e.upsert_region(id,
        rect_scene(10.0, 10.0, 30.0, 30.0, Color::rgba8(0, 255, 0, 255)),
        Rect::new(10.0, 10.0, 40.0, 40.0),
        RenderCadence::Static);
    let s2 = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert_eq!(s2.cache_misses, 1, "content change = miss");
    assert_eq!(s2.cache_hits,   0);
    assert_eq!(e.cache_count(), 1, "cache count stays 1 (entry replaced)");
}

#[test]
fn invalidate_all_clears_cache() {
    let mut e = UrxEngine::new_cpu(100, 100);
    for i in 0..3u64 {
        let id = RegionId(i);
        e.upsert_region(id,
            rect_scene((i as f64) * 20.0, 0.0, 18.0, 18.0,
                       Color::rgba8(255, 0, 0, 255)),
            Rect::new((i as f64) * 20.0, 0.0, (i as f64) * 20.0 + 18.0, 18.0),
            RenderCadence::Static);
    }
    let mut p = Pixmap::new(100, 100);
    e.render(RenderTarget::Cpu(&mut p)).unwrap();
    assert!(e.cache_count() > 0);

    e.invalidate_all();
    assert_eq!(e.cache_count(), 0, "invalidate_all must purge cache");
    assert_eq!(e.cache_bytes(), 0);
}
