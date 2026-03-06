//! Font loading, CSS font string parsing, and text metrics for vello backends.
//!
//! Embeds all four Roboto variants (regular, bold, italic, bold-italic) and
//! caches them as `peniko::FontData` objects so they are only heap-allocated
//! once per process.

use std::sync::{Arc, OnceLock};

use vello::peniko::{Blob, FontData};
use skrifa::{MetadataProvider, raw::{FileRef, FontRef}};

// ---------------------------------------------------------------------------
// Embedded font bytes
// ---------------------------------------------------------------------------

static ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

// ---------------------------------------------------------------------------
// Cached FontData (created once, reused forever)
// ---------------------------------------------------------------------------

static CACHED_FONT_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_ITALIC: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

/// Return a `&'static FontData` for the Roboto variant matching `(bold, italic)`.
///
/// The `FontData` is lazily initialised the first time it is needed and then
/// stored permanently via `OnceLock`, so it is safe to call from any thread.
pub fn get_cached_font(bold: bool, italic: bool) -> &'static FontData {
    match (bold, italic) {
        (true, true) => CACHED_FONT_BOLD_ITALIC.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(ROBOTO_BOLD_ITALIC.to_vec())), 0)
        }),
        (true, false) => CACHED_FONT_BOLD.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(ROBOTO_BOLD.to_vec())), 0)
        }),
        (false, true) => CACHED_FONT_ITALIC.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(ROBOTO_ITALIC.to_vec())), 0)
        }),
        (false, false) => CACHED_FONT_REGULAR.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(ROBOTO_REGULAR.to_vec())), 0)
        }),
    }
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
    info.bold = false;
    info.italic = false;
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
pub fn measure_text(text: &str, font_info: &FontInfo) -> f64 {
    let font = get_cached_font(font_info.bold, font_info.italic);
    let font_ref = match to_font_ref(font) {
        Some(f) => f,
        None => return text.len() as f64 * font_info.size * 0.6,
    };

    let font_size = skrifa::instance::Size::new(font_info.size as f32);
    let var_loc = skrifa::instance::LocationRef::default();
    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(font_size, var_loc);

    let width: f32 = text
        .chars()
        .map(|ch| {
            let gid = charmap.map(ch).unwrap_or_default();
            glyph_metrics.advance_width(gid).unwrap_or_default()
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
}

/// Lay out `text` into a sequence of [`GlyphLayout`] entries using the font
/// described by `font_info`.
///
/// Returns an empty `Vec` if the font cannot be loaded.
pub fn layout_glyphs(text: &str, font_info: &FontInfo) -> Vec<GlyphLayout> {
    let font = get_cached_font(font_info.bold, font_info.italic);
    let font_ref = match to_font_ref(font) {
        Some(f) => f,
        None => return Vec::new(),
    };

    let font_size = skrifa::instance::Size::new(font_info.size as f32);
    let var_loc = skrifa::instance::LocationRef::default();
    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(font_size, var_loc);

    let mut pen_x = 0.0f32;
    text.chars()
        .filter_map(|ch| {
            let gid = charmap.map(ch).unwrap_or_default();
            let advance = glyph_metrics.advance_width(gid).unwrap_or_default();
            let g = GlyphLayout {
                glyph_id: gid.to_u32(),
                x: pen_x,
                y: 0.0,
            };
            pen_x += advance;
            Some(g)
        })
        .collect()
}
