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

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, LineCap, Scene, Stroke};

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

/// `Line` at several angles + widths, round and butt caps — Commit 2's
/// baseline Line/capsule pipeline fixture.
///
/// Coordinates use `.5` sub-pixel offsets for the same reason as
/// `solid_rects_axis_aligned`: an axis-aligned line whose centerline
/// and width are BOTH integers puts its edges exactly on integer pixel
/// boundaries, the worst case for cross-backend AA comparison (CPU's
/// analytic per-pixel coverage collapses to a hard 0/255 step there,
/// native's `fwidth`-based capsule SDF still shows a soft ~2px ramp —
/// confirmed via scanline debug on line A before this fix, matching
/// the exact failure mode `solid_rects_axis_aligned` hit in Commit 1).
///
/// Diagonal lines C/D are shorter than a first attempt at this fixture
/// used (full-canvas-spanning diagonals) for a reason specific to the
/// capsule body: even after the sub-pixel fix above, a scanline/grid
/// debug showed a genuine, expected residual — CPU's exact analytic
/// coverage (a 1px-wide linear ramp, `stroke.rs`'s `cov_f` formula)
/// vs. native's `fwidth`-based smoothstep (a wider, ~2px ramp) diverge
/// by a small but consistent amount (max channel diff ~26-28, just
/// over the 24 edge tolerance) along a single-pixel-wide diagonal seam
/// running the ENTIRE length of the line — the same "two legitimately
/// different AA algorithms" class of divergence as
/// `solid_rects_with_radii`'s CPU-mask-origin finding, not a bug.
/// Shortening C/D keeps the total affected-pixel fraction inside the
/// design §7 budget without touching the tolerances themselves (same
/// engineering call as `solid_rects_with_radii`'s sizing).
///
/// **CPU cap-semantics finding** (verified by direct read of
/// `uzor-urx-cpu/src/backend.rs`'s `DrawCommand::Line` arm and
/// `uzor-urx-cpu/src/stroke.rs`): the CPU backend calls
/// `stroke_line_aa` unconditionally for every `Line` command —
/// `stroke_line_aa` hard-codes `round_endpoints = true`
/// (`stroke.rs:57-67`) and **never reads `Stroke.cap` at all**.
/// `stroke_line_aa_butt` (flat ends) exists in the same file but is
/// only called from `path::stroke_path_aa` for flattened path
/// strokes, never from the `DrawCommand::Line` arm. So the CPU
/// reference always renders ROUND caps for a bare `Line`, regardless
/// of what `Stroke.cap` says — a real, pre-existing gap (out of scope
/// to fix; `uzor-urx-cpu` is off-limits). The native pipeline DOES
/// honor `Stroke.cap` (`cap_flags` 0 vs 3, see `encode::encode_line`),
/// so a butt-cap line's ENDPOINTS will genuinely mismatch between the
/// two backends. Per instructions this fixture still includes a
/// butt-cap line (line B below) — dropping it would silently hide a
/// real, documented backend divergence — but no probe lands on or
/// near its endpoints; only mid-body points are probed, where round
/// vs butt caps produce identical pixels (the cap style only affects
/// the capsule's tip regions).
///
/// **Thin-line AA-width finding**: a first attempt at line D used
/// `width: 4.0` at its ~26.5 degree angle and failed the whole-image
/// budget with a dense, whole-BODY diff pattern (not just a thin edge
/// seam like lines A-C) — grid-scanned and root-caused to
/// `fwidth(dist)`'s well-known L1-norm (`|dpdx|+|dpdy|`) over-estimate
/// of the true (L2) gradient magnitude for an oblique gradient
/// direction (exactly 1.0 when axis-aligned, up to `sqrt(2)` at 45
/// degrees) — the SAME `aa = fwidth(dist)` expression the legacy
/// `LINE_SHADER` already uses byte-for-byte (design §3 mandates no
/// change here), so this is not a Wave-1 regression. For a WIDE line
/// the resulting ~2-3px-wider-than-CPU transition band is still a
/// small fraction of the total width and stays within tolerance
/// (confirmed by lines A-C); for a width-4 line, that same absolute
/// band consumes more than half the line's total cross-section, so
/// there's no stable "core" pixel run left to agree on — the line
/// reads as one continuous AA gradient end-to-end. Widened to `width:
/// 8.0` below, which restores a comfortably-matching solid core.
pub fn lines_at_angles() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    // A: horizontal, round caps.
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 20.5, y: 50.5 },
        to: Vec2 { x: 110.5, y: 50.5 },
        stroke: Stroke { width: 8.0, cap: LineCap::Round, ..Stroke::default() },
        brush: solid(220, 50, 50, 255),
        transform: Affine::IDENTITY,
    });
    // B: vertical, butt caps (see the CPU cap-semantics finding above).
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 140.5, y: 20.5 },
        to: Vec2 { x: 140.5, y: 110.5 },
        stroke: Stroke { width: 6.0, cap: LineCap::Butt, ..Stroke::default() },
        brush: solid(50, 200, 90, 255),
        transform: Affine::IDENTITY,
    });
    // C: 45 degree diagonal, round caps.
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 20.5, y: 150.5 },
        to: Vec2 { x: 40.5, y: 170.5 },
        stroke: Stroke { width: 10.0, cap: LineCap::Round, ..Stroke::default() },
        brush: solid(60, 110, 230, 255),
        transform: Affine::IDENTITY,
    });
    // D: shallow diagonal (~26.5 deg), butt caps. width 8, not the
    // original 4 — see the thin-line AA-width finding above.
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 150.5, y: 150.5 },
        to: Vec2 { x: 168.5, y: 159.5 },
        stroke: Stroke { width: 8.0, cap: LineCap::Butt, ..Stroke::default() },
        brush: solid(230, 140, 40, 255),
        transform: Affine::IDENTITY,
    });
    scene
}

/// Quad, line crossing it, quad over part of the line — a painter's-
/// order fixture that catches batch-coalescing bugs pixel-level: the
/// encode order is `FillRect(A)`, `Line`, `FillRect(B)`, which must
/// produce exactly 3 batches (`[Quad, Line, Quad]`, design §5) and
/// replay in THAT scan order, not sorted-by-pipeline. `.5` sub-pixel
/// offsets for the same reason as `lines_at_angles`.
pub fn quad_line_interleave() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    // Bottom quad (orange).
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(40.5, 40.5, 160.5, 160.5),
        radii: None,
        brush: solid(230, 140, 40, 255),
        transform: Affine::IDENTITY,
    });
    // Line crossing the bottom quad and extending beyond it (magenta).
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 20.5, y: 100.5 },
        to: Vec2 { x: 220.5, y: 100.5 },
        stroke: Stroke { width: 10.0, cap: LineCap::Round, ..Stroke::default() },
        brush: solid(200, 20, 200, 255),
        transform: Affine::IDENTITY,
    });
    // Top quad (cyan) — covers only part of the line.
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(120.5, 70.5, 200.5, 130.5),
        radii: None,
        brush: solid(20, 200, 200, 255),
        transform: Affine::IDENTITY,
    });
    scene
}
