//! Glyph rasteriser smoke test — loads DejaVuSans, rasterises a few
//! glyphs, composites them into a buffer, checks pixels.

use uzor_urx_core::scene::Glyph;
use uzor_urx_glyph::{draw_glyph_run, rasterise_glyph, register_font};

const FONT_PATH: &str = "../uzor-fonts/fonts/DejaVuSans.ttf";

fn load_font() -> uzor_urx_core::scene::FontId {
    let bytes = std::fs::read(FONT_PATH).expect("read DejaVuSans.ttf");
    register_font(bytes).expect("register font")
}

#[test]
fn rasterise_single_glyph_produces_bitmap() {
    let font = load_font();
    // Glyph id 36 in DejaVuSans corresponds to 'A' in most cmap tables.
    // We don't ship cmap here; pick any non-zero glyph id and verify it
    // produces a non-empty bitmap.
    let bm = rasterise_glyph(font, 36, 32.0, 0).expect("rasterise");
    assert!(bm.width > 0 && bm.height > 0, "bitmap dims must be > 0");
    assert_eq!(bm.alpha.len(), (bm.width * bm.height) as usize, "buffer size");
    // At least one pixel of mask coverage must be non-zero.
    let has_ink = bm.alpha.iter().any(|&v| v > 32);
    assert!(has_ink, "rasterised glyph must have ink");
}

#[test]
fn glyph_run_paints_pixels() {
    let font = load_font();
    let mut buf = vec![0u8; 200 * 60 * 4];
    // Render two glyphs spaced 20px apart, at baseline y=40.
    let glyphs = vec![
        Glyph { glyph_id: 36, x: 10.0, y: 0.0 },
        Glyph { glyph_id: 37, x: 40.0, y: 0.0 },
    ];
    draw_glyph_run(&mut buf, 200, 60, 0.0, 40.0, &glyphs, font, 32.0, [255, 255, 255, 255])
        .expect("draw glyph run");
    let painted: usize = buf.chunks_exact(4)
        .filter(|c| c[3] > 0)
        .count();
    assert!(painted > 50, "glyph run should paint >50 pixels, got {}", painted);
}

#[test]
fn cached_lookup_returns_same_arc_payload() {
    let font = load_font();
    let bm1 = rasterise_glyph(font, 36, 24.0, 0).unwrap();
    let bm2 = rasterise_glyph(font, 36, 24.0, 0).unwrap();
    // Same Arc means cache hit. Cache uses Arc<GlyphBitmap>.
    assert!(std::sync::Arc::ptr_eq(&bm1, &bm2), "cache hit must return same Arc");
}
