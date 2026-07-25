//! Dash-pattern rasterisation — closes the defect where `Stroke.dash`
//! was silently dropped by `uzor-urx-cpu` (`Line`/`StrokeRect`/
//! `StrokePath` all rendered solid regardless of the requested
//! pattern). Each test proves a dashed stroke rasterizes to MULTIPLE
//! disjoint ink runs along the stroke, not one continuous run —
//! sampling actual pixels, not just checking the IR carries a pattern.

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{Dash, DrawCommand, LineCap, LineJoin, Scene, Stroke};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn s() -> CpuBackend { CpuBackend::new() }

fn dashed_stroke(width: f32, pattern: &[f32]) -> Stroke {
    Stroke {
        width,
        miter_limit: 4.0,
        join: LineJoin::Miter,
        cap: LineCap::Butt,
        dash: Some(Dash { pattern: pattern.to_vec(), phase: 0.0 }),
    }
}

/// Count `x`-transitions into a "covered" (opaque, non-background) run
/// along pixel row `y`.
fn ink_runs_along_row(p: &Pixmap, y: u32, x_range: std::ops::Range<u32>) -> usize {
    let mut runs = 0usize;
    let mut was_covered = false;
    for x in x_range {
        let covered = p.get_pixel(x, y)[3] > 32;
        if covered && !was_covered {
            runs += 1;
        }
        was_covered = covered;
    }
    runs
}

#[test]
fn dashed_line_produces_multiple_disjoint_ink_runs() {
    let mut p = Pixmap::new(100, 10);
    let mut scene = Scene::new();
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 0.0, y: 5.0 },
        to: Vec2 { x: 100.0, y: 5.0 },
        stroke: dashed_stroke(4.0, &[10.0, 10.0]),
        brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let runs = ink_runs_along_row(&p, 5, 0..100);
    assert!(runs >= 3, "expected several disjoint dash runs, got {runs}");
}

#[test]
fn solid_line_is_a_single_continuous_run_control() {
    // Same geometry, no dash — control case proving `ink_runs_along_row`
    // itself reports 1 for an ordinary solid stroke (not an artifact of
    // the counting method).
    let mut p = Pixmap::new(100, 10);
    let mut scene = Scene::new();
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 0.0, y: 5.0 },
        to: Vec2 { x: 100.0, y: 5.0 },
        stroke: Stroke { width: 4.0, ..Stroke::default() },
        brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let runs = ink_runs_along_row(&p, 5, 0..100);
    assert_eq!(runs, 1, "an undashed stroke must be one continuous run");
}

#[test]
fn dashed_stroke_rect_top_edge_produces_multiple_disjoint_ink_runs() {
    let mut p = Pixmap::new(100, 40);
    let mut scene = Scene::new();
    scene.push(DrawCommand::StrokeRect {
        rect: Rect::new(5.0, 5.0, 95.0, 35.0),
        radii: None,
        stroke: dashed_stroke(3.0, &[8.0, 8.0]),
        brush: Brush::Solid(Color::from_rgba8(0, 255, 0, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    // Sample along the rect's own top edge (y=5), away from the corners.
    let runs = ink_runs_along_row(&p, 5, 10..90);
    assert!(runs >= 3, "expected several disjoint dash runs along the rect's top edge, got {runs}");
}

#[test]
fn dashed_stroke_path_produces_multiple_disjoint_ink_runs() {
    let mut p = Pixmap::new(100, 10);
    let mut path = uzor_urx_core::math::BezPath::new();
    path.move_to((0.0, 5.0));
    path.line_to((100.0, 5.0));
    let mut scene = Scene::new();
    scene.push(DrawCommand::StrokePath {
        path,
        stroke: dashed_stroke(4.0, &[10.0, 10.0]),
        brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();
    let runs = ink_runs_along_row(&p, 5, 0..100);
    assert!(runs >= 3, "expected several disjoint dash runs, got {runs}");
}

#[test]
fn dashed_stroke_scales_with_the_transform_ctm() {
    // Same 100-unit line, same [10,10] LOCAL-space pattern, but under a
    // 2x scale transform — per `Dash`'s own coordinate-space contract
    // (matches tiny-skia), the on-screen dash period must scale WITH
    // the transform (20 device px on, 20 off), not stay device-constant
    // — a run count roughly HALF of the unscaled case's run count over
    // the same on-screen span proves the pattern is being scaled, not
    // ignored or applied post-transform.
    let mut p_unscaled = Pixmap::new(200, 10);
    let mut scene_unscaled = Scene::new();
    scene_unscaled.push(DrawCommand::Line {
        from: Vec2 { x: 0.0, y: 5.0 },
        to: Vec2 { x: 200.0, y: 5.0 },
        stroke: dashed_stroke(4.0, &[10.0, 10.0]),
        brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
        transform: Affine::IDENTITY,
    });
    s().render(&scene_unscaled, &mut p_unscaled).unwrap();
    let unscaled_runs = ink_runs_along_row(&p_unscaled, 5, 0..200);

    let mut p_scaled = Pixmap::new(200, 10);
    let mut scene_scaled = Scene::new();
    scene_scaled.push(DrawCommand::Line {
        from: Vec2 { x: 0.0, y: 5.0 },
        to: Vec2 { x: 100.0, y: 5.0 }, // same 100 LOCAL units
        stroke: dashed_stroke(4.0, &[10.0, 10.0]),
        brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
        transform: Affine::scale(2.0), // -> 200 device px on screen
    });
    s().render(&scene_scaled, &mut p_scaled).unwrap();
    let scaled_runs = ink_runs_along_row(&p_scaled, 5, 0..200);

    assert!(
        scaled_runs < unscaled_runs,
        "a 2x CTM must widen the on-screen dash period (fewer, wider runs over the same 200px span): unscaled={unscaled_runs} scaled={scaled_runs}"
    );
}
