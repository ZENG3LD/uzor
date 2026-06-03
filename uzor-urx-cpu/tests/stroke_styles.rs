//! Stroke join + cap styles. Confirms Miter/Round/Bevel/Square actually
//! affect output (prior to H7 every style rendered identically).

use kurbo::BezPath;
use uzor_urx_core::math::{Affine, Brush, Color};
use uzor_urx_core::scene::{DrawCommand, LineCap, LineJoin, Scene, Stroke};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn s() -> CpuBackend { CpuBackend::new() }

fn render_corner(join: LineJoin) -> Pixmap {
    // L-shaped path with one sharp 90° corner at (40, 40). Stroke 8px.
    let mut p = Pixmap::new(80, 80);
    let mut path = BezPath::new();
    path.move_to((40.0, 5.0));
    path.line_to((40.0, 40.0));
    path.line_to((75.0, 40.0));
    let mut scene = Scene::new();
    scene.push(DrawCommand::StrokePath {
        path,
        stroke: Stroke { width: 8.0, miter_limit: 10.0, join, cap: LineCap::Butt },
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    p
}

#[test]
fn miter_round_bevel_produce_distinct_outputs() {
    let p_miter  = render_corner(LineJoin::Miter);
    let p_round  = render_corner(LineJoin::Round);
    let p_bevel  = render_corner(LineJoin::Bevel);
    let hash = |p: &Pixmap| {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        p.pixels().hash(&mut h);
        h.finish()
    };
    let hm = hash(&p_miter);
    let hr = hash(&p_round);
    let hb = hash(&p_bevel);
    assert_ne!(hm, hr, "Miter and Round must differ");
    assert_ne!(hm, hb, "Miter and Bevel must differ");
    assert_ne!(hr, hb, "Round and Bevel must differ");
}

#[test]
fn miter_paints_more_pixels_than_bevel() {
    // Sharp V-corner. Miter quad reaches further than bevel chamfer
    // → total painted-pixel count must be strictly greater. This is
    // a hash-free, geometry-free assertion that's robust to coord
    // arithmetic shifts.
    fn render(join: LineJoin) -> Pixmap {
        let mut p = Pixmap::new(120, 80);
        let mut path = BezPath::new();
        path.move_to((20.0, 60.0));
        path.line_to((60.0, 60.0));
        path.line_to((20.0, 40.0));
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokePath {
            path,
            stroke: Stroke { width: 6.0, miter_limit: 20.0, join, cap: LineCap::Butt },
            brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
            transform: Affine::IDENTITY,
        });
        s().render(&scene, &mut p).unwrap();
        p
    }
    let count = |p: &Pixmap| -> u32 {
        let mut n = 0u32;
        for y in 0..80 { for x in 0..120 {
            if p.get_pixel(x, y)[3] > 0 { n += 1; }
        }}
        n
    };
    let nm = count(&render(LineJoin::Miter));
    let nb = count(&render(LineJoin::Bevel));
    assert!(nm > nb, "Miter must cover more pixels than Bevel: miter={}, bevel={}", nm, nb);
    // Sanity: difference must be material (>= 20 pixels), not floor noise.
    assert!(nm - nb >= 20, "Miter advantage too small: miter={}, bevel={}", nm, nb);
}

#[test]
fn miter_limit_falls_back_to_bevel_at_sharp_angle() {
    // Very acute angle joint with miter_limit=2. Miter point would
    // overshoot far past 2×half_w → fallback to bevel. Hash equality
    // vs Bevel proves it.
    let make = |join: LineJoin, miter_limit: f32| {
        let mut p = Pixmap::new(80, 80);
        let mut path = BezPath::new();
        // ~10° turn.
        path.move_to((10.0, 70.0));
        path.line_to((70.0, 70.0));
        path.line_to((10.0, 60.0));
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokePath {
            path,
            stroke: Stroke { width: 8.0, miter_limit, join, cap: LineCap::Butt },
            brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
            transform: Affine::IDENTITY,
        });
        s().render(&scene, &mut p).unwrap();
        p
    };
    let p_miter_capped = make(LineJoin::Miter, 2.0);
    let p_bevel        = make(LineJoin::Bevel, 10.0);
    assert_eq!(p_miter_capped.pixels(), p_bevel.pixels(),
        "miter past limit must fall back to bevel pixels");
}

#[test]
fn round_cap_extends_endpoint() {
    // Open line; round cap should paint a half-disc past the endpoint.
    let make = |cap: LineCap| {
        let mut p = Pixmap::new(40, 40);
        let mut path = BezPath::new();
        path.move_to((10.0, 20.0));
        path.line_to((30.0, 20.0));
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokePath {
            path,
            stroke: Stroke { width: 8.0, miter_limit: 10.0, join: LineJoin::Round, cap },
            brush: Brush::Solid(Color::rgba8(0, 0, 200, 255)),
            transform: Affine::IDENTITY,
        });
        s().render(&scene, &mut p).unwrap();
        p
    };
    let p_butt  = make(LineCap::Butt);
    let p_round = make(LineCap::Round);
    let p_square= make(LineCap::Square);
    // 2 px past the end at (32, 20) — Round/Square paint, Butt does not.
    let pb = p_butt.get_pixel(32, 20)[3];
    let pr = p_round.get_pixel(32, 20)[3];
    let ps = p_square.get_pixel(32, 20)[3];
    assert!(pb < 30, "butt must NOT extend past endpoint, got {}", pb);
    assert!(pr > 150, "round must extend past endpoint, got {}", pr);
    assert!(ps > 150, "square must extend past endpoint, got {}", ps);
}

#[test]
fn sub_pixel_stroke_width_does_not_panic() {
    // 0.3px line — exercise the sub-pixel branch and confirm it
    // produces SOME painted pixels (not silent skip).
    let mut p = Pixmap::new(20, 20);
    let mut path = BezPath::new();
    path.move_to((2.0, 10.0));
    path.line_to((18.0, 10.0));
    let mut scene = Scene::new();
    scene.push(DrawCommand::StrokePath {
        path,
        stroke: Stroke { width: 0.3, ..Stroke::default() },
        brush: Brush::Solid(Color::rgba8(255, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let painted: u32 = (2..18).map(|x| (p.get_pixel(x, 10)[3] > 0) as u32).sum();
    assert!(painted > 0, "0.3 px line must paint at least some pixels");
}
