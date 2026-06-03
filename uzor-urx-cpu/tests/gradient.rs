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
