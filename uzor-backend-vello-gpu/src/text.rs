//! Standalone text rendering helpers for vello scenes.
//!
//! These functions allow other crates (e.g. `chart-app-vello`) to render and
//! measure text without needing to construct a full [`VelloGpuRenderContext`].
//!
//! Both functions use the same Roboto font cache as `context.rs`.

use std::sync::Arc;

use vello::kurbo::Affine;
use vello::peniko::{Blob, Brush, Fill, FontData};
use vello::{Glyph, Scene};
use skrifa::{MetadataProvider, raw::{FileRef, FontRef}};

use vello::peniko::color::AlphaColor;
use vello::peniko::color::Srgb;

/// Public color type alias (same as the one in context.rs).
pub type Color = AlphaColor<Srgb>;

// ── Private font bytes (re-declared here to avoid cross-module statics) ──────
// We re-use the same bytes as context.rs — they will be deduplicated by the
// linker because both are `include_bytes!` of the same path.

static ROBOTO_REGULAR_T: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD_T: &[u8]    = include_bytes!("../fonts/Roboto-Bold.ttf");

use std::sync::OnceLock;

static CACHED_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_BOLD: OnceLock<FontData>    = OnceLock::new();

/// Return the cached [`FontData`] for the requested weight.
pub(crate) fn get_text_font(bold: bool) -> &'static FontData {
    if bold {
        CACHED_BOLD.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(ROBOTO_BOLD_T.to_vec())), 0)
        })
    } else {
        CACHED_REGULAR.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(ROBOTO_REGULAR_T.to_vec())), 0)
        })
    }
}

/// Convert a [`FontData`] reference to a skrifa [`FontRef`] for metric queries.
pub(crate) fn font_data_to_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(f) => Some(f),
        FileRef::Collection(c) => c.get(font.index).ok(),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Draw `text` into `scene` with the baseline positioned at `(x, y)`.
///
/// * `x`, `y`  — position in logical pixels; `y` is the text baseline.
/// * `font_size` — point size (e.g. `12.0`).
/// * `bold`    — select the Bold weight when `true`.
/// * `color`   — fill colour.
pub fn draw_text_to_scene(
    scene: &mut Scene,
    text: &str,
    x: f64,
    y: f64,
    font_size: f32,
    bold: bool,
    color: Color,
) {
    if text.is_empty() {
        return;
    }

    let font = get_text_font(bold);
    let font_ref = match font_data_to_ref(font) {
        Some(f) => f,
        None => return,
    };

    let size = skrifa::instance::Size::new(font_size);
    let var_loc = skrifa::instance::LocationRef::default();
    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(size, var_loc);

    let transform = Affine::translate((x, y));

    let mut pen_x = 0.0f32;
    scene
        .draw_glyphs(font)
        .font_size(font_size)
        .transform(transform)
        .brush(&Brush::Solid(color))
        .hint(true)
        .draw(
            Fill::NonZero,
            text.chars().map(|ch| {
                let gid = charmap.map(ch).unwrap_or_default();
                let advance = glyph_metrics.advance_width(gid).unwrap_or_default();
                let glyph_x = pen_x;
                pen_x += advance;
                Glyph {
                    id: gid.to_u32(),
                    x: glyph_x,
                    y: 0.0,
                }
            }),
        );
}

/// Measure the advance width of `text` in logical pixels at the given size.
///
/// Returns the total advance width. Falls back to `char_count * font_size * 0.6`
/// if the font cannot be loaded.
pub fn measure_text_width(text: &str, font_size: f32, bold: bool) -> f64 {
    let font = get_text_font(bold);
    let font_ref = match font_data_to_ref(font) {
        Some(f) => f,
        None => return text.len() as f64 * font_size as f64 * 0.6,
    };

    let size = skrifa::instance::Size::new(font_size);
    let var_loc = skrifa::instance::LocationRef::default();
    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(size, var_loc);

    let width: f32 = text
        .chars()
        .map(|ch| {
            let gid = charmap.map(ch).unwrap_or_default();
            glyph_metrics.advance_width(gid).unwrap_or_default()
        })
        .sum();

    width as f64
}
