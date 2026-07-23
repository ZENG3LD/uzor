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
use crate::render::{GlyphMetric, WrappedLine};

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

// ── Per-(font, text, width) wrapped-line cache ───────────────────────────────

/// Own cache for [`measure_glyphs_wrapped`] — keyed `(font, text,
/// max_width.to_bits())`.
///
/// Deliberately **not** shared with [`shape_cache`]/[`outline_cache`]: those
/// are keyed `(font, text)` only (no width dimension) and are read by every
/// unwrapped `measure_glyphs`/`text_to_path` call site across all four
/// shaper-backed backends. Mixing a wrapped result into them would silently
/// return a stale/wrong-width result to an unwrapped caller.
fn wrapped_cache() -> &'static Mutex<HashMap<(String, String, u64), Vec<WrappedLine>>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String, u64), Vec<WrappedLine>>>> =
        OnceLock::new();
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

// ── Glyph-id shaping (backs URX's `DrawCommand::GlyphRun`) ────────────────────
//
// `measure_glyphs`/`text_to_path` above only ever expose CLUSTER-level
// metrics or flattened outline commands — neither carries the raw
// per-glyph `(font_id, glyph_id)` a glyph-ATLAS-based renderer needs.
// cosmic-text's own `LayoutGlyph` already carries both directly
// (`glyph_id: u16`, `font_id: fontdb::ID` — confirmed by direct read of
// `cosmic-text-0.12.1/src/layout.rs`); `text_to_path_uncached` below
// even threads the SAME `glyph_id` into `SwashCache::get_outline_commands`
// today (via `physical.cache_key.glyph_id`) to build its own outlines —
// this module exposes that already-shaped value directly instead of
// only its downstream outline-command byproduct.

/// Opaque per-process font identity for glyph-id-based rendering paths
/// (URX's `DrawCommand::GlyphRun`) — wraps cosmic-text's own `fontdb::ID`
/// so callers never need `cosmic-text`/`fontdb` as a direct dependency
/// (this crate is the only place that type is named). Stable for the
/// lifetime of the process — `font_system()` is a process-wide
/// `OnceLock` singleton, never rebuilt, so the same `ShaperFontId`
/// always resolves to the same font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaperFontId(cosmic_text::fontdb::ID);

/// One shaped glyph within a [`GlyphSegment`] — `(x, y)` is the
/// ABSOLUTE pen position in the text's own local coordinate space (pen
/// origin at `(0, 0)`, y already carries the run's baseline
/// `run.line_y` — same convention `text_to_path_uncached`'s
/// `pen_x`/`pen_y` already use), before the caller's own
/// `fill_text(text, x, y)` origin / text-align / text-baseline offset
/// is applied on top.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub x: f32,
    pub y: f32,
}

/// One contiguous same-font run of shaped glyphs. A font-FALLBACK
/// boundary (e.g. Latin text falling back to an emoji/CJK face
/// mid-string) always starts a NEW segment, never straddles one —
/// mirrors how every other glyph-id consumer in this workspace already
/// handles per-run font switches (`uzor-render-vello-cpu`'s own
/// `resolve_glyphs_with_fallback` + `run_font_index`-contiguous
/// grouping, `context.rs:897-909`).
#[derive(Debug, Clone)]
pub struct GlyphSegment {
    pub font: ShaperFontId,
    pub font_size: f32,
    pub glyphs: Vec<ShapedGlyph>,
    /// Reconstructed source substring for JUST this segment's glyphs
    /// (byte-range span across the segment, not the whole `fill_text`
    /// call's string) — the `DrawCommand::GlyphRun::text` companion
    /// field's own documented purpose: "backends that take `&str`" (a
    /// legacy, currently-unused consumer class per
    /// `uzor_urx_core::scene::DrawCommand::GlyphRun`'s own doc comment
    /// — populated anyway since it's cheap and keeps the IR
    /// round-trip-friendly, per that same doc comment's "producers
    /// SHOULD set it" guidance).
    pub text: String,
}

/// Shape `text` in `font`, returning glyph ids + positions grouped into
/// contiguous same-font [`GlyphSegment`]s. Unlike [`measure_glyphs`],
/// this does NOT merge ligature clusters (no cluster string is needed
/// for a pure rendering path — merging exists there only for cursor/
/// selection math) and is NOT cached (glyph-id lists are cheap to
/// regenerate; callers needing caching own their own policy, matching
/// `uzor_urx_glyph::draw_glyph_run`'s own "caller decides caching"
/// convention).
///
/// `font` is a CSS shorthand, e.g. `"bold 16px Inter"`. Empty text
/// returns an empty `Vec` immediately.
pub fn shape_glyph_runs(text: &str, font: &str) -> Vec<GlyphSegment> {
    if text.is_empty() {
        return Vec::new();
    }

    let info = parse_css_font(font);
    let font_size = info.size;

    let family_name: &str = match info.family {
        FontFamily::Roboto        => "Roboto",
        FontFamily::PtRootUi      => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    };

    let Ok(mut fs) = font_system().lock() else {
        return Vec::new();
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

    // Each in-progress segment: (font, font_size, byte-range span within
    // its OWN line's text, that line's text, glyphs-so-far). The byte
    // range grows in place as same-font glyphs are appended; the final
    // substring is sliced once, when the segment closes.
    struct InProgress {
        font: ShaperFontId,
        font_size: f32,
        range: std::ops::Range<usize>,
        line_text: String,
        glyphs: Vec<ShapedGlyph>,
    }
    let mut segments: Vec<InProgress> = Vec::new();

    for run in buf.layout_runs() {
        let line_text = run.text;
        let pen_y = run.line_y;

        for glyph in run.glyphs {
            let font_id = ShaperFontId(glyph.font_id);
            let shaped = ShapedGlyph { glyph_id: glyph.glyph_id as u32, x: glyph.x, y: pen_y };

            let mut extended_open_segment = false;
            if let Some(seg) = segments.last_mut() {
                if seg.font == font_id && seg.line_text == line_text {
                    seg.range.start = seg.range.start.min(glyph.start);
                    seg.range.end = seg.range.end.max(glyph.end);
                    seg.glyphs.push(shaped);
                    extended_open_segment = true;
                }
            }
            if !extended_open_segment {
                segments.push(InProgress {
                    font: font_id,
                    font_size: glyph.font_size,
                    range: glyph.start..glyph.end,
                    line_text: line_text.to_string(),
                    glyphs: vec![shaped],
                });
            }
        }
    }

    segments
        .into_iter()
        .map(|s| GlyphSegment {
            font: s.font,
            font_size: s.font_size,
            glyphs: s.glyphs,
            text: s.line_text.get(s.range).unwrap_or_default().to_string(),
        })
        .collect()
}

/// Raw font bytes for a `ShaperFontId` — `None` only if the process-wide
/// font system's database somehow no longer has the face (should not
/// happen for any id `shape_glyph_runs` itself returned in the SAME
/// process). Callers use this ONCE per distinct `ShaperFontId` to
/// register it with their own glyph-id rasteriser (e.g.
/// `uzor_urx_glyph::register_font`), caching the resulting handle
/// themselves — this function does no caching of its own beyond
/// `fontdb`'s own internal in-memory storage (the embedded font set is
/// loaded once via `load_font_data`, never memory-mapped from disk —
/// see `font_system()`'s own doc comment).
pub fn font_bytes_for(id: ShaperFontId) -> Option<Vec<u8>> {
    let fs = font_system().lock().ok()?;
    fs.db().with_face_data(id.0, |data, _face_index| data.to_vec())
}

/// First (English US, per `fontdb::FaceInfo::families`'s own doc
/// comment) family name for a `ShaperFontId` — lets a caller apply
/// family-specific policy (e.g. "this is the color-emoji face, which a
/// monochrome-coverage glyph rasteriser can't render — fall back to
/// vector-outline text for this segment instead").
pub fn font_family_for(id: ShaperFontId) -> Option<String> {
    let fs = font_system().lock().ok()?;
    fs.db().face(id.0).and_then(|face| face.families.first().map(|(name, _)| name.clone()))
}

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

/// Shape `text` in `font`, word-wrapped to `max_width`, and return one
/// [`WrappedLine`] per visual line.
///
/// Unlike [`measure_glyphs`] (infinite width, `Wrap::None`), this sets a real
/// width on the `cosmic-text` buffer and enables `Wrap::Word` — cosmic-text's
/// own line breaker does the wrapping; this function only reads back
/// `buf.layout_runs()` (one `LayoutRun` per visual line already).
///
/// `font` is a CSS shorthand, e.g. `"bold 16px Inter"`. `max_width` is in
/// logical pixels and is clamped to a 1px minimum internally (never handed
/// to cosmic-text as zero/negative). A `max_width` at or beyond the text's
/// natural width degenerates to exactly one line, glyph-for-glyph identical
/// to [`measure_glyphs`].
///
/// Results are cached per `(font, text, max_width.to_bits())` triple in a
/// cache **owned by this function** — see [`wrapped_cache`] for why this
/// must never be merged with [`shape_cache`]/[`outline_cache`].
///
/// The returned `Vec`'s index order **is** the visual line index; cosmic-
/// text's `LayoutRun::line_i` is the *source*-line index (increments only on
/// `\n`) and is never used as a substitute here — see [`WrappedLine`]'s docs.
pub fn measure_glyphs_wrapped(text: &str, font: &str, max_width: f64) -> Vec<WrappedLine> {
    if text.is_empty() {
        return Vec::new();
    }

    let cache_key = (font.to_string(), text.to_string(), max_width.to_bits());

    // Cache read
    if let Ok(cache) = wrapped_cache().lock() {
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let result = measure_glyphs_wrapped_uncached(text, font, max_width);

    // Cache write
    if let Ok(mut cache) = wrapped_cache().lock() {
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

// ── Internal wrapped shaper ────────────────────────────────────────────────────

fn measure_glyphs_wrapped_uncached(text: &str, font: &str, max_width: f64) -> Vec<WrappedLine> {
    let info = parse_css_font(font);
    let font_size = info.size;

    // Map FontFamily → cosmic-text Family name
    let family_name: &str = match info.family {
        FontFamily::Roboto      => "Roboto",
        FontFamily::PtRootUi    => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    };

    let Ok(mut fs) = font_system().lock() else {
        return fallback_per_char_wrapped(text, font, max_width);
    };

    let metrics = Metrics::new(font_size, font_size * 1.2);
    let mut buf = Buffer::new_empty(metrics);
    // Real width (never zero/negative — cosmic-text's wrap accumulator
    // expects a sane bound) + no height cap (None behaves as unlimited
    // scroll, same as the unwrapped path's `Some(f32::MAX)` — every visual
    // line is shaped and returned, none pruned).
    let width_f32 = max_width.max(1.0) as f32;
    buf.set_size(&mut fs, Some(width_f32), None);
    buf.set_wrap(&mut fs, Wrap::Word);

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

    let mut result: Vec<WrappedLine> = Vec::new();

    // Enumerate `buf.layout_runs()` in iteration order — this order IS the
    // visual line index. `run.line_i` is deliberately never read as an
    // index (see WrappedLine's docs: it's the source-line index).
    for run in buf.layout_runs() {
        let line_text = run.text;
        let mut glyphs: Vec<GlyphMetric> = Vec::new();

        // Same ligature-merge rule as `shape_uncached`, reset per line.
        let mut last_byte_range: Option<(usize, usize)> = None;

        for glyph in run.glyphs {
            let cluster_str = &line_text[glyph.start..glyph.end];

            let x_off = glyph.x as f64;
            let y_off = (glyph.y_offset * glyph.font_size) as f64;
            let width = glyph.w as f64;
            let advance = width;

            let same_cluster = last_byte_range == Some((glyph.start, glyph.end));
            if same_cluster {
                if let Some(last) = glyphs.last_mut() {
                    last.advance += advance;
                    last.width   += width;
                    continue;
                }
            }

            last_byte_range = Some((glyph.start, glyph.end));
            glyphs.push(GlyphMetric {
                cluster: cluster_str.to_string(),
                x_offset: x_off,
                y_offset: y_off,
                advance,
                width,
            });
        }

        result.push(WrappedLine {
            glyphs,
            line_top: run.line_top as f64,
            baseline_y: run.line_y as f64,
            width: run.line_w as f64,
        });
    }

    result
}

/// Fallback used only when the FontSystem mutex is poisoned (should never
/// happen in practice). Naive greedy word-wrap over the same per-`char`
/// approximation as [`fallback_per_char`].
fn fallback_per_char_wrapped(text: &str, font: &str, max_width: f64) -> Vec<WrappedLine> {
    let info = parse_css_font(font);
    let char_w = info.size as f64 * 0.6;
    let line_height = info.size as f64 * 1.2;
    let ascent = info.size as f64 * 0.9;

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0.0f64;

    for word in text.split_whitespace() {
        let word_w = word.chars().count() as f64 * char_w;

        if current.is_empty() {
            current.push_str(word);
            current_w = word_w;
            continue;
        }

        let candidate_w = current_w + char_w + word_w;
        if candidate_w > max_width {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
            current_w = word_w;
        } else {
            current.push(' ');
            current.push_str(word);
            current_w = candidate_w;
        }
    }
    lines.push(current);

    let mut line_top = 0.0f64;
    lines
        .into_iter()
        .map(|line_text| {
            let glyphs = fallback_per_char(&line_text, font);
            let width = line_text.chars().count() as f64 * char_w;
            let wrapped = WrappedLine {
                glyphs,
                line_top,
                baseline_y: line_top + ascent,
                width,
            };
            line_top += line_height;
            wrapped
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &str = "16px Roboto";
    const LONG_SENTENCE: &str = "The quick brown fox jumps over the lazy dog \
        and then keeps running further down the road without stopping for a \
        very long time indeed";

    /// Degenerate case: at `f64::MAX` width, `measure_glyphs_wrapped` must
    /// produce exactly one line, glyph-for-glyph identical to `measure_glyphs`
    /// — proves the new wrapped path doesn't perturb the unwrapped one.
    #[test]
    fn wrapped_at_huge_width_matches_unwrapped_glyph_for_glyph() {
        let wrapped = measure_glyphs_wrapped(LONG_SENTENCE, FONT, f64::MAX);
        assert_eq!(
            wrapped.len(),
            1,
            "expected exactly one line at f64::MAX width, got {}",
            wrapped.len()
        );

        let unwrapped = measure_glyphs(LONG_SENTENCE, FONT);
        let wrapped_glyphs = &wrapped[0].glyphs;
        assert_eq!(wrapped_glyphs.len(), unwrapped.len());

        for (w, u) in wrapped_glyphs.iter().zip(unwrapped.iter()) {
            assert_eq!(w.cluster, u.cluster);
            assert!((w.x_offset - u.x_offset).abs() < 0.01);
            assert!((w.y_offset - u.y_offset).abs() < 0.01);
            assert!((w.advance - u.advance).abs() < 0.01);
            assert!((w.width - u.width).abs() < 0.01);
        }
    }

    /// A width comfortably larger than the sentence's natural width also
    /// degenerates to one line (not just `f64::MAX`).
    #[test]
    fn wrapped_at_wide_enough_width_is_a_single_line() {
        let natural_width = measure_glyphs(LONG_SENTENCE, FONT)
            .iter()
            .map(|g| g.x_offset + g.advance)
            .fold(0.0f64, f64::max);

        let lines = measure_glyphs_wrapped(LONG_SENTENCE, FONT, natural_width + 100.0);
        assert_eq!(lines.len(), 1);
    }

    /// Narrow width forces wrapping. Every line must stay within `max_width`
    /// (this fixture's longest word is well short of `max_width`, so no
    /// unbreakable-overflow tolerance is needed), `line_top` must strictly
    /// increase one `line_height` at a time, and total height must equal
    /// `line_count * line_height`.
    #[test]
    fn narrow_width_wraps_into_multiple_lines_with_expected_geometry() {
        let max_width = 150.0;
        let line_height = 16.0 * 1.2; // Metrics::new(font_size, font_size * 1.2)

        let lines = measure_glyphs_wrapped(LONG_SENTENCE, FONT, max_width);
        assert!(
            lines.len() > 1,
            "expected wrap into multiple lines, got {}",
            lines.len()
        );

        for (i, line) in lines.iter().enumerate() {
            let expected_top = i as f64 * line_height;
            assert!(
                (line.line_top - expected_top).abs() < 0.5,
                "line {i} top {} != expected {expected_top}",
                line.line_top
            );
            assert!(
                line.width <= max_width + 1.0,
                "line {i} width {} exceeds max_width {max_width}",
                line.width
            );
            for glyph in &line.glyphs {
                let end = glyph.x_offset + glyph.advance;
                assert!(
                    end <= max_width + 1.0,
                    "glyph '{}' end {end} exceeds max_width {max_width} on line {i}",
                    glyph.cluster
                );
            }
        }

        let total_height = lines.len() as f64 * line_height;
        let last_top = lines.last().map(|l| l.line_top).unwrap_or(0.0);
        assert!((last_top + line_height - total_height).abs() < 0.5);
    }

    /// Empty text short-circuits to an empty `Vec`, same convention as
    /// `measure_glyphs`.
    #[test]
    fn wrapped_empty_text_is_empty() {
        assert!(measure_glyphs_wrapped("", FONT, 100.0).is_empty());
    }

    // ── shape_glyph_runs (URX GlyphRun bridging) ──────────────────────

    #[test]
    fn shape_glyph_runs_empty_text_is_empty() {
        assert!(shape_glyph_runs("", FONT).is_empty());
    }

    #[test]
    fn shape_glyph_runs_single_font_text_is_one_segment() {
        let segments = shape_glyph_runs("Hello", FONT);
        assert_eq!(segments.len(), 1, "plain ASCII text in one font must stay one segment");
        let seg = &segments[0];
        assert_eq!(seg.glyphs.len(), 5, "one glyph per character, no ligatures in \"Hello\"");
        assert_eq!(seg.text, "Hello");
        // glyph_id 0 is always .notdef -- a real font must never resolve
        // ASCII letters to it.
        assert!(seg.glyphs.iter().all(|g| g.glyph_id != 0), "no glyph should resolve to .notdef for plain ASCII text");
    }

    #[test]
    fn shape_glyph_runs_x_positions_are_monotonically_increasing_for_ltr_text() {
        let segments = shape_glyph_runs("Hello", FONT);
        let seg = &segments[0];
        for w in seg.glyphs.windows(2) {
            assert!(w[1].x >= w[0].x, "LTR glyph pen positions must not go backwards: {} then {}", w[0].x, w[1].x);
        }
    }

    #[test]
    fn shape_glyph_runs_font_bytes_for_returns_a_real_font_file() {
        let segments = shape_glyph_runs("Hello", FONT);
        let bytes = font_bytes_for(segments[0].font).expect("Roboto must resolve to real font bytes");
        // TrueType/OpenType files start with a 4-byte version tag —
        // 0x00010000 (TrueType) or "OTTO" (CFF-flavoured). Either way,
        // a real font file is comfortably larger than a few KB.
        assert!(bytes.len() > 1024, "a real font file must be more than 1KB, got {}", bytes.len());
    }

    #[test]
    fn shape_glyph_runs_font_family_for_reports_roboto() {
        let segments = shape_glyph_runs("Hello", FONT);
        let family = font_family_for(segments[0].font).expect("Roboto must resolve to a family name");
        assert_eq!(family, "Roboto");
    }

    #[test]
    fn shape_glyph_runs_is_deterministic_across_calls() {
        let a = shape_glyph_runs("Hello, world!", FONT);
        let b = shape_glyph_runs("Hello, world!", FONT);
        assert_eq!(a.len(), b.len());
        for (sa, sb) in a.iter().zip(b.iter()) {
            assert_eq!(sa.font, sb.font);
            assert_eq!(sa.text, sb.text);
            assert_eq!(sa.glyphs.len(), sb.glyphs.len());
            for (ga, gb) in sa.glyphs.iter().zip(sb.glyphs.iter()) {
                assert_eq!(ga.glyph_id, gb.glyph_id);
                assert_eq!(ga.x, gb.x);
                assert_eq!(ga.y, gb.y);
            }
        }
    }
}
