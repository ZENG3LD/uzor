//! Path fill + stroke tests.

use kurbo::BezPath;
use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, FillRule, Scene, Stroke};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn make_triangle(x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((x0, y0));
    p.line_to((x1, y1));
    p.line_to((x2, y2));
    p.close_path();
    p
}

fn make_rect_path(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((x, y));
    p.line_to((x + w, y));
    p.line_to((x + w, y + h));
    p.line_to((x, y + h));
    p.close_path();
    p
}

fn s() -> CpuBackend { CpuBackend::new() }

#[test]
fn fill_path_triangle_interior() {
    let mut p = Pixmap::new(40, 40);
    let mut scene = Scene::new();
    let tri = make_triangle(5.0, 5.0, 35.0, 5.0, 20.0, 35.0);
    scene.push(DrawCommand::FillPath {
        path: tri,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Center of triangle — should be opaque red.
    let c = p.get_pixel(20, 15);
    assert!(c[0] > 200 && c[3] > 200, "center filled, got {:?}", c);
    // Outside triangle — untouched.
    assert_eq!(p.get_pixel(2, 2), [0, 0, 0, 0]);
    assert_eq!(p.get_pixel(35, 35), [0, 0, 0, 0]);
}

#[test]
fn fill_path_rect_matches_fill_rect() {
    // FillPath(rect outline) should produce the same interior pixels as
    // FillRect — confirms the scanline fill matches the analytic rect fill
    // on simple shapes (not pixel-identical due to AA at edges, but
    // interior fully covered).
    let mut p_path = Pixmap::new(32, 32);
    let mut p_rect = Pixmap::new(32, 32);

    let mut sp = Scene::new();
    sp.push(DrawCommand::FillPath {
        path: make_rect_path(8.0, 8.0, 16.0, 16.0),
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(0, 0, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&sp, &mut p_path).unwrap();

    let mut sr = Scene::new();
    sr.push(DrawCommand::FillRect {
        rect: Rect::new(8.0, 8.0, 24.0, 24.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 0, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&sr, &mut p_rect).unwrap();

    // Center pixel — both must be solid blue.
    assert_eq!(p_path.get_pixel(16, 16), [0, 0, 255, 255]);
    assert_eq!(p_rect.get_pixel(16, 16), [0, 0, 255, 255]);
    // Far outside — both untouched.
    assert_eq!(p_path.get_pixel(2, 2), [0, 0, 0, 0]);
    assert_eq!(p_rect.get_pixel(2, 2), [0, 0, 0, 0]);
}

#[test]
fn fill_path_even_odd_creates_hole() {
    // Two concentric squares with EvenOdd → outer-minus-inner ring.
    let mut p = Pixmap::new(40, 40);
    let mut path = BezPath::new();
    // Outer square (5..35, 5..35)
    path.move_to((5.0, 5.0));
    path.line_to((35.0, 5.0));
    path.line_to((35.0, 35.0));
    path.line_to((5.0, 35.0));
    path.close_path();
    // Inner square (15..25, 15..25)
    path.move_to((15.0, 15.0));
    path.line_to((25.0, 15.0));
    path.line_to((25.0, 25.0));
    path.line_to((15.0, 25.0));
    path.close_path();

    let mut scene = Scene::new();
    scene.push(DrawCommand::FillPath {
        path,
        rule: FillRule::EvenOdd,
        brush: Brush::Solid(Color::rgba8(0, 255, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    // Ring zone — filled.
    let c = p.get_pixel(10, 20);
    assert!(c[1] > 200, "ring zone filled, got {:?}", c);
    // Center of hole — NOT filled.
    let c = p.get_pixel(20, 20);
    assert_eq!(c, [0, 0, 0, 0], "even-odd hole, got {:?}", c);
}

#[test]
fn stroke_path_traces_outline() {
    let mut p = Pixmap::new(40, 40);
    let mut scene = Scene::new();
    let tri = make_triangle(5.0, 5.0, 35.0, 5.0, 20.0, 35.0);
    scene.push(DrawCommand::StrokePath {
        path: tri,
        stroke: Stroke { width: 2.0, ..Stroke::default() },
        brush: Brush::Solid(Color::rgba8(0, 0, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Edge midpoint — should have blue.
    let c = p.get_pixel(20, 5);
    assert!(c[2] > 100 || c[3] > 100, "top edge midpoint, got {:?}", c);
    // Interior of triangle — should NOT be filled (only outline).
    let c = p.get_pixel(20, 20);
    assert_eq!(c, [0, 0, 0, 0], "interior must be empty for stroke, got {:?}", c);
}

#[test]
fn fill_path_with_transform_translates() {
    let mut p1 = Pixmap::new(40, 40);
    let mut p2 = Pixmap::new(40, 40);
    let tri = make_triangle(0.0, 0.0, 10.0, 0.0, 5.0, 10.0);

    // Render at translation (5, 5).
    let mut s1 = Scene::new();
    s1.push(DrawCommand::FillPath {
        path: tri.clone(),
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::translate((5.0, 5.0)),
    });
    s().render(&s1, &mut p1).unwrap();

    // Render at translation (15, 15).
    let mut s2 = Scene::new();
    s2.push(DrawCommand::FillPath {
        path: tri,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::translate((15.0, 15.0)),
    });
    s().render(&s2, &mut p2).unwrap();

    // p1 should have pixels around (10, 5..15); p2 around (20, 15..25).
    assert!(p1.get_pixel(10, 5)[3] > 100,  "p1 should have pixels at translation (5,5)");
    assert_eq!(p1.get_pixel(20, 20), [0, 0, 0, 0], "p1 should NOT have pixels where p2 does");
    assert!(p2.get_pixel(20, 20)[3] > 100, "p2 should have pixels at translation (15,15)");
    assert_eq!(p2.get_pixel(10, 5), [0, 0, 0, 0], "p2 should NOT have pixels where p1 does");
}
