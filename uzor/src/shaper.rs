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

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap};

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

// ── Per-(font, text) outline cache ───────────────────────────────────────────

fn outline_cache() -> &'static Mutex<HashMap<(String, String), String>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Process-wide [`SwashCache`] for glyph outline scaling.
///
/// Wrapped in a `Mutex` because `SwashCache` is not `Sync` and must be
/// accessed exclusively.  The same `font_system()` mutex serialises
/// `FontSystem` access, so holding both locks in the same order (font_system
/// first, swash_cache second) avoids deadlock.
fn swash_cache() -> &'static Mutex<SwashCache> {
    static SC: OnceLock<Mutex<SwashCache>> = OnceLock::new();
    SC.get_or_init(|| Mutex::new(SwashCache::new()))
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

/// Rasterize glyph outlines for `text` in `font` as an SVG path `d` string.
///
/// Positions are relative to the text origin `(0, 0)` — the same anchor
/// you would pass to `fill_text(text, 0, 0)`.
///
/// Coordinate system: SVG y-down (canvas convention).  swash gives y-up
/// font coordinates; this function flips the sign on every y component so
/// the result composites correctly with `push_clip_svg_path`.
///
/// Coordinates are rounded to integer pixels (subpixel positioning
/// discarded — sufficient for clip masks).
///
/// Returns an empty string for empty input or if the font system is
/// unavailable.  Results are cached per `(font, text)` pair (unbounded).
pub fn text_to_path(text: &str, font: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let cache_key = (font.to_string(), text.to_string());

    // Cache read
    if let Ok(cache) = outline_cache().lock() {
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let result = text_to_path_uncached(text, font);

    // Cache write
    if let Ok(mut cache) = outline_cache().lock() {
        cache.insert(cache_key, result.clone());
    }

    result
}

// ── Internal outline builder ──────────────────────────────────────────────────

fn text_to_path_uncached(text: &str, font: &str) -> String {
    use cosmic_text::Command;

    let info = parse_css_font(font);
    let font_size = info.size;

    let family_name: &str = match info.family {
        FontFamily::Roboto      => "Roboto",
        FontFamily::PtRootUi    => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    };

    // Acquire font_system first, then swash_cache — consistent lock order.
    let Ok(mut fs) = font_system().lock() else {
        return String::new();
    };
    let Ok(mut sc) = swash_cache().lock() else {
        return String::new();
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

    let mut d = String::new();

    for run in buf.layout_runs() {
        for glyph in run.glyphs {
            // Pen position of this glyph in logical (non-subpixel) coordinates.
            // x is the hitbox left edge; line_y is the baseline y.
            let pen_x = glyph.x;
            let pen_y = run.line_y;

            let physical = glyph.physical((0.0, 0.0), 1.0);
            let Some(cmds) = sc.get_outline_commands(&mut fs, physical.cache_key) else {
                continue;
            };

            for cmd in cmds {
                match *cmd {
                    Command::MoveTo(p) => {
                        let x = (pen_x + p.x).round() as i32;
                        let y = (pen_y - p.y).round() as i32;
                        if !d.is_empty() { d.push(' '); }
                        d.push_str(&format!("M {x} {y}"));
                    }
                    Command::LineTo(p) => {
                        let x = (pen_x + p.x).round() as i32;
                        let y = (pen_y - p.y).round() as i32;
                        d.push_str(&format!(" L {x} {y}"));
                    }
                    Command::QuadTo(c, p) => {
                        let cx = (pen_x + c.x).round() as i32;
                        let cy = (pen_y - c.y).round() as i32;
                        let x  = (pen_x + p.x).round() as i32;
                        let y  = (pen_y - p.y).round() as i32;
                        d.push_str(&format!(" Q {cx} {cy} {x} {y}"));
                    }
                    Command::CurveTo(c1, c2, p) => {
                        let c1x = (pen_x + c1.x).round() as i32;
                        let c1y = (pen_y - c1.y).round() as i32;
                        let c2x = (pen_x + c2.x).round() as i32;
                        let c2y = (pen_y - c2.y).round() as i32;
                        let x   = (pen_x + p.x).round() as i32;
                        let y   = (pen_y - p.y).round() as i32;
                        d.push_str(&format!(" C {c1x} {c1y} {c2x} {c2y} {x} {y}"));
                    }
                    Command::Close => {
                        d.push_str(" Z");
                    }
                }
            }
        }
    }

    d
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
