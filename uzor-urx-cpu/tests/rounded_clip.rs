//! Rounded clip tests.

use kurbo::RoundedRect as KRRect;
use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn s() -> CpuBackend { CpuBackend::new() }

#[test]
fn rounded_clip_excludes_corners() {
    // 40×40 rounded rect with 10px radius corners. Fill green.
    // Corner pixel (0,0) should be CLIPPED (alpha < 50);
    // center should be FULLY filled.
    let mut p = Pixmap::new(40, 40);
    let rr = KRRect::new(0.0, 0.0, 40.0, 40.0, 10.0);
    let mut scene = Scene::new();
    scene.push(DrawCommand::PushClipRoundedRect {
        rect: rr,
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 40.0, 40.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 255, 0, 255)),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip);
    s().render(&scene, &mut p).unwrap();

    // Corner — outside the rounded mask, should be ~0 alpha.
    let corner = p.get_pixel(1, 1);
    assert!(corner[3] < 50, "rounded corner must be clipped, got {:?}", corner);
    // Center — fully inside.
    let center = p.get_pixel(20, 20);
    assert_eq!(center, [0, 255, 0, 255], "center fully filled");
    // Edge midpoint — fully filled (radius doesn't bite edge midpoint).
    let edge = p.get_pixel(20, 1);
    assert!(edge[1] > 200, "edge midpoint filled, got {:?}", edge);
}

#[test]
fn rounded_clip_pop_restores() {
    // Push rounded clip, draw → clipped. Pop. Draw again → unclipped.
    let mut p = Pixmap::new(40, 40);
    let rr = KRRect::new(10.0, 10.0, 30.0, 30.0, 5.0);
    let mut scene = Scene::new();
    // Inside clip — fill red, see clipped output.
    scene.push(DrawCommand::PushClipRoundedRect {
        rect: rr,
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(10.0, 10.0, 30.0, 30.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip);
    // After pop — draw a small green square outside the clip's prior area.
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 5.0, 5.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 255, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    // The center of the rounded clip — fully red.
    let center = p.get_pixel(20, 20);
    assert_eq!(center, [255, 0, 0, 255], "center fully filled");
    // The corner OUTSIDE the rounded clip — must be 0 (clipped).
    // Pixel (10, 10) is at corner of bounds, distance to corner-circle-
    // center (15, 15) = sqrt(50) ≈ 7.07, radius=5 → SDF +2.07 → outside.
    let corner = p.get_pixel(10, 10);
    assert!(corner[3] < 30, "corner clipped, got {:?}", corner);
    // The green square outside — fully filled.
    let post = p.get_pixel(2, 2);
    assert_eq!(post[1], 255, "post-pop draw renders, got {:?}", post);
}

#[test]
fn rounded_clip_mask_is_cached() {
    // Two identical rounded clip pushes should hit the same mask.
    // We can't directly observe the cache; instead verify two identical
    // scenes produce identical pixels (regression-grade).
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let rr = KRRect::new(0.0, 0.0, 50.0, 50.0, 8.0);
    let make_scene = || {
        let mut sc = Scene::new();
        sc.push(DrawCommand::PushClipRoundedRect { rect: rr, transform: Affine::IDENTITY });
        sc.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
            radii: None,
            brush: Brush::Solid(Color::rgba8(50, 100, 200, 255)),
            transform: Affine::IDENTITY,
        });
        sc.push(DrawCommand::PopClip);
        sc
    };
    let mut p1 = Pixmap::new(50, 50);
    let mut p2 = Pixmap::new(50, 50);
    s().render(&make_scene(), &mut p1).unwrap();
    s().render(&make_scene(), &mut p2).unwrap();
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    p1.pixels().hash(&mut h1);
    p2.pixels().hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish(), "same rounded clip → identical pixels");
}
