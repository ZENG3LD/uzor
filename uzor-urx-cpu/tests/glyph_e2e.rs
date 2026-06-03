//! End-to-end glyph render via DrawCommand::GlyphRun + CpuBackend
//! (feature `glyph`). Confirms wiring is honest, not a silent drop.

#![cfg(feature = "glyph")]

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::scene::{DrawCommand, Glyph, Scene};
use uzor_urx_cpu::{CpuBackend, Pixmap};
use uzor_urx_glyph::register_font;

#[test]
fn glyph_run_via_backend_paints_pixels() {
    let bytes = std::fs::read("../uzor-fonts/fonts/DejaVuSans.ttf").unwrap();
    let font = register_font(bytes).unwrap();

    let mut p = Pixmap::new(200, 80);
    // Solid background so we can detect text via blend, not transparency.
    let mut scene = Scene::new();
    scene.push(DrawCommand::FillRect {
        rect: Rect::new(0.0, 0.0, 200.0, 80.0),
        radii: None,
        brush: Brush::Solid(Color::rgba8(0, 0, 0, 255)),
        transform: Affine::IDENTITY,
    });
    // Render two glyphs over the black bg.
    scene.push(DrawCommand::GlyphRun {
        glyphs: vec![
            Glyph { glyph_id: 36, x: 10.0, y: 0.0 },
            Glyph { glyph_id: 37, x: 40.0, y: 0.0 },
        ],
        font,
        font_size: 32.0,
        brush: Brush::Solid(Color::rgba8(255, 255, 255, 255)),
        transform: Affine::translate((0.0, 50.0)),
    });
    CpuBackend::new().render(&scene, &mut p).unwrap();

    // Count non-black pixels — must be > 50 (glyph ink).
    let painted = p.pixels()
        .chunks_exact(4)
        .filter(|c| c[0] > 30 || c[1] > 30 || c[2] > 30)
        .count();
    assert!(painted > 50, "glyph run via backend must paint >50 pixels, got {}", painted);
}
