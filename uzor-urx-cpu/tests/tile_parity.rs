//! Bit-exact parity: tile pipeline vs scanline pipeline on the same
//! scene. This is THE invariant that lets us ship the tile path —
//! pixels must match byte-for-byte on any scene that doesn't trigger
//! the opaque-replacement optimisation. When opaque replacement IS
//! triggered, the buried layers are gone — but the FINAL pixel is
//! identical because those layers were completely covered.

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};

fn rect_cmd(x: f64, y: f64, w: f64, h: f64, rgba: [u8; 4]) -> DrawCommand {
    DrawCommand::FillRect {
        rect: Rect::new(x, y, x + w, y + h),
        radii: None,
        brush: Brush::Solid(Color::rgba8(rgba[0], rgba[1], rgba[2], rgba[3])),
        transform: Affine::IDENTITY,
    }
}

fn render_both(scene: &Scene, w: u32, h: u32) -> (Pixmap, Pixmap) {
    let mut a = Pixmap::new(w, h);
    let mut b = Pixmap::new(w, h);
    let backend = CpuBackend::new();
    backend.render(scene, &mut a).unwrap();
    uzor_urx_cpu::tile::render_tiled(scene, &mut b);
    (a, b)
}

fn assert_bit_exact(a: &Pixmap, b: &Pixmap, label: &str) {
    if a.pixels() == b.pixels() { return; }
    let mut diffs = 0usize;
    let mut first = usize::MAX;
    for (i, (x, y)) in a.pixels().iter().zip(b.pixels().iter()).enumerate() {
        if x != y {
            if first == usize::MAX { first = i; }
            diffs += 1;
        }
    }
    panic!(
        "tile != scanline ({}): {} byte diffs, first @ {} (pixel {}). scanline={:?} tile={:?}",
        label, diffs, first, first / 4,
        &a.pixels()[first..first+4.min(a.pixels().len()-first)],
        &b.pixels()[first..first+4.min(b.pixels().len()-first)],
    );
}

#[test]
fn opaque_overdraw_collapses_to_topmost() {
    // 5 fully-overlapping opaque rects of different colors. Scanline
    // path blends each in turn (final = topmost). Tile path: each
    // FillRect that fully covers the tile triggers cmd.clear() + bg
    // = topmost. Final pixels MUST match (both = topmost color).
    let mut s = Scene::new();
    s.push(rect_cmd(0.0, 0.0, 100.0, 60.0, [200, 50, 50, 255]));
    s.push(rect_cmd(0.0, 0.0, 100.0, 60.0, [50, 200, 50, 255]));
    s.push(rect_cmd(0.0, 0.0, 100.0, 60.0, [50, 50, 200, 255]));
    s.push(rect_cmd(0.0, 0.0, 100.0, 60.0, [200, 200, 50, 255]));
    s.push(rect_cmd(0.0, 0.0, 100.0, 60.0, [100, 100, 100, 255])); // topmost
    let (a, b) = render_both(&s, 100, 60);
    // Topmost must win in BOTH.
    let p_scan = a.get_pixel(50, 30);
    let p_tile = b.get_pixel(50, 30);
    assert_eq!(p_scan, [100, 100, 100, 255]);
    assert_eq!(p_tile, [100, 100, 100, 255]);
    assert_bit_exact(&a, &b, "opaque_overdraw");
}

#[test]
fn semi_transparent_stack_byte_identical() {
    // 3 semi-transparent rects of different colours overlapping.
    // No opaque-replacement triggers; tile path falls through to
    // replay-via-SIMD which IS the scanline path. Pixels must match.
    let mut s = Scene::new();
    s.push(rect_cmd(10.0, 10.0, 60.0, 40.0, [200,  50,  50, 180]));
    s.push(rect_cmd(20.0,  5.0, 60.0, 40.0, [ 50, 200,  50, 160]));
    s.push(rect_cmd(30.0, 15.0, 60.0, 40.0, [ 50,  50, 200, 140]));
    let (a, b) = render_both(&s, 100, 60);
    assert_bit_exact(&a, &b, "semi_transparent");
}

#[test]
fn sub_pixel_rects_byte_identical() {
    let mut s = Scene::new();
    s.push(rect_cmd(5.5, 10.3, 60.0, 40.5, [200, 100,  50, 220]));
    s.push(rect_cmd(7.2,  8.9, 50.5, 50.7, [ 50, 100, 200, 200]));
    let (a, b) = render_both(&s, 100, 60);
    assert_bit_exact(&a, &b, "sub_pixel");
}

#[test]
fn partial_tile_coverage_keeps_blending() {
    // An opaque rect that DOESN'T cover the whole tile must blend
    // properly, not bg-replace.
    let mut s = Scene::new();
    s.push(rect_cmd(0.0, 0.0, 100.0, 60.0, [200,  50,  50, 255])); // full opaque
    s.push(rect_cmd(20.0, 10.0, 40.0, 40.0, [ 50, 200,  50, 200])); // partial semi
    let (a, b) = render_both(&s, 100, 60);
    assert_bit_exact(&a, &b, "partial_coverage");
}

#[test]
fn empty_scene_clear_match() {
    let s = Scene::new();
    let (a, b) = render_both(&s, 50, 30);
    assert_bit_exact(&a, &b, "empty");
}

#[test]
fn full_scene_with_overdraw_pattern() {
    // Stress: many full-opaque rects + transparent overlay (mimics
    // a card grid with a translucent header).
    let mut s = Scene::new();
    for i in 0..20 {
        let x = (i % 5) as f64 * 200.0;
        let y = (i / 5) as f64 * 200.0;
        s.push(rect_cmd(x, y, 200.0, 200.0,
            [((i * 30 + 50) & 0xff) as u8, ((i * 60 + 100) & 0xff) as u8, ((i * 90 + 150) & 0xff) as u8, 255]));
    }
    // Semi-transparent overlay top half.
    s.push(rect_cmd(0.0, 0.0, 1000.0, 200.0, [0, 0, 0, 80]));
    let (a, b) = render_both(&s, 1000, 800);
    assert_bit_exact(&a, &b, "card_grid_with_overlay");
}
