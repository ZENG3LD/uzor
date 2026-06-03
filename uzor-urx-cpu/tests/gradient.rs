//! Gradient brush tests — linear + radial.

use uzor_urx_core::math::{
    Affine, Brush, Color, ColorStop, ColorStops, Extend, Gradient, GradientKind, Point, Rect,
};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn s() -> CpuBackend { CpuBackend::new() }

fn make_linear(start: Point, end: Point, stops: Vec<ColorStop>) -> Brush {
    let mut g = Gradient::new_linear(start, end);
    g.stops = stops.into_iter().collect::<ColorStops>();
    g.extend = Extend::Pad;
    Brush::Gradient(g)
}

fn make_radial(center: Point, radius: f32, stops: Vec<ColorStop>) -> Brush {
    let mut g = Gradient::new_radial(center, radius);
    g.stops = stops.into_iter().collect::<ColorStops>();
    g.extend = Extend::Pad;
    Brush::Gradient(g)
}

#[test]
fn linear_red_to_blue_left_to_right() {
    // Horizontal gradient red→blue across 100px.
    let mut p = Pixmap::new(100, 20);
    let brush = make_linear(
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        vec![
            ColorStop { offset: 0.0, color: Color::rgba8(255, 0, 0, 255) },
            ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 255, 255) },
        ],
    );
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 20.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    // Left edge: pure red.
    let l = p.get_pixel(2, 10);
    assert!(l[0] > 200 && l[2] < 30, "left should be red, got {:?}", l);
    // Right edge: pure blue.
    let r = p.get_pixel(97, 10);
    assert!(r[2] > 200 && r[0] < 30, "right should be blue, got {:?}", r);
    // Middle: roughly equal R and B (sRGB-premul lerp).
    let m = p.get_pixel(50, 10);
    assert!(m[0] > 80 && m[2] > 80, "middle should mix RB, got {:?}", m);
}

#[test]
fn linear_pad_extends_outside() {
    // 50px-wide gradient inside a 100px rect. Outside [25..75] should
    // be pad-clamped to the end stops.
    let mut p = Pixmap::new(100, 20);
    let brush = make_linear(
        Point::new(25.0, 0.0),
        Point::new(75.0, 0.0),
        vec![
            ColorStop { offset: 0.0, color: Color::rgba8(0, 255, 0, 255) },   // start green
            ColorStop { offset: 1.0, color: Color::rgba8(255, 0, 255, 255) }, // end magenta
        ],
    );
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 20.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    // Far left (outside start) — pad to green.
    let l = p.get_pixel(5, 10);
    assert!(l[1] > 200 && l[0] < 30, "pad-left = green, got {:?}", l);
    // Far right (outside end) — pad to magenta.
    let r = p.get_pixel(95, 10);
    assert!(r[0] > 200 && r[2] > 200 && r[1] < 30, "pad-right = magenta, got {:?}", r);
}

#[test]
fn radial_center_is_inner_stop() {
    let mut p = Pixmap::new(60, 60);
    let brush = make_radial(
        Point::new(30.0, 30.0),
        25.0,
        vec![
            ColorStop { offset: 0.0, color: Color::rgba8(255, 255, 0, 255) },
            ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 0, 255) },
        ],
    );
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    // Center — yellow.
    let c = p.get_pixel(30, 30);
    assert!(c[0] > 200 && c[1] > 200, "center yellow, got {:?}", c);
    // Edge of radius — near black.
    let e = p.get_pixel(55, 30);
    assert!(e[0] < 50 && e[1] < 50, "outside-radius near black, got {:?}", e);
}

#[test]
fn linear_extend_repeat_cycles() {
    // 20-px gradient slot, extend repeat across 100-px rect → red↔blue
    // cycle should repeat 5 times. Sample at multiples of 20 — every
    // 5-th, 25-th, 45-th, ... pixel should be near-red (offset 0).
    let mut p = Pixmap::new(100, 4);
    let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(20.0, 0.0));
    g.stops = vec![
        ColorStop { offset: 0.0, color: Color::rgba8(255, 0, 0, 255) },
        ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 255, 255) },
    ].into_iter().collect::<ColorStops>();
    g.extend = Extend::Repeat;
    let brush = Brush::Gradient(g);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 4.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Every 20px the cycle restarts → near-red.
    for cycle in 0..5 {
        let x = (cycle * 20 + 1) as u32; // +1 to skip the seam ambiguity
        let c = p.get_pixel(x, 2);
        assert!(c[0] > 200 && c[2] < 60, "cycle {} at x={} should be near-red, got {:?}", cycle, x, c);
    }
}

#[test]
fn linear_extend_reflect_pingpongs() {
    // 20-px slot, reflect across 80-px rect → red→blue→red→blue→red ping.
    // At x=20 (end of slot 0) → blue, x=40 (end of mirrored slot) → red.
    let mut p = Pixmap::new(80, 4);
    let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(20.0, 0.0));
    g.stops = vec![
        ColorStop { offset: 0.0, color: Color::rgba8(255, 0, 0, 255) },
        ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 255, 255) },
    ].into_iter().collect::<ColorStops>();
    g.extend = Extend::Reflect;
    let brush = Brush::Gradient(g);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 80.0, 4.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let c_at_19 = p.get_pixel(19, 2);   // end of slot 0 → near blue
    let c_at_21 = p.get_pixel(21, 2);   // start of mirrored slot → also near blue
    let c_at_39 = p.get_pixel(39, 2);   // end of mirrored slot → near red
    let c_at_41 = p.get_pixel(41, 2);   // start of next slot → also near red
    assert!(c_at_19[2] > 200, "x=19 near blue, got {:?}", c_at_19);
    assert!(c_at_21[2] > 200, "x=21 reflected near blue, got {:?}", c_at_21);
    assert!(c_at_39[0] > 200, "x=39 reflected back to red, got {:?}", c_at_39);
    assert!(c_at_41[0] > 200, "x=41 continues red, got {:?}", c_at_41);
}

#[test]
fn linear_multistop_three_colors() {
    // 3-stop gradient: red(0) → green(0.5) → blue(1.0). Middle should
    // be green, not a R+B mix. Confirms multi-stop bracket walking.
    let mut p = Pixmap::new(100, 4);
    let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
    g.stops = vec![
        ColorStop { offset: 0.0, color: Color::rgba8(255, 0, 0, 255) },
        ColorStop { offset: 0.5, color: Color::rgba8(0, 255, 0, 255) },
        ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 255, 255) },
    ].into_iter().collect::<ColorStops>();
    let brush = Brush::Gradient(g);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 4.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let l = p.get_pixel(2, 2);
    let m = p.get_pixel(50, 2);
    let r = p.get_pixel(97, 2);
    assert!(l[0] > 200,  "left red, got {:?}", l);
    assert!(m[1] > 200 && m[0] < 60 && m[2] < 60, "middle pure green, got {:?}", m);
    assert!(r[2] > 200,  "right blue, got {:?}", r);
}

#[test]
fn sweep_gradient_paints_full_circle() {
    // Sweep from start_angle=-PI to end_angle=PI, red→blue.
    // At angle 0 (east of centre): mid t ≈ 0.5 → mix.
    // At angle close to ±PI (west of centre): edge of gradient.
    let mut p = Pixmap::new(60, 60);
    let mut g = Gradient::new_sweep(Point::new(30.0, 30.0), -std::f32::consts::PI, std::f32::consts::PI);
    g.stops = vec![
        ColorStop { offset: 0.0, color: Color::rgba8(255, 0, 0, 255) },
        ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 255, 255) },
    ].into_iter().collect::<ColorStops>();
    let brush = Brush::Gradient(g);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Any non-centre pixel should NOT be (0,0,0,0) — sweep paints
    // everywhere (previous impl returned false → silent no-op).
    let mut painted = 0u32;
    for y in [5_u32, 15, 30, 45, 55] {
        for x in [5_u32, 15, 30, 45, 55] {
            if (x, y) == (30, 30) { continue; }
            if p.get_pixel(x, y)[3] > 0 { painted += 1; }
        }
    }
    assert!(painted >= 20, "sweep should paint most cells, got {}/{}", painted, 24);
}

#[test]
fn focal_radial_falls_back_to_concentric() {
    // Focal radial — start_center 10px off from end_center. Previous
    // impl silently dropped start_center. Now we render concentric
    // approximation + bump a counter; verify rendering is non-empty.
    let mut p = Pixmap::new(60, 60);
    let mut g = Gradient::new_radial(Point::new(30.0, 30.0), 25.0);
    if let GradientKind::Radial { start_center, start_radius, .. } = &mut g.kind {
        *start_center = Point::new(20.0, 20.0);
        *start_radius = 5.0;
    }
    g.stops = vec![
        ColorStop { offset: 0.0, color: Color::rgba8(255, 255, 0, 255) },
        ColorStop { offset: 1.0, color: Color::rgba8(0, 0, 0, 255) },
    ].into_iter().collect::<ColorStops>();
    let brush = Brush::Gradient(g);
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        radii: None,
        brush,
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Centre rendered. Falls back to concentric on end_center so (30,30) → yellow.
    let c = p.get_pixel(30, 30);
    assert!(c[0] > 200 && c[1] > 200, "focal-degraded centre still painted, got {:?}", c);
}

#[test]
fn linear_lut_cached_across_renders() {
    // Second render of the same gradient must reuse the LUT —
    // we can't directly observe this without exposing the cache, but
    // hash should match bit-exactly between two runs.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut p1 = Pixmap::new(50, 50);
    let mut p2 = Pixmap::new(50, 50);
    let make_scene = || {
        let brush = make_linear(
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            vec![
                ColorStop { offset: 0.0, color: Color::rgba8(100, 200, 50, 255) },
                ColorStop { offset: 1.0, color: Color::rgba8(255, 100, 200, 255) },
            ],
        );
        let mut sc = Scene::new();
        sc.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
            radii: None,
            brush,
            transform: Affine::IDENTITY,
        });
        sc
    };
    s().render(&make_scene(), &mut p1).unwrap();
    s().render(&make_scene(), &mut p2).unwrap();
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    p1.pixels().hash(&mut h1);
    p2.pixels().hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish(), "same gradient → identical pixels");
}
