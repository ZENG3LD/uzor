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
fn fill_path_concave_star_no_holes_inside() {
    // 5-point star — concave, with many edges crossing each scanline.
    // Catches AET duplicate-edge bugs: a row near the middle has ALL
    // edges active and any double-count corrupts the winding parity.
    let mut p = Pixmap::new(120, 120);
    let cx = 60.0_f64;
    let cy = 60.0_f64;
    let r_out = 50.0_f64;
    let r_in  = 22.0_f64;
    let mut path = BezPath::new();
    for i in 0..10 {
        let ang = (i as f64) * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2;
        let r = if i % 2 == 0 { r_out } else { r_in };
        let x = cx + r * ang.cos();
        let y = cy + r * ang.sin();
        if i == 0 { path.move_to((x, y)); } else { path.line_to((x, y)); }
    }
    path.close_path();
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillPath {
        path,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(255, 200, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Star centre — must be filled (NonZero winding = -2 on inner core
    // for a CCW star path; abs != 0 → fill).
    let c = p.get_pixel(60, 60);
    assert!(c[3] > 240, "star centre filled, got {:?}", c);
    // Sample several radial pixels on the spokes — they MUST be filled,
    // a duplicate-edge bug would flip parity here.
    for ang_step in 0..5 {
        let ang = (ang_step as f64) * 2.0 * std::f64::consts::PI / 5.0 - std::f64::consts::FRAC_PI_2;
        let r = 30.0;
        let px = (cx + r * ang.cos()) as u32;
        let py = (cy + r * ang.sin()) as u32;
        let c = p.get_pixel(px, py);
        assert!(c[3] > 200, "spoke pixel ({}, {}) must be filled, got {:?}", px, py, c);
    }
    // Sample dead-zone between spokes far enough out — must be EMPTY.
    let ang = std::f64::consts::PI / 5.0; // halfway between spoke 0 and 1
    let r = 45.0;
    let px = (cx + r * ang.cos()) as u32;
    let py = (cy + r * ang.sin()) as u32;
    let c = p.get_pixel(px, py);
    assert!(c[3] < 30, "outside-star pixel ({}, {}) must be empty, got {:?}", px, py, c);
}

#[test]
fn fill_path_three_disjoint_subpaths_nonzero() {
    // Three disjoint triangles in ONE BezPath, NonZero winding.
    // Catches the bug where AET dedup-by-float fails on multi-subpath
    // and corrupts later subpath rows.
    let mut p = Pixmap::new(120, 60);
    let mut path = BezPath::new();
    for offset in [0.0_f64, 40.0, 80.0] {
        path.move_to((offset + 5.0, 50.0));
        path.line_to((offset + 35.0, 50.0));
        path.line_to((offset + 20.0, 10.0));
        path.close_path();
    }
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillPath {
        path,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(0, 200, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Each triangle's centroid must be filled.
    for cx in [20_u32, 60, 100] {
        let c = p.get_pixel(cx, 36);
        assert!(c[3] > 200, "triangle centroid ({}, 36) must be filled, got {:?}", cx, c);
    }
    // Gap between triangles (x=37..40) must be empty.
    for gap_x in [38_u32, 78] {
        let c = p.get_pixel(gap_x, 36);
        assert!(c[3] < 30, "gap pixel ({}, 36) must be empty, got {:?}", gap_x, c);
    }
}

#[test]
fn fill_path_scale_transform_stays_subpixel() {
    // Path defined at native size, rendered with 4× scale. With the
    // pre-fix tolerance bug the 4× scale would land at effective 1.0px
    // tolerance → visible polygon-edge stair-stepping. After the fix
    // tolerance is divided by max-scale → still sub-pixel post-transform.
    let mut p = Pixmap::new(80, 80);
    // A small triangle that becomes a big one under 4× scale.
    let tri = make_triangle(1.0, 1.0, 9.0, 1.0, 5.0, 9.0);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillPath {
        path: tri,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(0, 0, 200, 255)),
        transform: Affine::scale(4.0),
    });
    s().render(&scene, &mut p).unwrap();
    // Centroid of the SCALED triangle = (4*5, 4*4) = (20, 16). Filled.
    let c = p.get_pixel(20, 16);
    assert!(c[3] > 200, "scaled centroid filled, got {:?}", c);
    // A pixel just outside the scaled triangle bottom (y=37) must be
    // empty — confirms scanline range still respects screen-space bbox.
    let c = p.get_pixel(20, 38);
    assert!(c[3] < 30, "outside-bbox pixel must be empty, got {:?}", c);
}

#[test]
fn fill_path_empty_path_does_not_panic() {
    let mut p = Pixmap::new(8, 8);
    let path = BezPath::new();
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillPath {
        path,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    assert_eq!(p.get_pixel(4, 4), [0, 0, 0, 0]);
}

#[test]
fn fill_path_horizontal_sliver_renders_or_skips_no_artifacts() {
    // Path with very thin (height < 0.5px) sliver. Must either render
    // something faint or render nothing — but MUST NOT leave junk
    // outside the slim bbox.
    let mut p = Pixmap::new(40, 40);
    let mut path = BezPath::new();
    path.move_to((5.0, 20.0));
    path.line_to((35.0, 20.0));
    path.line_to((35.0, 20.3));
    path.line_to((5.0, 20.3));
    path.close_path();
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillPath {
        path,
        rule: FillRule::NonZero,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // No pixel rows above/below the sliver must be touched.
    for y in [0_u32, 5, 15, 25, 35] {
        for x in [0_u32, 10, 20, 30] {
            let c = p.get_pixel(x, y);
            assert!(c[3] < 30, "pixel ({}, {}) must be untouched, got {:?}", x, y, c);
        }
    }
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
