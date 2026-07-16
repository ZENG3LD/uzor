//! [`draw_layout`] — paint a [`ParagraphLayout`] via existing paint
//! primitives (design law 6: no new drawing primitive for text).

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::layout::ParagraphLayout;

/// Fill every glyph of `layout` at `origin`, translated by each glyph's own
/// already-measured `(x, y)` — one `fill_text` call per cluster, positions
/// come straight from `layout` (design law 1: one measure path, never a
/// second ad hoc width/position formula).
///
/// Forces `TextAlign::Left` + `TextBaseline::Alphabetic` on `ctx` before
/// painting (glyph `y` is already an alphabetic-baseline absolute
/// position — any other baseline state would double-offset it). Does
/// **not** call `ctx.set_font` — `layout`'s positions were measured
/// against a specific [`crate::model::FontSpec`]; the caller must have
/// already set that same font on `ctx` (Phase 1's `GlyphLayout` carries no
/// per-glyph font to read one back from — single font per paragraph).
pub fn draw_layout(ctx: &mut dyn RenderContext, origin: (f64, f64), layout: &ParagraphLayout, color: &str) {
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Alphabetic);
    ctx.set_fill_color(color);

    for glyph in &layout.glyphs {
        if glyph.cluster.is_empty() {
            continue;
        }
        ctx.fill_text(&glyph.cluster, origin.0 + glyph.x, origin.1 + glyph.y);
    }
}

#[cfg(test)]
mod tests {
    //! Headless proof: render a wrapped paragraph (Left) and the same
    //! paragraph Center-aligned via `uzor-export`, at a fixed resolution
    //! with a deterministic (seeded, no RNG/time) sample paragraph, and
    //! write both to `uzor/out/` for a human to eyeball (design law 8 — one
    //! screenshot is never proof, hence the Left/Center pair).

    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_export::{render_to_png, ExportSpec};

    use super::draw_layout;
    use crate::layout::{align_lines, layout_text, Align};
    use crate::model::FontSpec;
    use crate::shape::CosmicShaper;

    const WIDTH: u32 = 500;
    const HEIGHT: u32 = 300;
    const MARGIN: f64 = 20.0;

    /// Fixed seeded sample paragraph — no lorem-ipsum RNG (design law 8).
    const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog and \
        then keeps running further down the road without stopping, a fixed \
        seeded sample paragraph used for every uzor-text Phase 1 proof render.";

    fn out_dir() -> PathBuf {
        // Fixed path — `uzor/out/` is the shared human-eyeball drop point
        // for every headless proof render in this workspace (matches
        // uzor-viz's `proof_tests::out_dir`).
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    #[test]
    fn wrapped_paragraph_left_aligned_renders_to_a_valid_png() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = (WIDTH as f64) - 2.0 * MARGIN;

        let layout = layout_text(PARAGRAPH, &font, max_width, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            ctx.set_font(&font.to_css_font());
            draw_layout(ctx, (MARGIN, 40.0), &layout, "#111111");
        })
        .expect("left-aligned proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("text_p1_wrap_left.png", &bytes);
    }

    #[test]
    fn wrapped_paragraph_center_aligned_renders_to_a_valid_png() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let max_width = (WIDTH as f64) - 2.0 * MARGIN;

        let mut layout = layout_text(PARAGRAPH, &font, max_width, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        align_lines(&mut layout, max_width, Align::Center);

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            ctx.set_font(&font.to_css_font());
            draw_layout(ctx, (MARGIN, 40.0), &layout, "#111111");
        })
        .expect("center-aligned proof render should succeed");

        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("text_p1_wrap_center.png", &bytes);
    }
}
