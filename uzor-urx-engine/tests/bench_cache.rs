//! Phase 6 cache value demonstration — animate transform 100×, count
//! hits vs misses. The whole POINT of retained mode: animated
//! transform/opacity is composite-only, no re-raster.

use std::time::Instant;

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::Pixmap;
use uzor_urx_engine::{RenderCadence, RenderTarget, UrxEngine};

fn heavy_scene(x: f64, y: f64) -> Scene {
    // ~120-rect panel — heavier than a button but smaller than mlc chart.
    let mut s = Scene::new();
    // bg
    s.push(DrawCommand::FillRect {
        rect: Rect::new(x, y, x + 200.0, y + 150.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(33, 38, 45, 255)),
        transform: Affine::IDENTITY,
    });
    // grid
    for ry in 0..6 {
        for cx in 0..20 {
            let px = x + 4.0 + (cx as f64) * 10.0;
            let py = y + 4.0 + (ry as f64) * 24.0;
            s.push(DrawCommand::FillRect {
                rect: Rect::new(px, py, px + 8.0, py + 20.0),
                radii: None,
                brush: Brush::Solid(Color::rgba8(88, 166, 255,
                                                  ((cx + ry) as u8).wrapping_mul(20))),
                transform: Affine::IDENTITY,
            });
        }
    }
    s
}

#[test]
fn cache_makes_transform_animation_cheap() {
    let mut e = UrxEngine::new_cpu(640, 480);
    let id = RegionId(1);
    e.upsert_region(id,
        heavy_scene(20.0, 20.0),
        Rect::new(20.0, 20.0, 220.0, 170.0),
        RenderCadence::Static);

    let mut p = Pixmap::new(640, 480);

    // First frame — miss (rasterise + cache).
    let t0 = Instant::now();
    let s1 = e.render(RenderTarget::Cpu(&mut p)).unwrap();
    let cold_us = t0.elapsed().as_micros();
    assert_eq!(s1.cache_misses, 1);
    assert_eq!(s1.cache_hits, 0);

    // Now animate transform — 100 frames, each TransformOnly.
    // Every frame should be a cache HIT (cheap blit).
    let t_anim_start = Instant::now();
    for i in 0..100 {
        let dx = (i as f64) * 1.0;
        e.mark_transform_only(id, Affine::translate((dx, 0.0)));
        let s = e.render(RenderTarget::Cpu(&mut p)).unwrap();
        assert_eq!(s.cache_hits, 1, "frame {} should be a hit", i);
        assert_eq!(s.cache_misses, 0, "frame {} should be miss-free", i);
    }
    let anim_us = t_anim_start.elapsed().as_micros();
    let per_frame_us = anim_us / 100;

    eprintln!(
        "[bench_cache] cold-frame {}µs ; 100 transform-only frames {}µs ({}µs/frame avg)",
        cold_us, anim_us, per_frame_us,
    );

    // Animation frame should be CHEAPER than cold (cache hit vs raster).
    // On a small region the ratio isn't huge, but it should be < cold.
    // (Both numbers vary by machine; this is a sanity assertion.)
    assert!(per_frame_us < cold_us, "cached anim frame must beat cold raster");
}
