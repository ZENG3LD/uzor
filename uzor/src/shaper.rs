//! Per-cluster text shaper backed by [`cosmic_text`].
//!
//! Enabled only when the `shaper` feature is active. Backends that need
//! cluster-correct [`GlyphMetric`] results (tiny-skia, vello-gpu/cpu/hybrid)
//! enable this feature and call [`measure_glyphs`].
//!
//! # Font loading
//!
//! A process-wide [`cosmic_text::FontSystem`] is constructed once (via
//! [`OnceLock`]) from the embedded font bytes in [`uzor_fonts`]. **No system
//! fonts are loaded** — the font set is fully self-contained.
//!
//! # Caching
//!
//! Results are cached in a process-wide `Mutex<HashMap<(font_str, text), Vec<GlyphMetric>>>`.
//! The cache is unbounded for Phase 4 — callers (typically `emit_per_glyph_layers`)
//! run at scene-load time, not per-frame, so memory growth is bounded in practice.
//! A bounded LRU can replace this cache if memory pressure becomes a concern.

use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Wrap};

use crate::fonts::{parse_css_font, FontFamily};
use crate::render::GlyphMetric;

// ── Process-wide font system ─────────────────────────────────────────────────

/// Returns the process-wide [`FontSystem`] initialised with embedded fonts only
/// (no system font directories are scanned).
fn font_system() -> &'static Mutex<FontSystem> {
    static FS: OnceLock<Mutex<FontSystem>> = OnceLock::new();
    FS.get_or_init(|| {
        use cosmic_text::fontdb;
        use uzor_fonts as f;

        let mut db = fontdb::Database::new();

        // Load all embedded font bytes. fontdb identifies faces by their
        // PostScript name — we load every variant so cosmic-text can match
        // bold/italic Attrs correctly.
        for bytes in &[
            f::ROBOTO_REGULAR,
            f::ROBOTO_BOLD,
            f::ROBOTO_ITALIC,
            f::ROBOTO_BOLD_ITALIC,
            f::PT_ROOT_UI_VF,
            f::JETBRAINS_MONO_REGULAR,
            f::JETBRAINS_MONO_BOLD,
            f::SYMBOLS_NERD_FONT_MONO,
            f::NOTO_SANS_SYMBOLS2,
            f::NOTO_COLOR_EMOJI,
            f::NOTO_EMOJI,
            f::DEJAVU_SANS,
        ] {
            db.load_font_data(bytes.to_vec());
        }

        // Set generic family names so Attrs::new().family(Family::SansSerif)
        // resolves to Roboto and Family::Monospace resolves to JetBrains Mono.
        db.set_sans_serif_family("Roboto");
        db.set_monospace_family("JetBrains Mono");
        db.set_serif_family("Roboto");

        let fs = FontSystem::new_with_locale_and_db("en-US".to_string(), db);
        Mutex::new(fs)
    })
}

// ── Per-(font, text) shape cache ─────────────────────────────────────────────

fn shape_cache() -> &'static Mutex<HashMap<(String, String), Vec<GlyphMetric>>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), Vec<GlyphMetric>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Shape `text` in `font` and return per-cluster [`GlyphMetric`] values.
///
/// Returns cached results on repeated calls with the same `(text, font)` pair.
/// Empty text returns an empty `Vec` immediately.
///
/// `font` is a CSS shorthand, e.g. `"bold 16px Inter"`.
pub fn measure_glyphs(text: &str, font: &str) -> Vec<GlyphMetric> {
    if text.is_empty() {
        return Vec::new();
    }

    let cache_key = (font.to_string(), text.to_string());

    // Cache read
    if let Ok(cache) = shape_cache().lock() {
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let result = shape_uncached(text, font);

    // Cache write
    if let Ok(mut cache) = shape_cache().lock() {
        cache.insert(cache_key, result.clone());
    }

    result
}

// ── Internal shaper ───────────────────────────────────────────────────────────

fn shape_uncached(text: &str, font: &str) -> Vec<GlyphMetric> {
    let info = parse_css_font(font);
    let font_size = info.size;

    // Map FontFamily → cosmic-text Family name
    let family_name: &str = match info.family {
        FontFamily::Roboto      => "Roboto",
        FontFamily::PtRootUi    => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    };

    let Ok(mut fs) = font_system().lock() else {
        return fallback_per_char(text, font);
    };

    let metrics = Metrics::new(font_size, font_size * 1.2);
    let mut buf = Buffer::new_empty(metrics);
    buf.set_size(&mut fs, Some(f32::MAX), Some(f32::MAX));
    buf.set_wrap(&mut fs, Wrap::None);

    let attrs = Attrs::new()
        .family(Family::Name(family_name))
        .weight(if info.bold {
            cosmic_text::Weight::BOLD
        } else {
            cosmic_text::Weight::NORMAL
        })
        .style(if info.italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        });

    buf.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buf.shape_until_scroll(&mut fs, false);

    let mut result: Vec<GlyphMetric> = Vec::new();

    // Track the byte range of the last emitted cluster so that ligatures
    // (multiple LayoutGlyphs sharing identical start..end) are merged into
    // one GlyphMetric. Two separate identical characters (e.g. "ll") will
    // have DIFFERENT byte ranges and are therefore NOT merged.
    let mut last_byte_range: Option<(usize, usize)> = None;

    for run in buf.layout_runs() {
        let line_text = run.text;

        for glyph in run.glyphs {
            let cluster_str = &line_text[glyph.start..glyph.end];

            // x is the hitbox left edge in the line (pen position).
            let x_off = glyph.x as f64;
            let y_off = (glyph.y_offset * glyph.font_size) as f64;
            let width = glyph.w as f64;
            // advance: hitbox width == advance for horizontal LTR text.
            let advance = width;

            // Merge only when the byte range is IDENTICAL to the previous
            // glyph (ligature: multiple shaped glyphs for the same cluster).
            let same_cluster = last_byte_range == Some((glyph.start, glyph.end));
            if same_cluster {
                if let Some(last) = result.last_mut() {
                    last.advance += advance;
                    last.width   += width;
                    continue;
                }
            }

            last_byte_range = Some((glyph.start, glyph.end));
            result.push(GlyphMetric {
                cluster: cluster_str.to_string(),
                x_offset: x_off,
                y_offset: y_off,
                advance,
                width,
            });
        }
    }

    result
}

/// Fallback used only when the FontSystem mutex is poisoned (should never
/// happen in practice). Returns one entry per `char` using `text_bounds`-style
/// approximation.
fn fallback_per_char(text: &str, font: &str) -> Vec<GlyphMetric> {
    let info = parse_css_font(font);
    let char_w = info.size as f64 * 0.6;
    let mut x = 0.0f64;
    text.chars()
        .map(|c| {
            let cluster = c.to_string();
            let advance = char_w;
            let m = GlyphMetric {
                cluster,
                x_offset: x,
                y_offset: 0.0,
                advance,
                width: advance,
            };
            x += advance;
            m
        })
        .collect()
}
