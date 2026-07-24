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

use uzor_urx_core::math::{Affine, BezPath, BlendMode, Brush, Color, Extend, Gradient, Rect, RoundedRect, Vec2};
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

/// `GlyphRun` under an active `PushClipRect` that PARTIALLY clips the
/// text (Wave 7 tail clip fix, 2026-07-24: `uzor-urx-cpu`'s `GlyphRun`
/// arm previously ignored `ClipStack` entirely — every other primitive
/// — `FillRect`/`StrokeRect`/`Line`/`FillPath`/`StrokePath`/`Image` —
/// already threaded `&clip` through it). Reuses
/// [`glyph_run_two_letters`] wholesale (byte-identical font/glyph/pen
/// geometry, so its own three already-proven "deep in stroke, no AA
/// edge" probe points — see `tests/parity.rs::parity_glyph_run_two_letters`'s
/// own doc comment — carry over unchanged) and splices in a clip rect
/// whose right edge (`x=100`) falls INSIDE glyph 37's own ink, between
/// its two known probes `(97,105)` (kept visible, `x=97 < 100`) and
/// `(102,110)` (clipped away, `x=102 >= 100`) — genuinely partially
/// clipping that glyph rather than fully hiding or fully showing it.
/// A plain rect clip is a hard binary scissor test on BOTH backends
/// (`ClipStack::coverage`'s `Rect` arm: `0` or `255`, no AA at all) —
/// unlike a rounded clip, there is no extra boundary-AA divergence
/// source here, so this fixture stays on the base `_TEXT` tolerance
/// tier (same as the unclipped fixture) rather than needing the wider
/// `_CLIP` tier.
pub fn glyph_run_clipped_by_rect() -> Scene {
    let mut scene = glyph_run_two_letters();
    scene.commands.insert(1, DrawCommand::PushClipRect {
        rect: Rect::new(0.0, 0.0, 100.0, CANVAS as f64),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip);
    scene
}

/// `GlyphRun` under an active `PushClipRoundedRect` (Wave 7 tail clip
/// fix) — same two-letter scene, wrapped in the SAME clip shape
/// `rounded_clip_content_crosses_corner` already uses
/// (`RoundedRect(50.5, 50.5, 200.5, 200.5, 40.0)`), chosen so its
/// top-left corner's own cut genuinely overlaps glyph 36's ink —
/// unlike a clip shape positioned to avoid the glyphs entirely, this
/// one has real teeth: a per-pixel diff scan against the PRE-FIX code
/// (a throwaway scratch test, since removed) found a stable,
/// fully-saturated (`diff=255`) divergence region there — e.g.
/// `(47, 115)`: `cpu=[255,255,255,255]` (raw unclipped glyph ink)
/// vs `native=[0,0,0,255]` (correctly clipped background). See
/// `tests/parity.rs::parity_glyph_run_clipped_by_rounded_rect`'s own
/// doc comment for the full probe derivation (that `(47, 115)` point,
/// plus 3 more sanity probes reused from [`glyph_run_two_letters`]
/// proving the REST of the run still composites normally).
pub fn glyph_run_clipped_by_rounded_rect() -> Scene {
    let mut scene = glyph_run_two_letters();
    scene.commands.insert(1, DrawCommand::PushClipRoundedRect {
        rect: RoundedRect::new(50.5, 50.5, 200.5, 200.5, 40.0),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip);
    scene
}

// ── Wave 3 Commit 4: rounded-clip + blend-layer fixtures ──────────
//
// `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`
// §6. All three fixtures below reuse the SAME `.5`-sub-pixel-offset
// and single-shared-background conventions established above.

/// The bbox-vs-real-shape acceptance case (design §6.1) — geometry
/// copied verbatim from the design doc's own sketch. A large radius
/// (40) keeps the bbox-corner-cut area generous. Fills the WHOLE clip
/// bbox: under a bbox-approx clip every pixel here would show fill
/// color; under a real rounded clip, the 4 corner triangles must show
/// BACKGROUND instead — this is the fixture's whole point, and the
/// SAME scene this crate's own `renderer.rs` GPU test
/// (`rounded_clip_corner_is_background_and_center_is_fill`, Wave 3
/// Commit 2) already proved renders correctly on real hardware; this
/// fixture puts it through the full CPU-vs-native parity harness too.
pub fn rounded_clip_content_crosses_corner() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::PushClipRoundedRect {
        rect: RoundedRect::new(50.5, 50.5, 200.5, 200.5, 40.0),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(50.5, 50.5, 200.5, 200.5),
        radii: None,
        brush: solid(220, 60, 60, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip);
    scene
}

/// Nested rounded+rect clip (design §6.2): `PushClipRect(outer)` →
/// `PushClipRoundedRect(inner, nested)` → one content rect that pokes
/// outside BOTH the inner shape's rounded corners AND the outer rect's
/// bounds entirely → `PopClip` (inner) → `PopClip` (outer). A single
/// oversized content rect (rather than one sized to stay inside the
/// outer bound) deliberately exercises BOTH clip mechanisms against
/// the SAME draw call: the inner rounded clip must cut its corners,
/// the outer plain-rect clip must independently cut anything beyond
/// its own bounds, and the two must compose (intersect), not override
/// each other.
///
/// Probe geometry (matches this fixture's 3 named regions in
/// `tests/parity.rs`):
/// - (a) `(128, 128)`: dead center of the inner rounded shape's flat
///   interior — visible (fill color).
/// - (b) `(62, 62)`: 1.5px in from the inner bbox's own top-left
///   corner `(60.5, 60.5)` — inside the OUTER rect's bounds, but
///   ~10.3px outside the inner shape's actual corner ARC (centered at
///   `(90.5, 90.5)`, radius 30) — must be clipped by the ROUNDED
///   mechanism specifically (a bbox-only approximation would
///   incorrectly show fill color here, since this point IS inside the
///   inner bbox and the outer rect both).
/// - (c) `(15, 15)`: outside the outer rect's bounds (`x0 = 30.5`)
///   entirely — must be clipped by the plain-rect `clip_rect`
///   mechanism, proving the outer level is independently enforced.
pub fn nested_rounded_and_rect_clip() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::PushClipRect { rect: Rect::new(30.5, 30.5, 226.5, 226.5), transform: Affine::IDENTITY });
    scene.push(DrawCommand::PushClipRoundedRect {
        rect: RoundedRect::new(60.5, 60.5, 196.5, 196.5, 30.0),
        transform: Affine::IDENTITY,
    });
    // Deliberately larger than BOTH the inner rounded bbox and the
    // outer rect — pokes past every side of both.
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(10.5, 10.5, 246.5, 246.5),
        radii: None,
        brush: solid(220, 60, 60, 255),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::PopClip); // inner rounded
    scene.push(DrawCommand::PopClip); // outer rect
    scene
}

/// Two overlapping translucent rects, same params in BOTH variants
/// below — the ONLY difference is whether they're wrapped in a blend
/// layer (design §6.3). Shared so `blend_layer_group`/
/// `blend_layer_group_reference_no_layer` are provably drawing
/// IDENTICAL content, isolating "wrapped in a layer or not" as the
/// only variable.
fn overlapping_pair(scene: &mut Scene) {
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(60.0, 60.0, 180.0, 180.0),
        radii: None,
        brush: solid(220, 30, 30, 200),
        transform: Affine::IDENTITY,
    });
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(110.0, 110.0, 230.0, 230.0),
        radii: None,
        brush: solid(30, 60, 220, 200),
        transform: Affine::IDENTITY,
    });
}

/// `overlapping_pair` wrapped in a `PushBlendLayer{alpha: 0.6}` (design
/// §6.3) — the grouped-composite case: content isolated a coherent
/// unit, THEN scaled by 0.6 as one single translucent group before
/// hitting the background, rather than each rect fading independently.
pub fn blend_layer_group() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::PushBlendLayer { mode: BlendMode::default(), alpha: 0.6, transform: Affine::IDENTITY });
    overlapping_pair(&mut scene);
    scene.push(DrawCommand::PopBlendLayer);
    scene
}

/// The SAME `overlapping_pair`, drawn directly with NO layer wrapper
/// and no alpha multiplier (design §6.3) — the reference scene the
/// isolation-differs tests compare against (each rect fades
/// independently against whatever's already drawn, rather than as one
/// grouped unit).
pub fn blend_layer_group_reference_no_layer() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    overlapping_pair(&mut scene);
    scene
}

// ── Wave 4 Commits 5+6: gradients, images, full affine, per-corner ──
// ── radii parity fixtures ────────────────────────────────────────────
//
// `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
// §9. All 7 reuse the SAME `.5`-sub-pixel-offset + single-shared-
// background conventions established above.

/// `FillRect`, `Brush::Gradient(Radial)`, CONCENTRIC (design §9's
/// "exact-agreement baseline case") — `Gradient::new_radial` sets
/// `start_center == end_center` and `start_radius == 0.0` by
/// construction, so `gradient_radial_focal_degraded` never fires here
/// (the concentric case IS the non-approximated exact math, §2.6).
/// `Extend::Pad` (peniko's default) — no spread-mode wrap boundary is
/// crossed inside the rect (the inscribed-circle radius keeps every
/// corner beyond `t == 1.0`, clamped flat by Pad), so this stays on the
/// BASE shape tolerance tier: both backends sample from byte-identical
/// LUT bytes (the shared `uzor-urx-core::gradient_lut` functions), so
/// agreement should be tight.
pub fn radial_gradient_rect() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let rect = Rect::new(69.5, 69.5, 187.5, 187.5);
    let radius = (rect.width().min(rect.height()) * 0.5) as f32;
    let gradient = Gradient::new_radial(rect.center(), radius).with_stops([
        (0.0f32, Color::from_rgba8(30, 60, 220, 255)),
        (1.0f32, Color::from_rgba8(230, 180, 30, 255)),
    ]);
    scene.push(DrawCommand::FillRect { rect, radii: None, brush: Brush::Gradient(gradient), transform: Affine::IDENTITY });
    scene
}

/// `FillRect`, `Brush::Gradient(Sweep)`, default FULL-CIRCLE span
/// (design §9 — "first-ever Sweep fixture in this harness"; CPU already
/// implemented Sweep, per §0.1c/§3, this proves GPU parity). Uses
/// `Extend::Repeat` (NOT the default `Pad`) — a full 0..2π span is
/// only a smooth, seamless spin under `Repeat`: `Pad` would clamp every
/// `atan2`-negative pixel (half the circle) to the start colour,
/// producing a visible hard seam rather than a genuine sweep. This
/// deliberately CROSSES the angle-wrap boundary (`atan2`'s own
/// -π/+π seam), so this fixture uses the widened `_GRADIENT` tolerance
/// tier (`tests/parity.rs`'s own doc comment) rather than the base
/// shape tier.
pub fn sweep_gradient_rect() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let rect = Rect::new(69.5, 69.5, 187.5, 187.5);
    let gradient = Gradient::new_sweep(rect.center(), 0.0, std::f32::consts::TAU)
        .with_extend(Extend::Repeat)
        .with_stops([
            (0.0f32, Color::from_rgba8(230, 60, 60, 255)),
            (1.0f32, Color::from_rgba8(60, 90, 230, 255)),
        ]);
    scene.push(DrawCommand::FillRect { rect, radii: None, brush: Brush::Gradient(gradient), transform: Affine::IDENTITY });
    scene
}

/// `StrokePath`, over the SAME star geometry as
/// `tessellated_path_star`'s own stroke star (`star_path(185.5, 140.5,
/// 45.0, 18.0)`), with a Radial `Brush::Gradient` instead of solid —
/// proves the "gradient StrokeRect/FillPath/StrokePath closes for
/// free" claim (design §2.5): the exact SAME `emit_gradient_mesh` path
/// `FillRect` already uses, fed a star-shaped stroke mesh instead of a
/// rect's.
///
/// **GPU-ONLY — no CPU comparison, a THIRD previously-undocumented
/// finding**: measuring this as an ordinary `ParityCase` first showed a
/// huge mismatch (`cpu=[230,60,60,255]`, exactly the gradient's first
/// stop; `native≈[113,80,177,255]`, a real interpolated colour) at the
/// SAME probe `tessellated_path_star`'s own solid-brush stroke star
/// uses. Direct read of `uzor-urx-cpu/src/backend.rs` confirms why:
/// `Line`, `StrokeRect` (non-radii), `FillPath`, and `StrokePath` ALL
/// resolve their brush via `brush_to_color` (a flat first-stop-or-solid
/// colour) UNCONDITIONALLY — `FillRect` is the ONLY `DrawCommand` whose
/// CPU arm ever calls `try_fill_rect_gradient`. So design §2.5's
/// "closes for free" claim describes a GPU-ONLY closure — CPU has
/// never rendered a real gradient on anything but `FillRect`, a gap
/// this design didn't discover (§0.1-§0.4's own defect list never
/// mentions it). Out of scope to fix (`uzor-urx-cpu` is off-limits) —
/// verified instead by
/// `tests/parity.rs::gradient_on_stroke_path_star_gpu_only_correctness`,
/// which hand-computes the expected interpolated colour at the SAME
/// probe point and asserts GPU alone produces it (matching the
/// measured value above almost exactly: `t = dist((190,110),
/// (185.5,140.5)) / 45 ≈ 0.685`, LUT index `round(0.685*255) = 175`,
/// straight-lerp — both stops fully opaque, so straight-lerp and
/// premultiplied-lerp coincide exactly, same convention as
/// `linear_gradient_rect` — gives `(113, 81, 177)`, matching the
/// measured `(113-115, 80, 175-177)` within ordinary AA/rounding
/// slack).
pub fn gradient_on_stroke_path_star() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let path = star_path(185.5, 140.5, 45.0, 18.0);
    let gradient = Gradient::new_radial((185.5, 140.5), 45.0).with_stops([
        (0.0f32, Color::from_rgba8(230, 60, 60, 255)),
        (1.0f32, Color::from_rgba8(60, 90, 230, 255)),
    ]);
    scene.push(DrawCommand::StrokePath {
        path,
        stroke: Stroke { width: 5.0, join: LineJoin::Round, ..Stroke::default() },
        brush: Brush::Gradient(gradient),
        transform: Affine::IDENTITY,
    });
    scene
}

/// Coordinator's 2026-07-25 fix — `FillPath` over a genuinely NON-RECT
/// shape (the same 5-point star generator as every other star fixture
/// above), filled with a 3-STOP Linear gradient. This is the exact
/// class of scene that surfaced BOTH halves of the closed bug:
/// `uzor-urx-cpu`'s `FillPath` arm previously flattened ANY gradient
/// brush to `brush_to_color`'s first stop unconditionally (`FillRect`
/// was the only command that ever rendered a real one); this native
/// pipeline's own Linear-gradient mesh emitter (`emit_gradient_mesh`,
/// closed since Wave 4 for the ROUTING) evaluated colour per-VERTEX
/// then let the rasteriser barycentric-interpolate it across each
/// triangle — mathematically exact for 2 stops, but silently WRONG
/// for 3+ (a rect's own 2-triangle tessellation carries no vertex at
/// any interior stop boundary, so the middle stop's colour never
/// appeared). Both are now fixed: CPU samples a real
/// [`crate::gradient::GradientSampler`]-equivalent per pixel, GPU
/// joined Radial/Sweep's per-FRAGMENT LUT pipeline — see
/// `uzor-urx-wgpu/src/encode.rs::transform_gradient_params`'s own doc
/// comment for the measured before/after. Both probes sit inside the
/// star's own 18px inner-radius inscribed disc (never near a concave
/// point or an AA edge), so this is a genuinely-agreeing-shape case —
/// base shape tolerance tier, same reasoning `radial_gradient_rect`'s
/// own doc comment already establishes for byte-identical LUT
/// sampling on both backends.
pub fn linear_gradient_fill_path_star() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let path = star_path(128.5, 128.5, 45.0, 18.0);
    let gradient = Gradient::new_linear((128.5, 88.5), (128.5, 168.5)).with_stops([
        (0.0f32, Color::from_rgba8(230, 60, 60, 255)),
        (0.5f32, Color::from_rgba8(60, 200, 60, 255)),
        (1.0f32, Color::from_rgba8(60, 90, 230, 255)),
    ]);
    scene.push(DrawCommand::FillPath { path, rule: FillRule::NonZero, brush: Brush::Gradient(gradient), transform: Affine::IDENTITY });
    scene
}

/// Radial counterpart of [`linear_gradient_fill_path_star`] — same
/// star shape, a concentric Radial `Brush::Gradient` on `FillPath`
/// instead. Radial's per-fragment routing was already correct on GPU
/// pre-fix (only Linear had the barycentric-interpolation defect); this
/// fixture exists to prove CPU's NEW `FillPath` gradient dispatch
/// (`try_fill_path_gradient`) handles every `GradientKind`, not just
/// Linear.
pub fn radial_gradient_fill_path_star() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let path = star_path(128.5, 128.5, 45.0, 18.0);
    let gradient = Gradient::new_radial((128.5, 128.5), 18.0).with_stops([
        (0.0f32, Color::from_rgba8(255, 255, 0, 255)),
        (1.0f32, Color::from_rgba8(0, 0, 0, 255)),
    ]);
    scene.push(DrawCommand::FillPath { path, rule: FillRule::NonZero, brush: Brush::Gradient(gradient), transform: Affine::IDENTITY });
    scene
}

/// `FillRect`, `radii: Some([4.0, 40.0, 4.0, 40.0])` (design's own
/// literal example, deliberately non-uniform — `top_left=4,
/// top_right=40, bottom_right=4, bottom_left=40` per kurbo's
/// `RoundedRectRadii::new` corner order), Solid brush — the
/// Quad-SDF-vs-Triangle-routing acceptance case (design §6.2/§9).
/// `top_right`'s 40px radius is the discriminating probe (see
/// `tests/parity.rs`): a point 8px diagonally in from that corner sits
/// ~45.25px from the TRUE arc center (outside the 40px radius,
/// background) but would sit well past a WRONG uniform approximation
/// using `top_left`'s tiny 4px radius (inside, fill) — a uniform
/// approximation gets this pixel visibly wrong. CPU renders true
/// per-corner radii via its own `RoundedRect`
/// (`uzor-urx-cpu/src/backend.rs:154-179`), so this is a
/// genuinely-agreeing-shape case — base shape tolerance tier. Sized
/// down in two steps (160x160 measured 2.06%; 140x140 measured 2.00%
/// exactly, still technically over since the check is a strict `>`;
/// 130x130 comfortably clears it) to fit the base tier; the
/// discriminating probe's own geometry is corner-relative, unaffected
/// by the overall rect size.
pub fn per_corner_radii_rect() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(63.5, 63.5, 193.5, 193.5),
        radii: Some([4.0, 40.0, 4.0, 40.0]),
        brush: solid(80, 160, 230, 255),
        transform: Affine::IDENTITY,
    });
    scene
}

/// `FillRect`, uniform `radii`, `transform` = 30-degree rotation about
/// the rect's own center (design §9 — the Quad-SDF rotation-path
/// acceptance case, §5.3). Sized down to 24x24 (from 34x34, then
/// measured again) — CPU bbox-approximates the rotated rect entirely
/// (design §0.3), so the "true rotated shape vs. CPU's larger
/// circumscribing bbox" mismatch area scales roughly quadratically with
/// the rect's side length `L`; shrinking helps, but a SECOND,
/// independent divergence source (the rotated edges are themselves
/// OBLIQUE — `fwidth`'s well-known AA-width overestimate at non-axis-
/// aligned angles, the SAME finding `lines_at_angles` already documents
/// for diagonal lines) scales only roughly linearly with `L` and
/// doesn't shrink away as fast — so even at 24x24 this fixture needs
/// the WIDENED `_ROTATED_BBOX` tolerance tier (`tests/parity.rs`'s own
/// doc comment), not the base shape tier design §9's tier table
/// originally assigned it (measured: 34x34 -> 3.31%, 24x24 -> 2.44%,
/// both over the 2% base budget; 24x24 fits comfortably under
/// `_ROTATED_BBOX`'s 3%).
///
/// **`radii: None` — a SECOND, previously-undocumented CPU defect found
/// while measuring this fixture**: a first attempt used `Some([6.0;
/// 4])` (design's literal "uniform radii" spec) and measured CPU
/// showing pure BACKGROUND at the shape's own dead center — not merely
/// a bbox-vs-true-shape corner mismatch, empty content entirely. Root
/// cause (direct read, `uzor-urx-cpu/src/rounded.rs::rounded_clip_to_mask`):
/// the rounded-clip MASK's own screen position is computed from ONLY
/// `(sx, sy)` (`coeffs[0]`/`coeffs[3]`) — the shear/rotation
/// off-diagonal coefficients are read ONLY to decide whether to count
/// `rounded_clip_rotated_degraded` (an EXISTING, already-instrumented
/// counter), never folded into the mask's own placement math. Under a
/// genuine rotation this places the mask FAR from where
/// `fill_rect_aa`'s OWN (correctly full-affine-bbox'd) fill actually
/// lands, so their intersection can end up empty — worse than "wrong
/// shape at the corners" (§0.3's documented Rect-fill finding), it's
/// "wrong position for radii'd content entirely." Out of scope to fix
/// (`uzor-urx-cpu` is off-limits) — this fixture avoids the interaction
/// by using `radii: None`, which still fully exercises the Quad-SDF
/// rotation path (`decompose_similarity` routing is independent of
/// radius value) without combining it with the separately-broken
/// rounded-mask-under-rotation mechanism.
///
/// Probes are restricted to the shape's INTERIOR ONLY (§0.3(ii)): both
/// backends are analytically forced to agree only where GPU's TRUE
/// rotated shape's interior lies (a strict subset of CPU's bbox), so
/// CPU/GPU-agreeing points must stay well inside that intersection —
/// see `tests/parity.rs` for the exact points and their derivation.
pub fn rotated_uniform_radius_rect() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let center = (128.5, 128.5);
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(116.5, 116.5, 140.5, 140.5),
        radii: None,
        brush: solid(220, 140, 40, 255),
        transform: Affine::rotate_about(30.0f64.to_radians(), center),
    });
    scene
}

/// Coordinator's Commit 5 addition — the parity-level regression proof
/// for design §0.2's stroke-width unification. Two shapes, BOTH under
/// a 2x scale transform, exercising BOTH width-carrying mechanisms:
/// a `StrokeRect` (Quad SDF — `border_width` no longer multiplied by
/// `scale` at all) and a `StrokePath` shaped as a plain rectangle
/// (Triangle pipeline — `tess_stroke_scaled`'s local-width pre-divide).
/// Both now render a DEVICE-CONSTANT `width: 4.0` border regardless of
/// the 2x scale, matching CPU's own semantic (`stroke.width` applied
/// AFTER transforming points, never itself scaled) — this fixture
/// would have FAILED before Wave 4 Commit 4 landed (GPU used to
/// multiply width by `scale` on TOP of the already-correct CPU
/// semantic, doubling the apparent border width to 8.0).
///
/// **Width MUST be even, not the more "obviously round" `3.0`** — a
/// first attempt used `3.0` and failed the whole-image budget with a
/// HARD (zero-antialiased) mismatch band, root-caused via a scanline
/// dump: the `.25`-local/`.5`-screen sub-pixel offset convention (this
/// module's own `solid_rects_axis_aligned` doc comment) keeps the
/// RECT's edges off the integer grid, but an ODD width's HALF-width
/// (`1.5`) lands the border BAND's OWN edges (`rect_edge ± half_width`
/// = `20.5 ± 1.5` = `19.0`/`22.0`) back EXACTLY on integer pixel
/// boundaries — recreating the identical worst-case grid-alignment
/// scenario the sub-pixel offset exists to avoid, just one derivation
/// step removed from the rect's own position. An EVEN width's
/// half-width is itself a `.5` value, so `20.5 ± 2.0` stays at `18.5`/
/// `22.5` — off the grid, restoring normal soft antialiasing on both
/// backends.
pub fn scaled_stroke_width() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);

    scene.push(DrawCommand::StrokeRect {
        rect: Rect::new(10.25, 10.25, 40.25, 40.25),
        radii: None,
        stroke: Stroke { width: 4.0, ..Stroke::default() },
        brush: solid(240, 210, 40, 255),
        transform: Affine::scale(2.0),
    });

    let mut rect_path = BezPath::new();
    rect_path.move_to((60.25, 10.25));
    rect_path.line_to((90.25, 10.25));
    rect_path.line_to((90.25, 40.25));
    rect_path.line_to((60.25, 40.25));
    rect_path.close_path();
    scene.push(DrawCommand::StrokePath {
        path: rect_path,
        stroke: Stroke { width: 4.0, join: LineJoin::Miter, ..Stroke::default() },
        brush: solid(40, 190, 190, 255),
        transform: Affine::scale(2.0),
    });

    scene
}

/// Build a small (`size`x`size`) synthetic checkerboard `ImageData` —
/// fully OPAQUE cells (sidesteps the premultiply-arithmetic-regime
/// divergence the same way `linear_gradient_rect`'s two opaque stops
/// do) — and register it via `uzor_urx_image::register_image`,
/// returning the fresh `ImageId`. The grid is deliberately
/// 3-cells-per-axis (`cell` px each, last cell absorbing any
/// remainder) so the image's own CENTER pixel lands solidly in the
/// MIDDLE cell, several pixels from any cell boundary in either
/// direction — this is the property `image_axis_aligned_and_rotated`'s
/// rotated-instance probe depends on (see that fixture's doc comment).
fn checkerboard_image(size: u32, cell: u32, color_a: [u8; 4], color_b: [u8; 4]) -> uzor_urx_core::scene::ImageId {
    let mut bytes = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let cx = (x / cell).min(2);
            let cy = (y / cell).min(2);
            let color = if (cx + cy) % 2 == 0 { color_a } else { color_b };
            let idx = ((y * size + x) * 4) as usize;
            bytes[idx..idx + 4].copy_from_slice(&color);
        }
    }
    let data = uzor_urx_image::ImageData::from_raw_premul(size, size, bytes).expect("size matches by construction");
    uzor_urx_image::register_image(data)
}

/// `DrawCommand::Image` — an axis-aligned instance (identity transform)
/// and a second instance rotated 45 degrees about its own center
/// (design §9), both sampling the SAME small synthetic checkerboard
/// registered via `uzor_urx_image::register_image`. Proves
/// `ImagePipeline`'s rotation path and the shared registry read
/// together — same shared-registry precedent as
/// `glyph_run_two_letters`'s `register_font` (exactly one call, the
/// resulting `ImageId` baked into this ONE `Scene`, both backends
/// resolve it against the SAME process-wide `uzor_urx_image` registry).
///
/// The axis-aligned instance upscales the 32x32 source 2x into a 64x64
/// `dest` (genuine bilinear stretching, not the degenerate 1:1 case) —
/// its own edges use the widened `_IMAGE` tolerance tier
/// (`tests/parity.rs`'s doc comment: bilinear-vs-1:1-fast-path
/// divergence). The rotated instance is deliberately SMALLER (32x32,
/// no upscale) to keep the CPU-bbox-vs-GPU-true-rotated-shape mismatch
/// area (design §0.3, same sizing tradeoff as
/// `rotated_uniform_radius_rect`) inside that same `_IMAGE` budget —
/// its ONLY safe probe is the dest's own dead center (see
/// `checkerboard_image`'s doc comment for why that's the one point
/// BOTH backends' mappings agree on regardless of rotation: a
/// center-preserving rotation maps center to center under ANY
/// reasonable UV parameterization, bbox-based or true-rotated-quad-based
/// alike).
pub fn image_axis_aligned_and_rotated() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    let image_id = checkerboard_image(32, 10, [200, 60, 60, 255], [60, 120, 200, 255]);

    // Axis-aligned, 2x upscale (32x32 source -> 64x64 dest).
    scene.push(DrawCommand::Image {
        src: image_id,
        src_rect: None,
        dest: Rect::new(20.0, 20.0, 84.0, 84.0),
        transform: Affine::IDENTITY,
    });

    // Rotated 45 degrees about its own center, no upscale (32x32 ->
    // 32x32) — kept small so the CPU-bbox mismatch area stays inside
    // the `_IMAGE` whole-image budget (design §0.3 sizing tradeoff).
    let rotated_center = (166.0, 166.0);
    scene.push(DrawCommand::Image {
        src: image_id,
        src_rect: None,
        dest: Rect::new(150.0, 150.0, 182.0, 182.0),
        transform: Affine::rotate_about(std::f64::consts::FRAC_PI_4, rotated_center),
    });

    scene
}

/// GPU-ONLY correctness fixture (design §0.3/§9, option (i)) — a
/// `FillRect` under a genuine shear transform (`decompose_similarity`
/// rejects it — columns not orthogonal — routing it through the
/// Triangle pipeline, §5.2). No CPU comparison exists for this scene:
/// CPU bbox-approximates ANY non-axis-aligned transform (§0.3), and a
/// sheared parallelogram's bbox differs from its true shape by MORE
/// than an AA-edge tolerance, structurally, not just at the edges.
/// Used ONLY by `tests/parity.rs::sheared_rect_gpu_only_correctness`,
/// which hand-computes the true parallelogram's corners and asserts
/// GPU paints exactly that shape, not the wider bbox.
pub fn sheared_rect_scene() -> Scene {
    let mut scene = Scene::new();
    push_background(&mut scene);
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(-20.0, -20.0, 20.0, 20.0),
        radii: None,
        brush: solid(220, 60, 60, 255),
        // (x, y) -> (x + 0.5*y + 128, y + 128) — a genuine shear (no
        // orthogonal columns) centered on the canvas.
        transform: Affine::new([1.0, 0.0, 0.5, 1.0, 128.0, 128.0]),
    });
    scene
}
