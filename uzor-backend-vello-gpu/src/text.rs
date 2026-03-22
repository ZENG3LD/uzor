//! Standalone text rendering helpers for vello scenes.
//!
//! These functions allow other crates (e.g. `chart-app-vello`) to render and
//! measure text without needing to construct a full [`VelloGpuRenderContext`].
//!
//! Both functions use the same Roboto font cache as `context.rs`, with
//! Unicode fallback to NotoSansSymbols2 and NotoEmoji for characters that
//! Roboto does not cover.

use std::sync::Arc;

use vello::kurbo::Affine;
use vello::peniko::{Blob, Brush, Fill, FontData};
use vello::{Glyph, Scene};
use skrifa::{MetadataProvider, raw::{FileRef, FontRef}};

use vello::peniko::color::AlphaColor;
use vello::peniko::color::Srgb;

use std::sync::OnceLock;

/// Public color type alias (same as the one in context.rs).
pub type Color = AlphaColor<Srgb>;

// ── Private font bytes (re-declared here to avoid cross-module statics) ──────
// We re-use the same bytes as context.rs — they will be deduplicated by the
// linker because both are `include_bytes!` of the same path.

static ROBOTO_REGULAR_T: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD_T: &[u8]    = include_bytes!("../fonts/Roboto-Bold.ttf");

static NOTO_SYMBOLS2_T: &[u8] = include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf");
static NOTO_EMOJI_T: &[u8]    = include_bytes!("../fonts/NotoEmoji-Regular.ttf");

static CACHED_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_BOLD: OnceLock<FontData>    = OnceLock::new();

static CACHED_FALLBACK_SYMBOLS2: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_EMOJI: OnceLock<FontData>    = OnceLock::new();

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

/// Return fallback fonts in priority order: [NotoSansSymbols2, NotoEmoji].
fn get_fallback_fonts() -> &'static [FontData] {
    static FALLBACK_LIST: OnceLock<Vec<FontData>> = OnceLock::new();
    FALLBACK_LIST.get_or_init(|| {
        let s2 = CACHED_FALLBACK_SYMBOLS2.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(NOTO_SYMBOLS2_T.to_vec())), 0)
        });
        let em = CACHED_FALLBACK_EMOJI.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(NOTO_EMOJI_T.to_vec())), 0)
        });
        vec![s2.clone(), em.clone()]
    })
}

/// Convert a [`FontData`] reference to a skrifa [`FontRef`] for metric queries.
pub(crate) fn font_data_to_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(f) => Some(f),
        FileRef::Collection(c) => c.get(font.index).ok(),
    }
}

// ── Internal resolved glyph type ─────────────────────────────────────────────

struct ResolvedGlyph {
    /// `None` = primary font; `Some(i)` = fallback index i.
    font_index: Option<usize>,
    glyph_id: u32,
    x: f32,
    advance: f32,
}

/// Resolve characters to glyphs with Unicode fallback.
fn resolve_glyphs(
    text: &str,
    primary_ref: &FontRef<'_>,
    font_size: f32,
) -> Vec<ResolvedGlyph> {
    let size = skrifa::instance::Size::new(font_size);
    let var_loc = skrifa::instance::LocationRef::default();
    let primary_charmap = primary_ref.charmap();
    let primary_metrics = primary_ref.glyph_metrics(size, var_loc);
    let fallbacks = get_fallback_fonts();

    let mut pen_x = 0.0f32;
    let mut result = Vec::with_capacity(text.len());

    for ch in text.chars() {
        let primary_gid = primary_charmap.map(ch).unwrap_or_default();
        if primary_gid != skrifa::GlyphId::new(0) {
            let adv = primary_metrics.advance_width(primary_gid).unwrap_or_default();
            result.push(ResolvedGlyph {
                font_index: None,
                glyph_id: primary_gid.to_u32(),
                x: pen_x,
                advance: adv,
            });
            pen_x += adv;
        } else {
            let mut found_index = None;
            let mut found_gid = primary_gid;
            let mut found_adv = primary_metrics.advance_width(primary_gid).unwrap_or_default();

            for (idx, fb_font) in fallbacks.iter().enumerate() {
                if let Some(fb_ref) = font_data_to_ref(fb_font) {
                    let fb_gid = fb_ref.charmap().map(ch).unwrap_or_default();
                    if fb_gid != skrifa::GlyphId::new(0) {
                        let fb_metrics = fb_ref.glyph_metrics(size, var_loc);
                        found_adv = fb_metrics.advance_width(fb_gid).unwrap_or_default();
                        found_gid = fb_gid;
                        found_index = Some(idx);
                        break;
                    }
                }
            }

            result.push(ResolvedGlyph {
                font_index: found_index,
                glyph_id: found_gid.to_u32(),
                x: pen_x,
                advance: found_adv,
            });
            pen_x += found_adv;
        }
    }

    result
}

/// Total advance width from resolved glyphs.
fn total_width(glyphs: &[ResolvedGlyph]) -> f32 {
    glyphs.last().map_or(0.0, |g| g.x + g.advance)
}

/// Emit resolved glyphs to scene, one draw call per contiguous same-font run.
fn draw_resolved(
    scene: &mut Scene,
    glyphs: &[ResolvedGlyph],
    primary_font: &FontData,
    fallbacks: &[FontData],
    font_size: f32,
    transform: Affine,
    color: Color,
) {
    if glyphs.is_empty() {
        return;
    }
    let brush = Brush::Solid(color);
    let mut i = 0;

    while i < glyphs.len() {
        let run_font_index = glyphs[i].font_index;
        let run_start = i;

        while i < glyphs.len() && glyphs[i].font_index == run_font_index {
            i += 1;
        }

        let run = &glyphs[run_start..i];
        let font = match run_font_index {
            None => primary_font,
            Some(idx) if idx < fallbacks.len() => &fallbacks[idx],
            _ => primary_font,
        };

        scene
            .draw_glyphs(font)
            .font_size(font_size)
            .transform(transform)
            .brush(&brush)
            .hint(true)
            .draw(
                Fill::NonZero,
                run.iter().map(|g| Glyph {
                    id: g.glyph_id,
                    x: g.x,
                    y: 0.0,
                }),
            );
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Draw `text` into `scene` with the baseline positioned at `(x, y)`.
///
/// * `x`, `y`  — position in logical pixels; `y` is the text baseline.
/// * `font_size` — point size (e.g. `12.0`).
/// * `bold`    — select the Bold weight when `true`.
/// * `color`   — fill colour.
///
/// Characters not covered by Roboto are rendered using the NotoSansSymbols2
/// or NotoEmoji fallback fonts.
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

    let primary_font = get_text_font(bold);
    let primary_ref = match font_data_to_ref(primary_font) {
        Some(f) => f,
        None => return,
    };

    let resolved = resolve_glyphs(text, &primary_ref, font_size);
    let transform = Affine::translate((x, y));
    let fallbacks = get_fallback_fonts();

    draw_resolved(scene, &resolved, primary_font, fallbacks, font_size, transform, color);
}

/// Measure the advance width of `text` in logical pixels at the given size.
///
/// Returns the total advance width. Falls back to `char_count * font_size * 0.6`
/// if the font cannot be loaded.  Uses the Unicode fallback chain for
/// characters not covered by Roboto.
pub fn measure_text_width(text: &str, font_size: f32, bold: bool) -> f64 {
    let primary_font = get_text_font(bold);
    let primary_ref = match font_data_to_ref(primary_font) {
        Some(f) => f,
        None => return text.len() as f64 * font_size as f64 * 0.6,
    };

    let resolved = resolve_glyphs(text, &primary_ref, font_size);
    total_width(&resolved) as f64
}
