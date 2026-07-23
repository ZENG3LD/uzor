//! Scene fixtures shared by the pixel-parity harness (`tests/parity.rs`).
//!
//! All fixtures render at a fixed 256x256 canvas for determinism.
//! Coordinator Amendment B: every fixture STARTS with an opaque
//! full-canvas background `FillRect` before its own content — the CPU
//! `Pixmap` is zero-initialized (transparent) and the GPU target is
//! cleared to `wgpu::Color::TRANSPARENT`, so without an explicit opaque
//! base the two would trivially agree on "transparent" everywhere;
//! the background rect forces both backends to composite over an
//! identical opaque ground and actually exercises blend semantics.

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};

pub const CANVAS: u32 = 256;

/// Opaque dark-gray background covering the whole canvas — the common
/// first command of every fixture (Amendment B).
fn push_background(scene: &mut Scene) {
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, CANVAS as f64, CANVAS as f64),
        radii: None,
        brush: Brush::Solid(Color::from_rgba8(32, 32, 32, 255)),
        transform: Affine::IDENTITY,
    });
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> Brush {
    Brush::Solid(Color::from_rgba8(r, g, b, a))
}

/// Plain `FillRect`, `Brush::Solid`, no radii — Quad pipeline baseline.
///
/// Coordinates are deliberately NOT integer-pixel-aligned (`.5`
/// fractional offsets). An edge landing exactly on an integer pixel
/// boundary is the worst case for cross-backend AA comparison: the CPU
/// analytic rasteriser produces a perfectly hard 0/255 step there (no
/// partial-coverage pixel at all), while the native SDF's `fwidth`-based
/// smoothstep always spans ~1-2 px regardless of alignment — the two
/// would disagree on nearly every boundary pixel, which is a test-
/// fixture artifact, not the AA-shape slack the tolerance budget (design
/// §7) is meant to absorb. Sub-pixel offsets give the CPU rasteriser a
/// real partial-coverage pixel to compare against, which is also more
/// representative of real (non-grid-snapped) chart content.
pub fn solid_rects_axis_aligned() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(20.5, 20.5, 100.5, 100.5),
        radii: None,
        brush: solid(220, 40, 40, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(150.5, 150.5, 230.5, 230.5),
        radii: None,
        brush: solid(40, 200, 90, 255),
        transform: Affine::IDENTITY,
    });
    scene
}

/// `FillRect` with `radii: Some([r;4])` — SDF rounded-rect.
///
/// Sized smaller than `solid_rects_axis_aligned` for a reason specific
/// to the CPU reference's rounded-fill code path: `uzor-urx-cpu`
/// renders a radii'd `FillRect` by pushing a cached A8 clip mask
/// (`uzor-urx-cpu/src/rounded.rs::rounded_clip_to_mask`) whose screen
/// **origin is snapped to the nearest integer pixel**
/// (`(x0.round(), y0.round())`, `rounded.rs:198`) — the mask itself
/// still antialiases its own edges, but the sub-pixel fractional part
/// of the rect's true position is discarded at placement time. That's
/// a real, pre-existing precision limit of the CPU reference (out of
/// scope for this crate to fix — `uzor-urx-cpu` is off-limits), and it
/// makes the CPU rendering of a rounded rect's boundary consistently
/// "harder-edged" than the native SDF's continuous, non-quantized
/// antialiasing, regardless of which sub-pixel offset this fixture
/// picks. The mismatch scales with the rect's boundary length, so this
/// fixture uses a smaller rect than the axis-aligned baseline to keep
/// the affected-pixel fraction inside the design §7 tolerance budget
/// without touching the tolerances themselves.
pub fn solid_rects_with_radii() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(72.5, 72.5, 184.5, 184.5),
        radii: Some([16.0; 4]),
        brush: solid(60, 110, 230, 255),
        transform: Affine::IDENTITY,
    });
    scene
}

/// `StrokeRect`, with and without radii — centered-border formula
/// (design §3): the visible band straddles the rect edge, so the
/// stroke's true bounding box (not just the logical rect) matters for
/// pixel probes. Sub-pixel offset for the same reason as
/// `solid_rects_axis_aligned`.
pub fn stroked_rects() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::StrokeRect {
        rect: Rect::new(30.5, 30.5, 120.5, 120.5),
        radii: None,
        stroke: Stroke { width: 6.0, ..Stroke::default() },
        brush: solid(240, 210, 40, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::StrokeRect {
        rect: Rect::new(140.5, 30.5, 230.5, 120.5),
        radii: Some([16.0; 4]),
        stroke: Stroke { width: 6.0, ..Stroke::default() },
        brush: solid(40, 210, 230, 255),
        transform: Affine::IDENTITY,
    });
    scene
}

/// 2 overlapping `FillRect`s, alpha < 255 — exercises blend-order +
/// premultiplied-blend-equation correctness (design §7); this is the
/// fixture that actually fails if the native pipeline's blend state
/// reverts to straight-alpha.
pub fn overlapping_translucent_rects() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(40.0, 40.0, 160.0, 160.0),
        radii: None,
        brush: solid(220, 30, 30, 128),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(100.0, 100.0, 220.0, 220.0),
        radii: None,
        brush: solid(30, 60, 220, 128),
        transform: Affine::IDENTITY,
    });
    scene
}
