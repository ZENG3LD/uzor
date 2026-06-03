//! SIMD vs scalar pixel-bit-exact parity.
//!
//! This is the canonical "no SIMD math drift" test. If the assertions
//! here fail, the rest of the URX architecture is meaningless because
//! SIMD silently corrupts pixels under load.
//!
//! Strategy:
//!   * Build a deterministic scene with all the awkward edge cases
//!     (sub-pixel alignment, partial coverage, semi-transparent fills).
//!   * Render with the SIMD path (current default).
//!   * Render with the scalar path (forced by feeding a non-rect clip,
//!     which bypasses the SIMD fast path in fill_rect_aa).
//!   * Diff. Must be ZERO byte differences.

use uzor_urx_core::math::{Affine, Brush, Color, Rect, RoundedRect, RoundedRectRadii};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn make_scene(force_scalar: bool) -> Scene {
    let mut s = Scene::new();
    if force_scalar {
        // Push a max-extent rounded clip. ClipStack::all_rect() returns
        // false, fill_rect_aa takes the use_mask scalar branch.
        s.push(DrawCommand::PushClipRoundedRect {
            rect: RoundedRect::from_rect(
                Rect::new(0.0, 0.0, 200.0, 80.0),
                RoundedRectRadii::new(0.0, 0.0, 0.0, 0.0),
            ),
            transform: Affine::IDENTITY,
        });
    }
    // Edge case 1: rect on integer grid, opaque.
    s.push(DrawCommand::FillRect {
        rect: Rect::new(5.0, 5.0, 50.0, 50.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    // Edge case 2: rect with sub-pixel edges, half-alpha.
    s.push(DrawCommand::FillRect {
        rect: Rect::new(60.5, 10.3, 95.7, 60.9),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 255, 0, 128)),
        transform: Affine::IDENTITY,
    });
    // Edge case 3: rect overlapping prior fills (blend on top).
    s.push(DrawCommand::FillRect {
        rect: Rect::new(40.0, 30.0, 90.0, 60.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 100, 200, 180)),
        transform: Affine::IDENTITY,
    });
    // Edge case 4: thin sliver < 1 px tall.
    s.push(DrawCommand::FillRect {
        rect: Rect::new(100.0, 70.5, 180.0, 70.9),
        radii: None,
        brush: Brush::Solid(Color::rgba8(255, 255, 255, 200)),
        transform: Affine::IDENTITY,
    });
    if force_scalar {
        s.push(DrawCommand::PopClip);
    }
    s
}

#[test]
fn simd_path_bit_identical_to_scalar() {
    let backend = CpuBackend::new();
    let mut simd_pix   = Pixmap::new(200, 80);
    let mut scalar_pix = Pixmap::new(200, 80);
    backend.render(&make_scene(false), &mut simd_pix).unwrap();
    backend.render(&make_scene(true),  &mut scalar_pix).unwrap();

    let s = simd_pix.pixels();
    let r = scalar_pix.pixels();
    if s == r {
        return; // happy path
    }

    // Report the first diff (test fails AFTER detailed output).
    let mut diffs = 0usize;
    let mut first_diff_idx = 0usize;
    for (i, (a, b)) in s.iter().zip(r.iter()).enumerate() {
        if a != b {
            if diffs == 0 { first_diff_idx = i; }
            diffs += 1;
        }
    }
    let pixel_idx = first_diff_idx / 4;
    let px = pixel_idx % 200;
    let py = pixel_idx / 200;
    panic!(
        "SIMD vs scalar diverged: {} byte diffs across {} bytes; first diff at byte {} \
         (pixel ({}, {})). simd[i..i+4] = {:?}, scalar[i..i+4] = {:?}",
        diffs, s.len(), first_diff_idx, px, py,
        &s[first_diff_idx.. (first_diff_idx + 4).min(s.len())],
        &r[first_diff_idx.. (first_diff_idx + 4).min(r.len())],
    );
}
