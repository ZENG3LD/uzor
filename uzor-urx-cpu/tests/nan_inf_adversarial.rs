//! Adversarial input: NaN / ±Inf in any coord must NOT panic, must NOT
//! corrupt the pixmap (i.e. other valid commands still paint normally).
//!
//! Policy: silent skip + KEY_RENDER_SKIPPED_NONFINITE counter.

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
use uzor_urx_cpu::{CpuBackend, Pixmap};

const W: u32 = 64;
const H: u32 = 32;
const RED:   Color = Color { r: 255, g: 0, b: 0, a: 255 };
const GREEN: Color = Color { r: 0, g: 255, b: 0, a: 255 };

fn render(scene: &Scene) -> Pixmap {
    let mut p = Pixmap::new(W, H);
    CpuBackend::new().render(scene, &mut p).unwrap();
    p
}

#[test]
fn nan_rect_doesnt_panic_and_is_skipped() {
    let mut s = Scene::new();
    s.push(DrawCommand::FillRect {
        rect: Rect::new(f64::NAN, 0.0, 10.0, 10.0),
        radii: None,
        brush: Brush::Solid(RED),
        transform: Affine::IDENTITY,
    });
    let p = render(&s);
    // Whole pixmap stays transparent black — the bad cmd contributed nothing.
    assert_eq!(p.get_pixel(0, 0), [0, 0, 0, 0]);
    assert_eq!(p.get_pixel(5, 5), [0, 0, 0, 0]);
}

#[test]
fn inf_rect_doesnt_panic_and_is_skipped() {
    let mut s = Scene::new();
    s.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, f64::INFINITY, 10.0),
        radii: None,
        brush: Brush::Solid(RED),
        transform: Affine::IDENTITY,
    });
    let p = render(&s);
    assert_eq!(p.get_pixel(0, 0), [0, 0, 0, 0]);
}

#[test]
fn nan_transform_doesnt_panic() {
    let mut s = Scene::new();
    s.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        radii: None,
        brush: Brush::Solid(RED),
        transform: Affine::new([1.0, 0.0, 0.0, 1.0, f64::NAN, 0.0]),
    });
    let p = render(&s);
    assert_eq!(p.get_pixel(0, 0), [0, 0, 0, 0]);
}

#[test]
fn valid_cmds_still_paint_after_bad_cmd() {
    // Verify the bad cmd is skipped but subsequent valid cmds still run.
    let mut s = Scene::new();
    // Bad NaN rect — should be skipped.
    s.push(DrawCommand::FillRect {
        rect: Rect::new(f64::NAN, 0.0, 10.0, 10.0),
        radii: None,
        brush: Brush::Solid(RED),
        transform: Affine::IDENTITY,
    });
    // Valid green rect.
    s.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), GREEN);
    let p = render(&s);
    let px = p.get_pixel(5, 5);
    assert_eq!(px[1], 255, "green channel should be 255, got {:?}", px);
    assert_eq!(px[3], 255, "alpha should be 255, got {:?}", px);
}

#[test]
fn nan_line_endpoints_are_skipped() {
    let mut s = Scene::new();
    s.push(DrawCommand::Line {
        from: Vec2::new(f64::NAN, 0.0),
        to: Vec2::new(10.0, 10.0),
        stroke: Stroke { width: 1.0, ..Stroke::default() },
        brush: Brush::Solid(RED),
        transform: Affine::IDENTITY,
    });
    let p = render(&s);
    assert_eq!(p.get_pixel(5, 5), [0, 0, 0, 0]);
}

#[test]
fn nan_radii_are_skipped() {
    let mut s = Scene::new();
    s.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        radii: Some([1.0, f32::NAN, 1.0, 1.0]),
        brush: Brush::Solid(RED),
        transform: Affine::IDENTITY,
    });
    let p = render(&s);
    assert_eq!(p.get_pixel(5, 5), [0, 0, 0, 0]);
}

#[test]
fn many_bad_cmds_in_50plus_scene_still_safe() {
    // The tile path kicks in at ≥50 cmds. Verify tile_eligible
    // disqualifies any scene containing a non-finite cmd (so the
    // scanline path's silent-skip handles them safely).
    let mut s = Scene::new();
    for i in 0..60 {
        s.fill_rect_solid(
            Rect::new(i as f64, 0.0, i as f64 + 1.0, 5.0),
            GREEN,
        );
    }
    // Slip in one NaN-rect.
    s.push(DrawCommand::FillRect {
        rect: Rect::new(f64::NAN, 0.0, 10.0, 10.0),
        radii: None,
        brush: Brush::Solid(RED),
        transform: Affine::IDENTITY,
    });
    // Render — must not panic.
    let p = render(&s);
    // Most green rects should still be visible.
    assert_eq!(p.get_pixel(30, 2)[1], 255);
}
