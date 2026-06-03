//! Smoke + correctness tests for the CPU rasteriser.
//!
//! Test strategy:
//! - Tiny pixmaps (24x24) so a wrong pixel is easy to spot
//! - Aligned rects → expected pixel values exactly
//! - AA edges → tolerant assertions (coverage > X)
//! - Blend stack → check src-over math by reading pixels post-paint

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn s() -> CpuBackend { CpuBackend::new() }

#[test]
fn aligned_fill_rect_solid_inside() {
    // 24x24 transparent black background, fill [4..20, 4..20] opaque red.
    let mut p = Pixmap::new(24, 24);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(4.0, 4.0, 20.0, 20.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Center pixel — fully covered, opaque red.
    assert_eq!(p.get_pixel(12, 12), [255, 0, 0, 255], "center must be opaque red");
    // Outside — unchanged transparent.
    assert_eq!(p.get_pixel(2, 2), [0, 0, 0, 0], "outside must be unchanged");
}

#[test]
fn fill_rect_half_alpha_premul() {
    // Fill with 50% alpha red over transparent — premul should be
    // (~128, 0, 0, 128).
    let mut p = Pixmap::new(8, 8);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 8.0, 8.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 128)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let c = p.get_pixel(4, 4);
    // r/a in premultiplied form. r = 255*128/255 ≈ 128 (within +/-1 for rounding).
    assert!((c[0] as i32 - 128).abs() <= 1, "premul r ~128 got {}", c[0]);
    assert_eq!(c[3], 128, "alpha must be 128");
}

#[test]
fn aa_edges_have_partial_coverage() {
    // Fill rect with FRACTIONAL edges → boundary pixels should have
    // alpha between 0 and 255 (analytic AA).
    let mut p = Pixmap::new(16, 16);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(2.5, 2.5, 13.5, 13.5),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Edge pixel at x=2 (covers [2.0, 3.0], rect starts at 2.5)
    // → horizontal coverage = 0.5, vertical (y=8 inside) = 1.0,
    // combined = 0.5 → alpha ~127.
    let c = p.get_pixel(2, 8);
    assert!(c[3] > 100 && c[3] < 160, "edge alpha should be ~127, got {}", c[3]);
    // Interior pixel = fully covered.
    let c = p.get_pixel(8, 8);
    assert_eq!(c[3], 255, "interior must be fully covered");
}

#[test]
fn clip_rect_excludes_outside() {
    // Push a small clip, draw a big rect — only the clip area paints.
    let mut p = Pixmap::new(20, 20);
    let mut scene = Scene::new();
    scene.push(DrawCommand::PushClipRect {
        rect: Rect::new(5.0, 5.0, 10.0, 10.0),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 20.0, 20.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 255, 0, 255)),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip);
    s().render(&scene, &mut p).unwrap();
    // Inside clip → green.
    assert_eq!(p.get_pixel(7, 7), [0, 255, 0, 255]);
    // Outside clip → unchanged.
    assert_eq!(p.get_pixel(2, 2), [0, 0, 0, 0]);
    assert_eq!(p.get_pixel(15, 15), [0, 0, 0, 0]);
}

#[test]
fn stroke_rect_draws_4_bands() {
    // Draw a stroke rect. Interior should be untouched, border filled.
    let mut p = Pixmap::new(20, 20);
    let mut scene = Scene::new();
    scene.push(DrawCommand::StrokeRect {
        rect: Rect::new(4.0, 4.0, 16.0, 16.0),
        radii: None,
        stroke: Stroke { width: 2.0, ..Stroke::default() },
        brush: Brush::Solid(Color::rgba8(0, 0, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Border center @ y=4 → should be on the stroke band, has color.
    let c = p.get_pixel(10, 4);
    assert!(c[2] > 0 && c[3] > 0, "top border must have blue, got {:?}", c);
    // Interior (well inside) → untouched transparent.
    let c = p.get_pixel(10, 10);
    assert_eq!(c, [0, 0, 0, 0], "interior must be untouched, got {:?}", c);
    // Far outside → untouched.
    let c = p.get_pixel(0, 0);
    assert_eq!(c, [0, 0, 0, 0]);
}

#[test]
fn line_horizontal_paints_pixels() {
    // Horizontal line at y=10, x=2..18, width=2 → ~2-pixel-tall band.
    let mut p = Pixmap::new(20, 20);
    let mut scene = Scene::new();
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 2.0, y: 10.0 },
        to:   Vec2 { x: 18.0, y: 10.0 },
        stroke: Stroke { width: 2.0, ..Stroke::default() },
        brush: Brush::Solid(Color::rgba8(0, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Pixel on the line → covered (cyan).
    let c = p.get_pixel(10, 10);
    assert!(c[1] > 200 && c[2] > 200 && c[3] > 200, "on-line pixel cyan, got {:?}", c);
    // Far off the line → transparent.
    assert_eq!(p.get_pixel(10, 5), [0, 0, 0, 0]);
}

#[test]
fn src_over_blend_red_then_blue() {
    // Fill red at full alpha then blue at 50% — should blend src-over.
    let mut p = Pixmap::new(8, 8);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 8.0, 8.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 8.0, 8.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 0, 255, 128)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Expected: src = (0, 0, 128, 128). inv_a = 127.
    // dst was (255, 0, 0, 255). After blend:
    //   r = 0   + 255 * 127 / 255 = 127
    //   g = 0   + 0 * 127 / 255   = 0
    //   b = 128 + 0 * 127 / 255   = 128
    //   a = 128 + 255 * 127 / 255 = 255
    let c = p.get_pixel(4, 4);
    assert!((c[0] as i32 - 127).abs() <= 1, "r ~127, got {}", c[0]);
    assert_eq!(c[1], 0);
    assert!((c[2] as i32 - 128).abs() <= 1, "b ~128, got {}", c[2]);
    assert!(c[3] >= 254, "a ~255, got {}", c[3]);
}
