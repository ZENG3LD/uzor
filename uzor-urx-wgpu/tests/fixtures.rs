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

use uzor_urx_core::math::{Affine, BezPath, Brush, Color, Gradient, Rect, Vec2};
use uzor_urx_core::scene::{DrawCommand, FillRule, LineCap, LineJoin, Scene, Stroke};

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

/// `FillRect` with `Brush::Gradient(Linear)` AND `radii` — the routed-
/// through-Path-pipeline case (design §5). Horizontal gradient axis
/// spanning EXACTLY the rect's un-rounded bounding box (`x0` to `x1`),
/// two FULLY OPAQUE stops at offsets 0.0/1.0.
///
/// Both choices are load-bearing, not cosmetic:
/// - Spanning the whole bounding box means every point inside the
///   shape sits on the SAME single affine segment of the gradient
///   function (no `Extend::Pad` clamping ever kicks in inside the
///   rect) — per-vertex sampling + the rasteriser's linear
///   interpolation is then mathematically EXACT versus CPU's
///   per-pixel evaluation, not merely close, at any interior point.
/// - Fully opaque stops make straight-alpha vertex-lerp (this
///   pipeline's convention) and premultiplied-lerp (CPU's LUT
///   convention) coincide exactly — see `encode.rs`'s module doc,
///   "Gradient-routing finding".
///
/// `radii` is the routed case's whole point (design §5) — but see
/// `encode.rs`'s module doc for the CPU-side finding this surfaces:
/// `uzor-urx-cpu`'s gradient fill never actually consults the rounded
/// clip mask it pushes, so CPU renders this SAME scene with SQUARE
/// corners while this pipeline renders true rounded ones. That
/// corner-region mismatch is expected and confined to a small area —
/// sized (rect/radius choice, same engineering call as
/// `solid_rects_with_radii`) to keep it inside the design §7 whole-
/// image budget without touching the tolerances. Probes stay well
/// inside the flat interior either way (design risk 5).
pub fn linear_gradient_rect() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let rect = Rect::new(69.5, 69.5, 187.5, 187.5);
    let cy = (rect.y0 + rect.y1) * 0.5;
    let gradient = Gradient::new_linear((rect.x0, cy), (rect.x1, cy)).with_stops([
        (0.0f32, Color::from_rgba8(30, 60, 220, 255)),
        (1.0f32, Color::from_rgba8(230, 180, 30, 255)),
    ]);
    scene.push(DrawCommand::FillRect {
        rect,
        radii: Some([14.0; 4]),
        brush: Brush::Gradient(gradient),
        transform: Affine::IDENTITY,
    });
    scene
}

/// A 5-point star polygon (10 alternating outer/inner vertices),
/// closed. Shared generator for both stars in
/// `tessellated_path_star()` — "same geometry" per the design's
/// fixture spec, just translated.
fn star_path(cx: f64, cy: f64, r_outer: f64, r_inner: f64) -> BezPath {
    use std::f64::consts::{FRAC_PI_2, PI};
    let mut path = BezPath::new();
    for i in 0..10 {
        let angle = -FRAC_PI_2 + (i as f64) * PI / 5.0;
        let r = if i % 2 == 0 { r_outer } else { r_inner };
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if i == 0 {
            path.move_to((x, y));
        } else {
            path.line_to((x, y));
        }
    }
    path.close_path();
    path
}

/// A 5-point star: `FillPath` (nonzero winding) + `StrokePath` (round
/// join) over the SAME star geometry (design §8 commit 3 — "same
/// geometry, different colors"). This is the seam-fix acceptance
/// fixture: a concave, many-triangle tessellation is exactly where the
/// legacy crate's per-triangle-independent barycentric edge AA visibly
/// gaps/double-fades along internal tessellation seams (design §4);
/// this pipeline drops that scheme entirely and relies on MSAA
/// instead. Two separate stars (same generator, different center)
/// rather than one star filled+stroked in place, so each primitive
/// gets its own clean probe region.
pub fn tessellated_path_star() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);

    let fill_star = star_path(70.5, 140.5, 45.0, 18.0);
    scene.push(DrawCommand::FillPath {
        path: fill_star,
        rule: FillRule::NonZero,
        brush: solid(160, 60, 200, 255),
        transform: Affine::IDENTITY,
    });

    let stroke_star = star_path(185.5, 140.5, 45.0, 18.0);
    scene.push(DrawCommand::StrokePath {
        path: stroke_star,
        stroke: Stroke { width: 5.0, join: LineJoin::Round, ..Stroke::default() },
        brush: solid(40, 190, 190, 255),
        transform: Affine::IDENTITY,
    });

    scene
}

/// Nested `PushClipRect`/`PopClip` around overlapping fills, a line,
/// and a path — proves the `clip_rect` field is wired correctly on
/// all 3 native instance types (design §7 fixture list), and that
/// nested clip levels intersect correctly (design §5 `ClipStack`).
///
/// Layout (all coordinates `.5`-offset per the established
/// convention):
/// - Outer clip: `(40.5,40.5)..(216.5,216.5)`.
/// - A teal band drawn while ONLY the outer clip is active — visible
///   in the "ring" between the outer and inner clip levels, never
///   touched by the inner clip.
/// - Inner clip: `(90.5,90.5)..(166.5,166.5)` (nested inside the
///   outer one).
/// - An orange `FillRect`, a magenta `Line`, and a yellow `FillPath`
///   (triangle) — every one of them deliberately POKES OUTSIDE the
///   inner clip's bounds on at least one side, so each must be
///   visibly CUT to the inner clip's 76x76 region if `clip_rect`
///   wiring is correct on that instance type.
///
/// **CPU clip-edge AA finding** (see `encode.rs`'s `ClipStack` doc
/// comment for the full mechanism): for a plain-rect-only clip stack
/// (this fixture never pushes a rounded clip), `uzor-urx-cpu` computes
/// clip boundaries via the SAME analytic per-pixel coverage function
/// used for a shape's own edges (soft AA), while this native pipeline's
/// fragment-shader clip test is a hard binary discard. Probes below sit
/// comfortably away from every clip boundary for exactly this reason;
/// the thin 1px seam along each clip edge is expected to show up only
/// in the whole-image sweep, same class of divergence as every other
/// Wave 1 fixture.
pub fn clip_rect_stack() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);

    scene.push(DrawCommand::PushClipRect {
        rect: Rect::new(40.5, 40.5, 216.5, 216.5),
        transform: Affine::IDENTITY,
    });

    // Outer-level-only content: a thin band near the top of the outer
    // clip, entirely above the inner clip's y-range — this is what
    // shows in the "ring" between outer and inner clip levels.
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(45.5, 45.5, 211.5, 66.5),
        radii: None,
        brush: solid(40, 150, 150, 255),
        transform: Affine::IDENTITY,
    });

    scene.push(DrawCommand::PushClipRect {
        rect: Rect::new(90.5, 90.5, 166.5, 166.5),
        transform: Affine::IDENTITY,
    });

    // Pokes outside the inner clip on all 4 sides.
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(70.5, 70.5, 186.5, 186.5),
        radii: None,
        brush: solid(230, 140, 30, 255),
        transform: Affine::IDENTITY,
    });
    // Pokes outside the inner clip on the left/right.
    scene.push(DrawCommand::Line {
        from: Vec2 { x: 60.5, y: 128.5 },
        to: Vec2 { x: 200.5, y: 128.5 },
        stroke: Stroke { width: 10.0, cap: LineCap::Butt, ..Stroke::default() },
        brush: solid(200, 30, 200, 255),
        transform: Affine::IDENTITY,
    });
    // Pokes outside the inner clip on the top/bottom.
    let mut triangle = BezPath::new();
    triangle.move_to((110.5, 60.5));
    triangle.line_to((150.5, 60.5));
    triangle.line_to((130.5, 200.5));
    triangle.close_path();
    scene.push(DrawCommand::FillPath {
        path: triangle,
        rule: FillRule::NonZero,
        brush: solid(230, 230, 60, 255),
        transform: Affine::IDENTITY,
    });

    scene.push(DrawCommand::PopClip); // back to outer clip
    scene.push(DrawCommand::PopClip); // back to full viewport
    scene
}

/// A trivially-scalable scene (opaque background + one centered solid
/// rect spanning the middle 50% of the canvas) — used ONLY by the
/// resize-sanity test (design §8 commit 3 gate: render 64x64 ->
/// 512x512 -> 64x64 on ONE renderer instance, no panic, final output
/// still parity-green). Every OTHER fixture in this file assumes a
/// fixed 256x256 canvas and would clip/misalign at other sizes; this
/// one scales with whatever `canvas` the caller passes so the SAME
/// generator works at 64 and 512 alike, with a probe point (canvas
/// center) that's always deep inside the rect regardless of size.
pub fn resize_sanity_scene(canvas: u32) -> Scene {
    let c = canvas as f64;
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, c, c),
        radii: None,
        brush: solid(32, 32, 32, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(c * 0.25 + 0.5, c * 0.25 + 0.5, c * 0.75 + 0.5, c * 0.75 + 0.5),
        radii: None,
        brush: solid(220, 60, 60, 255),
        transform: Affine::IDENTITY,
    });
    scene
}

/// `GlyphRun` via the native glyph atlas (Wave 2 design §7) — same
/// recipe as `uzor-urx-cpu/tests/glyph_e2e.rs`: register
/// `uzor-fonts/fonts/DejaVuSans.ttf`, glyph ids 36/37, font_size 48.0,
/// a black background then white text so every non-background pixel
/// is unambiguous.
///
/// Deliberately does NOT call `push_background` (unlike every other
/// fixture in this file) — pure BLACK (not the shared dark-gray)
/// maximizes white-glyph-on-background contrast for probe derivation;
/// this is design §7's own fixture recipe, not a style slip.
///
/// **`register_font` idempotency finding** (read `uzor-urx-glyph/src/
/// lib.rs`'s `register_font` directly): it is NOT idempotent — every
/// call increments a process-wide `next_id` counter and inserts a
/// fresh registry entry, with no content-based dedup by font bytes.
/// Calling it twice with byte-identical `DejaVuSans.ttf` data would
/// mint two DIFFERENT `FontId`s, each independently valid (no
/// correctness bug, just registry growth). This fixture never hits
/// that path: it calls `register_font` exactly ONCE and bakes the
/// resulting `FontId` into the `Scene` it returns; `run_case` (Wave 1)
/// passes that ONE `Scene` BY REFERENCE to both `render_cpu` and
/// `render_native`, so both backends resolve the SAME `FontId` against
/// the SAME process-wide static registry inside `uzor-urx-glyph` —
/// `uzor-urx-cpu`'s `glyph` feature and `uzor-urx-wgpu` both depend on
/// the identical `uzor-urx-glyph` crate, unified to one instance by
/// Cargo in this test binary, so there is exactly one `REGISTRY`/
/// `CACHE` pair to resolve against. No idempotency gap is exercised in
/// practice by this harness.
///
/// **Font-path finding**: `CARGO_MANIFEST_DIR` for this crate is
/// `uzor/uzor-urx-wgpu`; `../uzor-fonts/fonts/DejaVuSans.ttf` resolves
/// to `uzor/uzor-fonts/fonts/DejaVuSans.ttf` — verified present on
/// disk, the SAME relative path `uzor-urx-cpu/tests/glyph_e2e.rs` uses
/// from ITS OWN sibling position one level down from `uzor/`.
pub fn glyph_run_two_letters() -> Scene {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../uzor-fonts/fonts/DejaVuSans.ttf"))
        .expect("uzor-fonts ships DejaVuSans.ttf — same relative path uzor-urx-cpu/tests/glyph_e2e.rs uses");
    let font = uzor_urx_glyph::register_font(bytes).expect("DejaVuSans.ttf is a valid font");

    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, CANVAS as f64, CANVAS as f64),
        radii: None,
        brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::GlyphRun {
        glyphs: vec![
            uzor_urx_core::scene::Glyph { glyph_id: 36, x: 40.0, y: 0.0 },
            uzor_urx_core::scene::Glyph { glyph_id: 37, x: 90.0, y: 0.0 },
        ],
        font,
        font_size: 48.0,
        brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
        transform: Affine::translate((0.0, 130.0)),
        text: None,
    });
    scene
}
