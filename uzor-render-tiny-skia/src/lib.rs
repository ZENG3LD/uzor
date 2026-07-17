//! CPU software rasterizer backend using `tiny-skia` + `fontdue`.
//!
//! Provides a pure-Rust, zero-GPU rendering context that implements the
//! [`uzor::render::RenderContext`] trait via [`TinySkiaCpuRenderContext`].

mod context;

pub use context::TinySkiaCpuRenderContext;

// ---------------------------------------------------------------------------
// Phase 5 text_to_path spot-checks
// ---------------------------------------------------------------------------

#[cfg(test)]
mod text_to_path_tests {
    use uzor::render::TextMetrics;
    use super::TinySkiaCpuRenderContext;

    fn ctx() -> TinySkiaCpuRenderContext {
        TinySkiaCpuRenderContext::new(1, 1, 1.0)
    }

    /// Non-empty input → non-empty path starting with "M".
    #[test]
    fn hello_returns_path_starting_with_m() {
        let d = ctx().text_to_path("HELLO", "bold 48px sans-serif");
        assert!(!d.is_empty(), "text_to_path('HELLO') should return non-empty path");
        assert!(d.starts_with('M'), "path should start with M, got: {:?}", &d[..d.len().min(20)]);
    }

    /// Empty text → empty string (no panic).
    #[test]
    fn empty_text_returns_empty_string() {
        let d = ctx().text_to_path("", "16px sans-serif");
        assert!(d.is_empty(), "empty text should return empty path");
    }

    /// Path must end with "Z" (all subpaths closed).
    #[test]
    fn path_ends_with_z() {
        let d = ctx().text_to_path("Hi", "16px sans-serif");
        assert!(!d.is_empty(), "non-empty input should produce a path");
        assert!(d.ends_with('Z'), "path should end with Z, got: {:?}", &d[d.len().saturating_sub(20)..]);
    }

    /// Cache deduplication: two calls return identical strings.
    #[test]
    fn cache_returns_same_string() {
        let c = ctx();
        let d1 = c.text_to_path("AB", "24px sans-serif");
        let d2 = c.text_to_path("AB", "24px sans-serif");
        assert_eq!(d1, d2, "cached call should return identical path");
    }

    /// Different inputs produce different paths.
    #[test]
    fn different_text_different_path() {
        let c = ctx();
        let d_a = c.text_to_path("A", "24px sans-serif");
        let d_b = c.text_to_path("B", "24px sans-serif");
        assert_ne!(d_a, d_b, "different glyphs should produce different paths");
    }
}

// ---------------------------------------------------------------------------
// Fix A: fill_text advance double-scaling regression
// ---------------------------------------------------------------------------

#[cfg(test)]
mod fill_text_scale_tests {
    //! Regression coverage for the `fill_text` glyph-advance
    //! double-scaling bug: glyphs are rasterized at `render_px = px *
    //! sx.max(sy).max(1.0)` (already in DEVICE pixels for `sx >= 1`), but
    //! the pen used to re-multiply that already-scaled `advance_width` by
    //! `sx` AGAIN, so under `ctx.scale(2, 2)` every string rendered with
    //! glyphs at the correct 2x size but the pen walking 4x too far
    //! between them ("H o p 1"-style letter-spacing garbling — exactly
    //! what wrecked the PDF raster backgrounds rendered via
    //! `RASTER_SCALE = 2.0`). Proven directly against rendered pixels
    //! (design law 8: numbers, not just a visual eyeball) — no font/text
    //! internals are reached, only the public [`RenderContext`]/
    //! [`Painter`]/[`TextRenderer`] surface plus [`TinySkiaCpuRenderContext::pixmap`].

    use uzor::render::{Painter, TextAlign, TextBaseline, TextRenderer};
    use tiny_skia::Pixmap;

    use super::TinySkiaCpuRenderContext;

    const FONT: &str = "24px sans-serif";
    const BASELINE_X: f64 = 10.0;
    const BASELINE_Y: f64 = 40.0;

    /// Render `text` into a fresh `w x h` pixmap, scaled by `(sx, sy)`
    /// BEFORE painting (mirrors how `pages_to_pdf` scales its own raster
    /// pass), at the SAME logical baseline position/font size regardless
    /// of scale — the transform alone is responsible for the device-pixel
    /// size/position difference between two renders of this fn.
    fn render_scaled(text: &str, w: u32, h: u32, sx: f64, sy: f64) -> TinySkiaCpuRenderContext {
        let mut ctx = TinySkiaCpuRenderContext::new(w, h, 1.0);
        ctx.scale(sx, sy);
        ctx.set_fill_color("#000000");
        ctx.set_font(FONT);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Alphabetic);
        ctx.fill_text(text, BASELINE_X, BASELINE_Y);
        ctx
    }

    /// Whether column `x` holds ANY inked (non-transparent) pixel across
    /// the pixmap's full height.
    fn column_has_ink(pixmap: &Pixmap, x: u32) -> bool {
        let w = pixmap.width();
        let h = pixmap.height();
        let data = pixmap.data();
        (0..h).any(|y| data[((y * w + x) * 4 + 3) as usize] > 0)
    }

    /// `[min_x, max_x]` inked-column bounding box, `None` if nothing painted.
    fn ink_bbox_x(pixmap: &Pixmap) -> Option<(u32, u32)> {
        let w = pixmap.width();
        let mut min_x = None;
        let mut max_x = None;
        for x in 0..w {
            if column_has_ink(pixmap, x) {
                min_x.get_or_insert(x);
                max_x = Some(x);
            }
        }
        min_x.zip(max_x)
    }

    /// Widest run of consecutive INK-FREE columns strictly between
    /// `min_x` and `max_x` — the inter-word gap for a two-word string
    /// whose ink bbox is `[min_x, max_x]`.
    fn widest_interior_gap(pixmap: &Pixmap, min_x: u32, max_x: u32) -> u32 {
        let mut widest = 0u32;
        let mut current = 0u32;
        for x in min_x..=max_x {
            if column_has_ink(pixmap, x) {
                current = 0;
            } else {
                current += 1;
                widest = widest.max(current);
            }
        }
        widest
    }

    /// The core Fix A proof: the SAME string rendered once at `scale(1)`
    /// into a `w x h` pixmap and once at `scale(2)` into a `2w x 2h`
    /// pixmap must produce an ink bounding-box width that's ~2x, NOT ~4x.
    #[test]
    fn scale_2x_render_ink_width_is_about_2x_not_4x() {
        let text = "Hello World";

        let ctx1 = render_scaled(text, 200, 60, 1.0, 1.0);
        let (min1, max1) = ink_bbox_x(ctx1.pixmap()).expect("scale(1) render must paint some ink");
        let width1 = (max1 - min1 + 1) as f64;

        let ctx2 = render_scaled(text, 400, 120, 2.0, 2.0);
        let (min2, max2) = ink_bbox_x(ctx2.pixmap()).expect("scale(2) render must paint some ink");
        let width2 = (max2 - min2 + 1) as f64;

        assert!(
            (width2 - 2.0 * width1).abs() < 6.0,
            "scale(2) ink width must be ~2x the scale(1) width (a few px tolerance), \
             got width1={width1} width2={width2} (ratio={:.2})",
            width2 / width1
        );
        // Explicit regression guard against the exact double-scaling bug
        // this test exists to catch: a 4x-wide render would be the
        // OLD (broken) behavior.
        assert!(
            width2 < 3.0 * width1,
            "scale(2) ink width must not be ~4x the scale(1) width (the double-scaling bug) — \
             got width1={width1} width2={width2} (ratio={:.2})",
            width2 / width1
        );
    }

    /// Second assert (per Fix A's own gate): the INTER-WORD gap of a
    /// two-word string must also scale ~2x, not ~4x — proving the fix
    /// applies to a whitespace cluster's own advance exactly like any
    /// other glyph's, not just letters.
    #[test]
    fn scale_2x_render_inter_word_gap_scales_about_2x_not_4x() {
        let text = "Hello World";

        let ctx1 = render_scaled(text, 200, 60, 1.0, 1.0);
        let (min1, max1) = ink_bbox_x(ctx1.pixmap()).expect("scale(1) render must paint some ink");
        let gap1 = widest_interior_gap(ctx1.pixmap(), min1, max1);
        assert!(gap1 > 0, "fixture must have a visible inter-word gap at scale(1) to make this test meaningful");

        let ctx2 = render_scaled(text, 400, 120, 2.0, 2.0);
        let (min2, max2) = ink_bbox_x(ctx2.pixmap()).expect("scale(2) render must paint some ink");
        let gap2 = widest_interior_gap(ctx2.pixmap(), min2, max2);

        let gap1 = gap1 as f64;
        let gap2 = gap2 as f64;
        assert!(
            (gap2 - 2.0 * gap1).abs() < 6.0,
            "inter-word gap must scale ~2x, not ~4x — got gap1={gap1} gap2={gap2} (ratio={:.2})",
            gap2 / gap1
        );
    }
}

// ---------------------------------------------------------------------------
// Phase 4 cluster-shaping spot-checks
// ---------------------------------------------------------------------------

#[cfg(test)]
mod shaper_tests {
    use uzor::render::TextMetrics;
    use super::TinySkiaCpuRenderContext;

    fn ctx() -> TinySkiaCpuRenderContext {
        TinySkiaCpuRenderContext::new(1, 1, 1.0)
    }

    const FONT: &str = "16px sans-serif";

    /// "Hello" → 5 clusters, x_offset strictly increasing.
    #[test]
    fn hello_five_clusters() {
        let glyphs = ctx().measure_text_glyphs("Hello", FONT);
        assert_eq!(glyphs.len(), 5, "expected 5 clusters for 'Hello', got {:?}", glyphs);
        for (i, g) in glyphs.iter().enumerate() {
            let expected = ["H", "e", "l", "l", "o"][i];
            assert_eq!(g.cluster, expected, "cluster[{i}] mismatch");
        }
        // x_offsets must be non-decreasing
        for w in glyphs.windows(2) {
            assert!(
                w[1].x_offset >= w[0].x_offset,
                "x_offset not increasing: {:?} >= {:?}", w[0].x_offset, w[1].x_offset
            );
        }
    }

    /// "café" → 4 clusters, with 'é' as a single cluster (not 'e' + combining).
    #[test]
    fn cafe_four_clusters() {
        let glyphs = ctx().measure_text_glyphs("caf\u{00e9}", FONT);
        assert_eq!(glyphs.len(), 4, "expected 4 clusters for 'café', got {:?}", glyphs);
        assert_eq!(glyphs[3].cluster, "\u{00e9}", "last cluster should be é (U+00E9)");
    }

    /// "hi 👋" → 4 clusters: 'h', 'i', ' ', and the wave emoji as one cluster.
    #[test]
    fn emoji_one_cluster() {
        let text = "hi \u{1F44B}"; // U+1F44B = waving hand
        let glyphs = ctx().measure_text_glyphs(text, FONT);
        assert_eq!(glyphs.len(), 4, "expected 4 clusters for 'hi 👋', got {:?}", glyphs);
        assert_eq!(glyphs[3].cluster, "\u{1F44B}", "emoji should be one cluster");
    }

    /// Empty string → empty Vec.
    #[test]
    fn empty_string_empty_vec() {
        let glyphs = ctx().measure_text_glyphs("", FONT);
        assert!(glyphs.is_empty(), "empty string should return empty Vec");
    }
}

// ---------------------------------------------------------------------------
// ImagePainter::draw_image_rgba — real blit proof
// ---------------------------------------------------------------------------

#[cfg(test)]
mod image_blit_tests {
    //! Proves `draw_image_rgba` is a REAL raster blit (scale + transform +
    //! alpha composite), not the old documented no-op, and that the
    //! `RenderContext::image_painter()` capability accessor reaches this
    //! backend's own `ImagePainter` impl.

    use tiny_skia::Color;
    use uzor::render::{ImagePainter, Painter, RenderContext, ShapeHelpers};

    use super::TinySkiaCpuRenderContext;

    fn pixel(ctx: &TinySkiaCpuRenderContext, x: u32, y: u32) -> [u8; 4] {
        let w = ctx.width();
        let data = ctx.pixels();
        let idx = (y * w + x) as usize * 4;
        [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
    }

    fn assert_close(actual: [u8; 4], expected: [u8; 4], tol: i32, what: &str) {
        for i in 0..4 {
            let d = (actual[i] as i32 - expected[i] as i32).abs();
            assert!(
                d <= tol,
                "{what}: channel {i} mismatch — got {actual:?}, expected {expected:?} (tol {tol})"
            );
        }
    }

    /// `image_painter()` reaches this backend's `ImagePainter` impl through
    /// a `&mut dyn RenderContext` — the accessor this task adds to core.
    #[test]
    fn image_painter_accessor_reaches_the_tiny_skia_backend() {
        let mut ctx = TinySkiaCpuRenderContext::new(4, 4, 1.0);
        let dyn_ctx: &mut dyn RenderContext = &mut ctx;
        assert!(dyn_ctx.image_painter().is_some());
    }

    /// A 2x2 source (4 distinct opaque colors, row-major top-to-bottom)
    /// blitted into an 8x8 white target must reproduce each quadrant's own
    /// color. Sample points (1,1)/(6,1)/(1,6)/(6,6) sit strictly outside
    /// tiny-skia's bilinear interpolation zone for a 4x integer scale (that
    /// zone is exactly pixel columns/rows 2..6 of each 8px axis — the two
    /// texel centers land at device coordinate 2.0 and 6.0, so any pixel
    /// center below 2.0 or above 6.0 samples its own texel's color with
    /// zero blend after `SpreadMode::Pad` clamping), so this is an EXACT
    /// match, not just "close enough."
    #[test]
    fn blits_2x2_source_into_8x8_target_reproducing_quadrant_colors() {
        #[rustfmt::skip]
        let rgba: [u8; 16] = [
            255,   0,   0, 255,      0, 255,   0, 255, // row0: (0,0)=red   (1,0)=green
              0,   0, 255, 255,    255, 255,   0, 255, // row1: (0,1)=blue  (1,1)=yellow
        ];

        let mut ctx = TinySkiaCpuRenderContext::new(8, 8, 1.0);
        ctx.clear(Color::WHITE);
        ctx.draw_image_rgba(&rgba, 2, 2, 0.0, 0.0, 8.0, 8.0);

        assert_close(pixel(&ctx, 1, 1), [255, 0, 0, 255], 4, "top-left quadrant (texel 0,0 = red)");
        assert_close(pixel(&ctx, 6, 1), [0, 255, 0, 255], 4, "top-right quadrant (texel 1,0 = green)");
        assert_close(pixel(&ctx, 1, 6), [0, 0, 255, 255], 4, "bottom-left quadrant (texel 0,1 = blue)");
        assert_close(pixel(&ctx, 6, 6), [255, 255, 0, 255], 4, "bottom-right quadrant (texel 1,1 = yellow)");
    }

    /// Row scan at a y sitting inside both painted rects, counting pixels
    /// whose color reads as the painted solid red (vs. the white background).
    fn red_row_extent(ctx: &TinySkiaCpuRenderContext, y: u32) -> u32 {
        let w = ctx.width();
        (0..w)
            .filter(|&x| {
                let p = pixel(ctx, x, y);
                p[0] > 200 && p[1] < 80 && p[2] < 80
            })
            .count() as u32
    }

    /// `ctx.scale(2, 2)` must double the painted extent — the same
    /// transform every other draw call on this backend already applies
    /// (`translate`/`fill_rect`/`fill_text`), proven here for the image
    /// blit's own transform composition.
    #[test]
    fn scale_2x_doubles_the_painted_extent() {
        #[rustfmt::skip]
        let rgba: [u8; 16] = [
            255, 0, 0, 255,  255, 0, 0, 255,
            255, 0, 0, 255,  255, 0, 0, 255,
        ]; // solid opaque red 2x2 — no internal structure needed for this test

        let mut ctx1 = TinySkiaCpuRenderContext::new(20, 20, 1.0);
        ctx1.clear(Color::WHITE);
        ctx1.draw_image_rgba(&rgba, 2, 2, 2.0, 2.0, 4.0, 4.0);
        let width1 = red_row_extent(&ctx1, 5);
        assert_eq!(width1, 4, "unscaled 4x4 rect at row 5 must read exactly 4px wide, got {width1}");

        let mut ctx2 = TinySkiaCpuRenderContext::new(20, 20, 1.0);
        ctx2.clear(Color::WHITE);
        ctx2.scale(2.0, 2.0);
        ctx2.draw_image_rgba(&rgba, 2, 2, 2.0, 2.0, 4.0, 4.0);
        let width2 = red_row_extent(&ctx2, 5);

        assert_eq!(
            width2,
            2 * width1,
            "ctx.scale(2, 2) must double the painted extent, got {width1}px -> {width2}px"
        );
    }

    /// A semi-transparent (alpha=128) source pixel composited over an
    /// opaque, differently-colored background must land strictly between
    /// the two colors (real alpha-over), never fully overwriting the
    /// background and never leaving it untouched.
    #[test]
    fn semi_transparent_source_composites_not_overwrites() {
        let mut ctx = TinySkiaCpuRenderContext::new(4, 4, 1.0);
        ctx.set_fill_color("#0000ff");
        ctx.fill_rect(0.0, 0.0, 4.0, 4.0);

        let rgba: [u8; 4] = [255, 0, 0, 128]; // half-opaque red, straight alpha
        ctx.draw_image_rgba(&rgba, 1, 1, 0.0, 0.0, 4.0, 4.0);

        let p = pixel(&ctx, 2, 2);
        assert_ne!(p, [0, 0, 255, 255], "source alpha must not be dropped (pure background survived)");
        assert_ne!(p, [255, 0, 0, 255], "source alpha must be respected (source must not fully overwrite)");
        // Straight src (255,0,0,128) premultiplies to (128,0,0,128)
        // (255*128/255 == 128 exactly — the straight red channel is
        // already at max, so premultiplying only rescales by alpha, it
        // doesn't halve an already-mid channel). SourceOver over opaque
        // premultiplied dst=(0,0,255,255): out = src + dst*(1 - src_a/255)
        // = (128,0,0,128) + (0,0,255,255)*(127/255) = (128,0,127,255).
        assert_close(p, [128, 0, 127, 255], 10, "semi-transparent red over opaque blue");
    }
}
