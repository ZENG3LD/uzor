//! Font loading, CSS font string parsing, and text metrics for vello backends.
//!
//! Embeds all four Roboto variants (regular, bold, italic, bold-italic) plus
//! four Unicode fallback fonts, and caches them as `peniko::FontData` objects
//! so they are only heap-allocated once per process.
//!
//! ## Fallback chain
//!
//! When a character maps to `GlyphId(0)` in the primary font, `layout_glyphs`
//! and `measure_text` try each fallback font in order:
//! 1. `SymbolsNerdFontMono` — Powerline + Nerd Font PUA + dev icons
//! 2. `NotoSansSymbols2`   — math / arrows / wide symbol coverage
//! 3. `NotoColorEmoji`     — color emoji (COLRv1/v0 + CBDT bitmaps); vello-gpu renders color
//! 4. `NotoEmoji`          — legacy monochrome emoji (works on all backends)
//!
//! The first font that returns a non-zero glyph ID is used for that character.

use std::sync::{Arc, OnceLock};

use vello::peniko::{Blob, FontData};
use skrifa::{MetadataProvider, raw::{FileRef, FontRef}};
use uzor::fonts;

// ---------------------------------------------------------------------------
// Cached FontData (created once, reused forever)
// ---------------------------------------------------------------------------

static CACHED_FONT_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_ITALIC: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

static CACHED_FONT_JB_MONO_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_JB_MONO_BOLD: OnceLock<FontData> = OnceLock::new();

static CACHED_FONT_PT_ROOT_UI: OnceLock<FontData> = OnceLock::new();

static CACHED_FALLBACK_NERD_FONT: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_SYMBOLS2: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_COLOR_EMOJI: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_EMOJI: OnceLock<FontData> = OnceLock::new();

/// Re-export of the backend-agnostic family enum from core uzor.
pub use uzor::fonts::FontFamily;

/// Return a `&'static FontData` for the given family + style combination.
///
/// Delegates family → bytes resolution to `uzor::fonts::font_bytes` and
/// caches the decoded `peniko::FontData` locally so each slot is constructed
/// at most once per process.
pub fn get_cached_font_family(
    family: FontFamily,
    bold: bool,
    italic: bool,
) -> &'static FontData {
    match family {
        FontFamily::PtRootUi => CACHED_FONT_PT_ROOT_UI.get_or_init(|| {
            FontData::new(
                Blob::new(Arc::new(fonts::font_bytes(family, bold, italic).to_vec())),
                0,
            )
        }),
        FontFamily::JetBrainsMono => {
            let _ = italic; // no italic variant bundled
            if bold {
                CACHED_FONT_JB_MONO_BOLD.get_or_init(|| {
                    FontData::new(
                        Blob::new(Arc::new(
                            fonts::font_bytes(family, true, false).to_vec(),
                        )),
                        0,
                    )
                })
            } else {
                CACHED_FONT_JB_MONO_REGULAR.get_or_init(|| {
                    FontData::new(
                        Blob::new(Arc::new(
                            fonts::font_bytes(family, false, false).to_vec(),
                        )),
                        0,
                    )
                })
            }
        }
        FontFamily::Roboto => get_cached_font(bold, italic),
    }
}

/// Return a `&'static FontData` for the Roboto variant matching `(bold, italic)`.
///
/// The `FontData` is lazily initialised the first time it is needed and then
/// stored permanently via `OnceLock`, so it is safe to call from any thread.
pub fn get_cached_font(bold: bool, italic: bool) -> &'static FontData {
    match (bold, italic) {
        (true, true) => CACHED_FONT_BOLD_ITALIC.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::ROBOTO_BOLD_ITALIC.to_vec())), 0)
        }),
        (true, false) => CACHED_FONT_BOLD.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::ROBOTO_BOLD.to_vec())), 0)
        }),
        (false, true) => CACHED_FONT_ITALIC.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::ROBOTO_ITALIC.to_vec())), 0)
        }),
        (false, false) => CACHED_FONT_REGULAR.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::ROBOTO_REGULAR.to_vec())), 0)
        }),
    }
}

/// Return the list of fallback `FontData` in priority order.
///
/// Each entry is tried in sequence when the primary font returns `GlyphId(0)`.
/// The slice is `&'static` — the `FontData` objects are heap-allocated once
/// per process.
pub fn get_cached_fallback_fonts() -> &'static [FontData] {
    // All four entries must be initialised before we return the slice.  We
    // store them in a single OnceLock<Vec> so the slice lifetime is correct.
    static FALLBACK_LIST: OnceLock<Vec<FontData>> = OnceLock::new();
    FALLBACK_LIST.get_or_init(|| {
        let nerd = CACHED_FALLBACK_NERD_FONT.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::SYMBOLS_NERD_FONT_MONO.to_vec())), 0)
        });
        let symbols2 = CACHED_FALLBACK_SYMBOLS2.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::NOTO_SANS_SYMBOLS2.to_vec())), 0)
        });
        let colrv1 = CACHED_FALLBACK_COLOR_EMOJI.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::NOTO_COLOR_EMOJI.to_vec())), 0)
        });
        let emoji = CACHED_FALLBACK_EMOJI.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(fonts::NOTO_EMOJI.to_vec())), 0)
        });
        // Clone the FontData — peniko FontData is an Arc wrapper so this is cheap.
        vec![nerd.clone(), symbols2.clone(), colrv1.clone(), emoji.clone()]
    })
}

/// Convert a `peniko::FontData` reference into a `skrifa::raw::FontRef` for
/// querying metrics and glyph advances.
///
/// Returns `None` if the font data cannot be parsed by skrifa.
pub fn to_font_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(f) => Some(f),
        FileRef::Collection(collection) => collection.get(font.index).ok(),
    }
}

// ---------------------------------------------------------------------------
// CSS font string parsing
// ---------------------------------------------------------------------------

/// Parsed representation of a CSS font shorthand string such as
/// `"bold 14px Roboto"` or `"italic 11.5px sans-serif"`.
#[derive(Clone, Debug)]
pub struct FontInfo {
    /// Font size in logical pixels.
    pub size: f64,
    /// Whether bold style is requested.
    pub bold: bool,
    /// Whether italic style is requested.
    pub italic: bool,
    /// Font family name (everything that is not a recognised keyword or size).
    pub family: String,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            size: 12.0,
            bold: false,
            italic: false,
            family: String::from("Roboto"),
        }
    }
}

/// Parse a CSS font shorthand string into a [`FontInfo`].
///
/// Recognised tokens (case-insensitive):
/// - `"bold"` — sets `bold = true`
/// - `"italic"` — sets `italic = true`
/// - `"<N>px"` — sets `size = N`
/// - Everything else is collected as the font family name.
///
/// Unrecognised or empty strings produce [`FontInfo::default`].
pub fn parse_css_font(font_str: &str) -> FontInfo {
    let mut info = FontInfo::default();
    info.family.clear();

    let lower = font_str.to_lowercase();
    let mut family_parts: Vec<&str> = Vec::new();

    for part in lower.split_whitespace() {
        if part == "bold" {
            info.bold = true;
        } else if part == "italic" {
            info.italic = true;
        } else if part.ends_with("px") {
            if let Ok(size) = part.trim_end_matches("px").parse::<f64>() {
                info.size = size;
            }
        } else {
            // Collect as family name (preserve case from original string if possible)
            family_parts.push(part);
        }
    }

    if !family_parts.is_empty() {
        info.family = family_parts.join(" ");
    }

    info
}

// ---------------------------------------------------------------------------
// Text measurement
// ---------------------------------------------------------------------------

/// Measure the rendered width of `text` in logical pixels using the font
/// described by `font_info`.
///
/// Returns an approximation (`len * size * 0.6`) if the font cannot be loaded.
/// Uses the Unicode fallback chain for characters not found in Roboto.
pub fn measure_text(text: &str, font_info: &FontInfo) -> f64 {
    let family = fonts::resolve_family(&font_info.family);
    let font = get_cached_font_family(family, font_info.bold, font_info.italic);
    let font_ref = match to_font_ref(font) {
        Some(f) => f,
        None => return text.len() as f64 * font_info.size * 0.6,
    };

    let font_size = skrifa::instance::Size::new(font_info.size as f32);
    let var_loc = skrifa::instance::LocationRef::default();
    let primary_charmap = font_ref.charmap();
    let primary_glyph_metrics = font_ref.glyph_metrics(font_size, var_loc);

    let fallbacks = get_cached_fallback_fonts();

    let width: f32 = text
        .chars()
        .map(|ch| {
            let primary_gid = primary_charmap.map(ch).unwrap_or_default();
            if primary_gid != skrifa::GlyphId::new(0) {
                primary_glyph_metrics.advance_width(primary_gid).unwrap_or_default()
            } else {
                // Try fallback fonts
                for fb_font in fallbacks {
                    if let Some(fb_ref) = to_font_ref(fb_font) {
                        let fb_gid = fb_ref.charmap().map(ch).unwrap_or_default();
                        if fb_gid != skrifa::GlyphId::new(0) {
                            let fb_metrics = fb_ref.glyph_metrics(font_size, var_loc);
                            return fb_metrics.advance_width(fb_gid).unwrap_or_default();
                        }
                    }
                }
                // Character not found anywhere — use primary's GlyphId(0) advance (usually 0 or tofu)
                primary_glyph_metrics.advance_width(primary_gid).unwrap_or_default()
            }
        })
        .sum();

    width as f64
}

// ---------------------------------------------------------------------------
// Glyph layout
// ---------------------------------------------------------------------------

/// A single positioned glyph ready for submission to vello's `draw_glyphs`.
#[derive(Clone, Debug)]
pub struct GlyphLayout {
    /// Raw glyph ID (as returned by skrifa's charmap).
    pub glyph_id: u32,
    /// Horizontal advance position (left edge of this glyph, in font units at
    /// the given font size — i.e. logical pixels).
    pub x: f32,
    /// Vertical position; always `0.0` for horizontal text.
    pub y: f32,
    /// Index into the fallback font list.  `None` means the primary font.
    /// `Some(0)` = SymbolsNerdFontMono, `Some(1)` = NotoSansSymbols2,
    /// `Some(2)` = NotoColorEmoji, `Some(3)` = NotoEmoji.
    pub fallback_index: Option<usize>,
}

/// Lay out `text` into a sequence of [`GlyphLayout`] entries using the font
/// described by `font_info`, with Unicode fallback for unmapped characters.
///
/// Returns an empty `Vec` if the font cannot be loaded.
pub fn layout_glyphs(text: &str, font_info: &FontInfo) -> Vec<GlyphLayout> {
    let family = fonts::resolve_family(&font_info.family);
    let font = get_cached_font_family(family, font_info.bold, font_info.italic);
    let font_ref = match to_font_ref(font) {
        Some(f) => f,
        None => return Vec::new(),
    };

    let font_size = skrifa::instance::Size::new(font_info.size as f32);
    let var_loc = skrifa::instance::LocationRef::default();
    let primary_charmap = font_ref.charmap();
    let primary_glyph_metrics = font_ref.glyph_metrics(font_size, var_loc);

    let fallbacks = get_cached_fallback_fonts();

    let mut pen_x = 0.0f32;
    text.chars()
        .map(|ch| {
            let primary_gid = primary_charmap.map(ch).unwrap_or_default();
            if primary_gid != skrifa::GlyphId::new(0) {
                let advance = primary_glyph_metrics.advance_width(primary_gid).unwrap_or_default();
                let g = GlyphLayout {
                    glyph_id: primary_gid.to_u32(),
                    x: pen_x,
                    y: 0.0,
                    fallback_index: None,
                };
                pen_x += advance;
                g
            } else {
                // Try fallback fonts
                let mut result_gid = primary_gid;
                let mut result_advance = primary_glyph_metrics
                    .advance_width(primary_gid)
                    .unwrap_or_default();
                let mut result_fb_index = None;

                for (fb_idx, fb_font) in fallbacks.iter().enumerate() {
                    if let Some(fb_ref) = to_font_ref(fb_font) {
                        let fb_gid = fb_ref.charmap().map(ch).unwrap_or_default();
                        if fb_gid != skrifa::GlyphId::new(0) {
                            let fb_metrics = fb_ref.glyph_metrics(font_size, var_loc);
                            result_advance = fb_metrics.advance_width(fb_gid).unwrap_or_default();
                            result_gid = fb_gid;
                            result_fb_index = Some(fb_idx);
                            break;
                        }
                    }
                }

                let g = GlyphLayout {
                    glyph_id: result_gid.to_u32(),
                    x: pen_x,
                    y: 0.0,
                    fallback_index: result_fb_index,
                };
                pen_x += result_advance;
                g
            }
        })
        .collect()
}
