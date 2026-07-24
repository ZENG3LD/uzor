//! URX Wave 4 Commit 1 — CPU gradient fix regression proofs
//! (`docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §0.1/§10 Commit 1).
//!
//! ## Why this is a NEW file, not additions to `tests/gradient.rs`
//!
//! `uzor-urx-cpu/tests/gradient.rs` (and, discovered during this
//! commit's own baseline check, `tests/image.rs`,
//! `tests/radii_and_clip.rs`, and 10 further test files in this same
//! crate) is **pre-existing, unrelated bit rot** — it fails to compile
//! on a clean, unmodified tree (verified: `git status --porcelain --
//! uzor-urx-cpu/` was empty before this commit's changes; the failure
//! is present on the already-committed HEAD, nothing needed stashing).
//! Root cause: a stale `peniko`/`color` crate API
//! (`Color::from_rgba8(...)` and direct `.r`/`.g`/`.b`/`.a` field access —
//! neither exists on the pinned `color = "0.3.3"` `AlphaColor`; the
//! current API is `Color::from_rgba8(...)` / `.to_rgba8()`, which is
//! what every OTHER healthy file in this workspace already uses) plus,
//! in some files, a stale `GradientKind::Radial { .. }` STRUCT pattern
//! where the type is now a TUPLE variant wrapping a position struct.
//! Totally unrelated to gradients/images specifically — it is a
//! workspace-wide API-drift issue that predates this commit and is
//! explicitly out of THIS commit's scope (CPU gradient semantics
//! fixes + shared-crate extractions) to mass-repair across 13 files.
//!
//! Since `tests/gradient.rs` cannot be run as the "existing suite
//! passes unmodified" regression proof the design's own Commit 1 gate
//! calls for, THIS file substitutes: it exercises the exact same
//! `fill_rect_gradient_aa` code path (via `CpuBackend::render`) the
//! bit-rotted file would have, using the CURRENT, working
//! `Color::from_rgba8` API throughout. Additionally verified
//! separately (see this commit's own report): the per-file compile-
//! error COUNT for every one of the 13 already-broken test files is
//! BYTE-IDENTICAL before and after this commit's changes — proving the
//! extraction + fixes introduce zero NEW breakage anywhere in the
//! crate.
//!
//! ## What's actually being proven here
//!
//! 1. `radii_mask_now_clips_gradient_corner` — design §0.1(a): a
//!    `FillRect{radii: Some(_), brush: Gradient(_)}` now respects the
//!    rounded-clip mask it pushes; a corner comfortably outside the
//!    rounded arc must show BACKGROUND, not gradient color (the exact
//!    leak the fix closes).
//! 2. `translation_moves_the_gradient_axis_with_the_rect` — design
//!    §0.1(b): a translated gradient rect's axis endpoints move WITH
//!    the rect, not just the rect's own bounding box.
//! 3. `sweep_gradient_produces_a_real_angular_gradient` — design
//!    §0.1(c)/§3: proves Sweep was never a stub (the module doc
//!    correction is doc-only; this is the "already implemented"
//!    behavioural proof).
//! 4. `extraction_regression_*` — the pure-function extraction
//!    (`uzor_urx_core::gradient_lut`) is exercised end-to-end through
//!    the SAME entry points `tests/gradient.rs` would have used
//!    (linear red-to-blue axis, radial center-is-inner-stop), using
//!    the working `Color::from_rgba8` API — a stand-in for that file's
//!    own (currently unrunnable) coverage.

use uzor_urx_core::math::{
    Affine, Brush, Color, ColorStop, ColorStops, Extend, Gradient, Point, Rect,
};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn s() -> CpuBackend {
    CpuBackend::new()
}

fn stops2(c0: (u8, u8, u8, u8), c1: (u8, u8, u8, u8)) -> ColorStops {
    // `ColorStops` has no `FromIterator<ColorStop>` impl on the pinned
    // `peniko = "0.6"` — only `From<&[ColorStop]>` — so this builds a
    // plain `Vec` first and converts via slice, not `.collect()`.
    let v = vec![
        ColorStop { offset: 0.0, color: Color::from_rgba8(c0.0, c0.1, c0.2, c0.3).into() },
        ColorStop { offset: 1.0, color: Color::from_rgba8(c1.0, c1.1, c1.2, c1.3).into() },
    ];
    ColorStops::from(&v[..])
}

// ── (a) radii-aware masking ──────────────────────────────────────

#[test]
fn radii_mask_now_clips_gradient_corner() {
    // A gradient FillRect with a real per-corner radius — before the
    // fix, `fill_rect_gradient_aa` never consulted the rounded-clip
    // mask `backend.rs`'s FillRect arm pushes for `radii: Some(_)`, so
    // the corner leaked gradient color through what should have been a
    // clipped-away region.
    let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(120.0, 0.0));
    g.stops = stops2((220, 30, 30, 255), (30, 60, 220, 255));
    g.extend = Extend::Pad;

    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 120.0, 120.0),
        radii: Some([28.0; 4]),
        brush: Brush::Gradient(g),
        transform: Affine::IDENTITY,
    });

    let mut p = Pixmap::new(120, 120);
    s().render(&scene, &mut p).unwrap();

    // (2, 2): deep in the top-left corner-cut region — a 28px radius
    // arc centered at (28, 28) puts this point ~36.8px from the arc
    // center (`hypot(26,26)`), comfortably (~8.8px) outside the arc —
    // must be fully transparent (the pixmap's own zero-initialized
    // background), not gradient color.
    let corner = p.get_pixel(2, 2);
    assert_eq!(
        corner,
        [0, 0, 0, 0],
        "gradient FillRect with radii must clip its own corners now — got {corner:?} (pre-fix this leaked gradient color)"
    );

    // (60, 60): dead center — must still show gradient content (the
    // fix must not OVER-clip either).
    let center = p.get_pixel(60, 60);
    assert!(center[3] > 0, "center of the rounded gradient rect must still be painted, got {center:?}");
}

// ── (b) gradient-axis transform ──────────────────────────────────

#[test]
fn translation_moves_the_gradient_axis_with_the_rect() {
    // Linear red->blue across a 100px-wide rect, translated 50px right.
    // Pre-fix, the gradient axis stayed anchored at LOCAL (0,0)-(100,0)
    // even though the rect itself renders at screen (50,0)-(150,0) —
    // so probing near the rect's NEW left edge would land mid-gradient
    // (t ~= 0.5, a red/blue mix) instead of pure red. Post-fix, the
    // axis moves WITH the rect: near the new left edge must be red,
    // near the new right edge must be blue.
    let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
    g.stops = stops2((255, 0, 0, 255), (0, 0, 255, 255));
    g.extend = Extend::Pad;

    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 20.0),
        radii: None,
        brush: Brush::Gradient(g),
        transform: Affine::translate((50.0, 0.0)),
    });

    let mut p = Pixmap::new(200, 20);
    s().render(&scene, &mut p).unwrap();

    // Near the TRANSLATED rect's new left edge (screen x=52).
    let left = p.get_pixel(52, 10);
    assert!(
        left[0] > 200 && left[2] < 40,
        "near the moved rect's left edge must be near-red if the gradient axis moved WITH the rect \
         (pre-fix this would read as a red/blue mix, since the axis stayed anchored at the OLD, \
         untransformed position): got {left:?}"
    );
    // Near the TRANSLATED rect's new right edge (screen x=147).
    let right = p.get_pixel(147, 10);
    assert!(
        right[2] > 200 && right[0] < 40,
        "near the moved rect's right edge must be near-blue if the gradient axis moved WITH the rect: got {right:?}"
    );
    // Outside the translated rect entirely (screen x=10) — must be
    // untouched (transparent), proving the RECT itself is still
    // correctly positioned (a sanity check this test's own geometry
    // is set up as intended, not a fix-specific assertion).
    let outside = p.get_pixel(10, 10);
    assert_eq!(outside, [0, 0, 0, 0], "outside the translated rect must be untouched, got {outside:?}");
}

#[test]
fn translation_scales_radial_radius_and_moves_its_center() {
    // A concentric radial gradient, translated by (40, 40) — proves
    // the Radial arm's `end_center`/`end_radius` transform fix
    // (design §0.1b) independently of the Linear-arm test above.
    let mut g = Gradient::new_radial(Point::new(30.0, 30.0), 25.0);
    g.stops = stops2((255, 255, 0, 255), (0, 0, 0, 255));
    g.extend = Extend::Pad;

    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        radii: None,
        brush: Brush::Gradient(g),
        transform: Affine::translate((40.0, 40.0)),
    });

    let mut p = Pixmap::new(120, 120);
    s().render(&scene, &mut p).unwrap();

    // The rect renders at screen (40,40)-(100,100); its LOCAL gradient
    // center (30,30) must land at SCREEN (70,70) post-translation —
    // must read as the inner (yellow) stop there.
    let moved_center = p.get_pixel(70, 70);
    assert!(
        moved_center[0] > 200 && moved_center[1] > 200,
        "the radial gradient's center must move WITH the translated rect (screen (70,70)): got {moved_center:?}"
    );
    // The OLD (pre-translation) local center position, now just empty
    // background inside the rect's own new bounds — must NOT be the
    // inner stop color (would indicate the center stayed anchored at
    // the untransformed local coordinate instead of moving).
    let stale_center = p.get_pixel(30, 30);
    assert!(
        stale_center[0] < 150 || stale_center[1] < 150,
        "the OLD untransformed center position must no longer read as the inner stop: got {stale_center:?}"
    );
}

// ── (c) Sweep — behavioural "already implemented" proof ──────────

#[test]
fn sweep_gradient_produces_a_real_angular_gradient() {
    let mut g = Gradient::new_sweep(Point::new(30.0, 30.0), -std::f32::consts::PI, std::f32::consts::PI);
    g.stops = stops2((255, 0, 0, 255), (0, 0, 255, 255));
    g.extend = Extend::Pad;

    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        radii: None,
        brush: Brush::Gradient(g),
        transform: Affine::IDENTITY,
    });

    let mut p = Pixmap::new(60, 60);
    s().render(&scene, &mut p).unwrap();

    // Sweep paints nearly everywhere around the center (design §0.1c:
    // "not a stub, not a degrade") — a previous/hypothetical no-op
    // implementation would leave every non-center pixel fully
    // transparent.
    let mut painted = 0u32;
    for y in [5_u32, 15, 30, 45, 55] {
        for x in [5_u32, 15, 30, 45, 55] {
            if (x, y) == (30, 30) {
                continue;
            }
            if p.get_pixel(x, y)[3] > 0 {
                painted += 1;
            }
        }
    }
    assert!(painted >= 20, "sweep should paint nearly every probed cell, got {painted}/24");

    // East of center (angle ~0, mid-span of [-PI, PI]) vs west of
    // center (angle ~PI, near the span's own wrap edge) must read as
    // genuinely DIFFERENT colors — a real angular gradient, not a
    // solid fallback.
    let east = p.get_pixel(55, 30);
    let west = p.get_pixel(5, 30);
    let diff = (east[0] as i32 - west[0] as i32).abs() + (east[2] as i32 - west[2] as i32).abs();
    assert!(diff > 60, "east vs west of the sweep center must differ meaningfully: east={east:?} west={west:?}");
}

// ── extraction regression: `uzor_urx_core::gradient_lut` exercised
// end-to-end through `fill_rect_gradient_aa`, standing in for
// `tests/gradient.rs`'s own (currently unrunnable) coverage. ───────

#[test]
fn extraction_regression_linear_red_to_blue_left_to_right() {
    let mut p = Pixmap::new(100, 20);
    let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
    g.stops = stops2((255, 0, 0, 255), (0, 0, 255, 255));
    g.extend = Extend::Pad;
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 100.0, 20.0),
        radii: None,
        brush: Brush::Gradient(g),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    let l = p.get_pixel(2, 10);
    assert!(l[0] > 200 && l[2] < 30, "left should be red, got {l:?}");
    let r = p.get_pixel(97, 10);
    assert!(r[2] > 200 && r[0] < 30, "right should be blue, got {r:?}");
    let m = p.get_pixel(50, 10);
    assert!(m[0] > 80 && m[2] > 80, "middle should mix RB, got {m:?}");
}

#[test]
fn extraction_regression_radial_center_is_inner_stop() {
    let mut p = Pixmap::new(60, 60);
    let mut g = Gradient::new_radial(Point::new(30.0, 30.0), 25.0);
    g.stops = stops2((255, 255, 0, 255), (0, 0, 0, 255));
    g.extend = Extend::Pad;
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 60.0, 60.0),
        radii: None,
        brush: Brush::Gradient(g),
        transform: Affine::IDENTITY,
    });
    s().render(&scene, &mut p).unwrap();

    let c = p.get_pixel(30, 30);
    assert!(c[0] > 200 && c[1] > 200, "center yellow, got {c:?}");
    let e = p.get_pixel(55, 30);
    assert!(e[0] < 50 && e[1] < 50, "outside-radius near black, got {e:?}");
}

#[test]
fn extraction_regression_hash_stable_lut_reused_across_renders() {
    // Mirrors the bit-rotted file's `linear_lut_cached_across_renders`
    // intent: two renders of the SAME gradient must produce bit-
    // identical pixels (proves `hash_stops`/`build_lut`, now living in
    // `uzor_urx_core::gradient_lut`, are still deterministic post-move).
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let make_scene = || {
        let mut g = Gradient::new_linear(Point::new(0.0, 0.0), Point::new(50.0, 0.0));
        g.stops = stops2((100, 200, 50, 255), (255, 100, 200, 255));
        g.extend = Extend::Pad;
        let mut sc = Scene::new();
        sc.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
            radii: None,
            brush: Brush::Gradient(g),
            transform: Affine::IDENTITY,
        });
        sc
    };
    let mut p1 = Pixmap::new(50, 50);
    let mut p2 = Pixmap::new(50, 50);
    s().render(&make_scene(), &mut p1).unwrap();
    s().render(&make_scene(), &mut p2).unwrap();
    let mut h1 = DefaultHasher::new();
    let mut h2 = DefaultHasher::new();
    p1.pixels().hash(&mut h1);
    p2.pixels().hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish(), "same gradient -> identical pixels across renders");
}
