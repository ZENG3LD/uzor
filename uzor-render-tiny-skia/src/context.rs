//! `TinySkiaCpuRenderContext` — CPU-only [`RenderContext`] backed by `tiny-skia` and `fontdue`.
//!
//! Rasterizes all drawing commands into an in-memory RGBA8 [`tiny_skia::Pixmap`].
//! Text is rendered via `fontdue` glyph bitmaps composited onto the pixmap.
//!
//! ## Frame lifecycle
//!
//! ```rust,ignore
//! let mut ctx = TinySkiaCpuRenderContext::new(800, 600, 1.0);
//! ctx.clear(tiny_skia::Color::BLACK);
//! ctx.set_fill_color("#1e1e2e");
//! ctx.fill_rect(0.0, 0.0, 800.0, 600.0);
//! let rgba = ctx.pixels(); // &[u8] RGBA8
//! ```

use std::f32::consts::PI;
use std::sync::OnceLock;

use tiny_skia::{
    BlendMode as TsBlendMode, Color, FillRule, GradientStop, LineCap, LinearGradient, LineJoin,
    Mask, Paint, Path, PathBuilder, Pixmap, Point, RadialGradient, Rect, Shader, SpreadMode,
    Stroke, StrokeDash, Transform,
};

use uzor::render::{
    BackdropBlur, BatchPainter, BlendMode as UzorBlendMode, CircleBatch, Effects,
    GradientPainter, ImagePainter, LineSegment, Masking, Painter,
    OffscreenTarget, OffscreenTargetDesc, OffscreenTargetId,
    RenderContext as UzorRenderContext, RenderContextExt, ShapeHelpers,
    TextAlign, TextBaseline, TextBounds, TextMetrics, TextRenderer, UiEffectHelpers,
};
use uzor::core::types::Rect as UzorRect;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Centralized font bytes (sourced from uzor::fonts)
// ---------------------------------------------------------------------------

use uzor::fonts;

// ---------------------------------------------------------------------------
// Cached fontdue fonts (one per process)
// ---------------------------------------------------------------------------

static FONT_REGULAR: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_BOLD: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_ITALIC: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_BOLD_ITALIC: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_PT_ROOT_UI: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_JB_MONO_REGULAR: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_JB_MONO_BOLD: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_NERD_FONT:    OnceLock<fontdue::Font> = OnceLock::new();
static FONT_SYMBOLS:      OnceLock<fontdue::Font> = OnceLock::new();
static FONT_COLOR_EMOJI:  OnceLock<fontdue::Font> = OnceLock::new();
static FONT_EMOJI:        OnceLock<fontdue::Font> = OnceLock::new();
static FONT_CJK_SC:       OnceLock<fontdue::Font> = OnceLock::new();
static FONT_ARABIC:       OnceLock<fontdue::Font> = OnceLock::new();
static FONT_DEVANAGARI:   OnceLock<fontdue::Font> = OnceLock::new();

/// Re-export of the backend-agnostic family enum from core uzor. All family
/// detection lives in `uzor::fonts` — this backend only caches the loaded
/// `fontdue::Font` per (family, style) slot.
use uzor::fonts::FontFamily;

fn make_font(bytes: &[u8]) -> fontdue::Font {
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
        .expect("embedded font bytes are valid")
}

/// Return the cached fontdue font for the requested family / style combination.
///
/// Delegates family → bytes resolution to `uzor::fonts::font_bytes` and caches
/// the decoded `fontdue::Font` locally so each slot is constructed at most
/// once per process.
fn get_font(family: FontFamily, bold: bool, italic: bool) -> &'static fontdue::Font {
    match family {
        FontFamily::PtRootUi => FONT_PT_ROOT_UI
            .get_or_init(|| make_font(fonts::font_bytes(family, bold, italic))),
        FontFamily::JetBrainsMono => {
            let _ = italic; // no italic variant bundled
            if bold {
                FONT_JB_MONO_BOLD
                    .get_or_init(|| make_font(fonts::font_bytes(family, true, false)))
            } else {
                FONT_JB_MONO_REGULAR
                    .get_or_init(|| make_font(fonts::font_bytes(family, false, false)))
            }
        }
        FontFamily::Roboto => match (bold, italic) {
            (true,  true ) => FONT_BOLD_ITALIC
                .get_or_init(|| make_font(fonts::font_bytes(family, true, true))),
            (true,  false) => FONT_BOLD
                .get_or_init(|| make_font(fonts::font_bytes(family, true, false))),
            (false, true ) => FONT_ITALIC
                .get_or_init(|| make_font(fonts::font_bytes(family, false, true))),
            (false, false) => FONT_REGULAR
                .get_or_init(|| make_font(fonts::font_bytes(family, false, false))),
        },
    }
}

fn get_nerd_font() -> &'static fontdue::Font {
    FONT_NERD_FONT.get_or_init(|| make_font(fonts::SYMBOLS_NERD_FONT_MONO))
}

fn get_symbols_font() -> &'static fontdue::Font {
    FONT_SYMBOLS.get_or_init(|| make_font(fonts::NOTO_SANS_SYMBOLS2))
}

fn get_color_emoji_font() -> &'static fontdue::Font {
    // fontdue renders the monochrome outline from COLRv1; color layers are
    // silently ignored, but the glyph shape is still useful as a last resort.
    FONT_COLOR_EMOJI.get_or_init(|| make_font(fonts::NOTO_COLOR_EMOJI))
}

fn get_emoji_font() -> &'static fontdue::Font {
    FONT_EMOJI.get_or_init(|| make_font(fonts::NOTO_EMOJI))
}

fn get_cjk_sc_font() -> &'static fontdue::Font {
    FONT_CJK_SC.get_or_init(|| make_font(fonts::NOTO_SANS_CJK_SC))
}

fn get_arabic_font() -> &'static fontdue::Font {
    FONT_ARABIC.get_or_init(|| make_font(fonts::NOTO_SANS_ARABIC))
}

fn get_devanagari_font() -> &'static fontdue::Font {
    FONT_DEVANAGARI.get_or_init(|| make_font(fonts::NOTO_SANS_DEVANAGARI))
}

// ---------------------------------------------------------------------------
// CSS color parsing
// ---------------------------------------------------------------------------

fn parse_css_color(s: &str) -> Color {
    let (r, g, b, a) = uzor::render::parse_color(s);
    Color::from_rgba8(r, g, b, a)
}

/// Apply a global alpha multiplier to a `tiny_skia::Color`.
fn with_alpha(color: Color, alpha: f32) -> Color {
    let a = (color.alpha() * alpha).clamp(0.0, 1.0);
    Color::from_rgba(color.red(), color.green(), color.blue(), a)
        .unwrap_or(color)
}

// ---------------------------------------------------------------------------
// CSS font parsing
// ---------------------------------------------------------------------------

/// Backend-local alias for the parsed CSS font info.
///
/// Delegates entirely to `uzor::fonts::FontInfo`; kept as a type alias so
/// existing call sites stay unchanged.
type FontInfo = uzor::fonts::FontInfo;

fn parse_css_font(font_str: &str) -> FontInfo {
    fonts::parse_css_font(font_str)
}

// ---------------------------------------------------------------------------
// Rotated/sheared text fallback
// ---------------------------------------------------------------------------
//
// `fill_text`'s own fast path (below) rasterizes each glyph as an
// axis-aligned bitmap blit and positions it by reading ONLY `tx`/`ty`/`sx`/
// `sy` off the active `Transform` — it never reads `kx`/`ky` (the terms
// that encode rotation/shear), so a `ctx.rotate(...)` call has ZERO
// effect on `fill_text`'s own output (found 2026-07-25, live-diagnosed via
// a `uzor-figures` `LabelOverflow::Rotate` proof PNG rendering completely
// unrotated, overlapping text). This is NOT a caller-side geometry bug —
// `uzor-render-urx`'s own `fill_text` already hit and fixed the identical
// defect for itself (see that crate's `is_translation_only` fallback):
// when the active transform carries rotation/shear, render the glyph
// outlines as a real vector PATH instead of a raster blit — `Pixmap::
// fill_path` (used by every other shape in this backend, e.g. `fill()`)
// DOES apply the full affine transform, since path-fill is this crate's
// own fundamental, unconditional vector-drawing primitive; only the
// specialized bitmap-blit glyph renderer took the rotation-blind
// shortcut.

/// `true` when `t` carries any rotation/shear component (`kx`/`ky`
/// non-zero) — the exact condition under which `fill_text`'s fast raster
/// path silently drops the rotation, so it must fall back to
/// [`fill_text_as_path_impl`] instead.
fn has_rotation_or_shear(t: Transform) -> bool {
    const EPS: f32 = 1e-6;
    t.kx.abs() > EPS || t.ky.abs() > EPS
}

/// Re-compose a CSS font shorthand string from an already-parsed
/// [`FontInfo`] — the inverse of [`parse_css_font`], needed because
/// `uzor::shaper::text_to_path` takes a CSS string, not a `FontInfo`.
/// Mirrors `uzor-render-urx`'s own identically-shaped `font_string`
/// helper (same 3 bundled families, same shorthand grammar).
fn font_css_string(info: &FontInfo) -> String {
    let family = match info.family {
        FontFamily::Roboto => "Roboto",
        FontFamily::PtRootUi => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    };
    let mut parts: Vec<String> = Vec::with_capacity(4);
    if info.italic {
        parts.push("italic".to_owned());
    }
    if info.bold {
        parts.push("bold".to_owned());
    }
    parts.push(format!("{}px", info.size));
    parts.push(family.to_owned());
    parts.join(" ")
}

/// Parse the RESTRICTED SVG path grammar `uzor::shaper::text_to_path`
/// emits (`M`/`L`/`Q`/`C`/`Z` only, always absolute, always
/// whitespace-separated integer coordinates — see that function's own
/// doc comment/implementation) into a `tiny_skia::Path`. Deliberately NOT
/// a general SVG path parser (no relative commands, no arcs — a font
/// glyph outline never needs either) — used only by
/// [`fill_text_as_path_impl`]. Returns `None` for empty input, an
/// unrecognized command, or a malformed coordinate (bailing rather than
/// mis-rendering).
fn parse_glyph_outline_path(svg: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let mut has_point = false;
    let mut tokens = svg.split_whitespace();

    fn next_f32<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Option<f32> {
        tokens.next()?.parse().ok()
    }

    while let Some(tok) = tokens.next() {
        match tok {
            "M" => {
                let x = next_f32(&mut tokens)?;
                let y = next_f32(&mut tokens)?;
                pb.move_to(x, y);
                has_point = true;
            }
            "L" => {
                let x = next_f32(&mut tokens)?;
                let y = next_f32(&mut tokens)?;
                pb.line_to(x, y);
            }
            "Q" => {
                let cx = next_f32(&mut tokens)?;
                let cy = next_f32(&mut tokens)?;
                let x = next_f32(&mut tokens)?;
                let y = next_f32(&mut tokens)?;
                pb.quad_to(cx, cy, x, y);
            }
            "C" => {
                let c1x = next_f32(&mut tokens)?;
                let c1y = next_f32(&mut tokens)?;
                let c2x = next_f32(&mut tokens)?;
                let c2y = next_f32(&mut tokens)?;
                let x = next_f32(&mut tokens)?;
                let y = next_f32(&mut tokens)?;
                pb.cubic_to(c1x, c1y, c2x, c2y, x, y);
            }
            "Z" => pb.close(),
            _ => return None, // unrecognized command — bail, never mis-render
        }
    }

    if !has_point {
        return None;
    }
    pb.finish()
}

// ---------------------------------------------------------------------------
// Process-wide glyph raster cache
// ---------------------------------------------------------------------------
//
// fontdue re-rasterizes a glyph's bitmap from its outline on EVERY
// `Font::rasterize` call — it keeps no cache of its own. Before this
// cache, `fill_text` paid that cost twice per glyph per call (once in
// `measure_text_width` for the align offset, once in the draw loop), and
// text-heavy per-frame consumers turned it into the dominant frame cost:
// the graph demo's 3D overlay (up to ~126 node labels × 8-direction halo
// = 9 `fill_text` per label, repainted every frame through this backend
// via `uzor-render-hub`'s compose overlay path) measured at ~11k outline
// rasterizations PER FRAME — tens of milliseconds of single-threaded CPU
// (live-diagnosed 2026-07-24, «параша с оптимизацией»). A cached glyph
// is a plain HashMap hit + `Arc` clone instead.
//
// Key includes the FULL fallback identity (family/bold/italic + exact
// size bits) and the cached value is the POST-fallback-chain result, so
// the (deep, 8-font) fallback probing is paid once per distinct glyph,
// not once per draw. Bounded defensively: a pathological consumer
// cycling thousands of distinct sizes clears the map rather than growing
// it without limit (a full re-warm is one frame of the old cost).

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    family: FontFamily,
    bold: bool,
    italic: bool,
    ch: char,
    px_bits: u32,
}

struct CachedGlyph {
    metrics: fontdue::Metrics,
    bitmap: Vec<u8>,
}

static GLYPH_CACHE: OnceLock<std::sync::Mutex<HashMap<GlyphKey, std::sync::Arc<CachedGlyph>>>> = OnceLock::new();

/// Bound on distinct (font, glyph, size) entries — generous for real UI
/// use (full ASCII × dozens of sizes × every family fits with room to
/// spare) while capping worst-case memory.
const GLYPH_CACHE_MAX_ENTRIES: usize = 32_768;

/// Rasterize `ch` at `px` through the SAME fallback chain `fill_text`
/// has always used (primary → NerdFont → Symbols2 → CJK SC → Arabic →
/// Devanagari → NotoColorEmoji → NotoEmoji → primary tofu; whitespace
/// never falls back), served from [`GLYPH_CACHE`] after the first call.
fn rasterize_cached(family: FontFamily, bold: bool, italic: bool, ch: char, px: f32) -> std::sync::Arc<CachedGlyph> {
    let key = GlyphKey { family, bold, italic, ch, px_bits: px.to_bits() };
    let cache = GLYPH_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Ok(map) = cache.lock() {
        if let Some(hit) = map.get(&key) {
            return std::sync::Arc::clone(hit);
        }
    }

    let font = get_font(family, bold, italic);
    let (mut metrics, mut bitmap) = font.rasterize(ch, px);
    if metrics.width == 0 && !ch.is_whitespace() {
        let fallbacks: [&fontdue::Font; 7] = [
            get_nerd_font(),
            get_symbols_font(),
            get_cjk_sc_font(),
            get_arabic_font(),
            get_devanagari_font(),
            get_color_emoji_font(),
            get_emoji_font(),
        ];
        for fb in fallbacks {
            let (m, b) = fb.rasterize(ch, px);
            if m.width > 0 {
                metrics = m;
                bitmap = b;
                break;
            }
        }
    }

    let entry = std::sync::Arc::new(CachedGlyph { metrics, bitmap });
    if let Ok(mut map) = cache.lock() {
        if map.len() >= GLYPH_CACHE_MAX_ENTRIES {
            map.clear();
        }
        map.insert(key, std::sync::Arc::clone(&entry));
    }
    entry
}

// ---------------------------------------------------------------------------
// Text width measurement via fontdue — rasterization-only escape hatch
// ---------------------------------------------------------------------------

/// Sum of fontdue's own per-glyph `advance_width` — the RAW, un-kerned
/// advance this backend's own [`fill_text`](TextRenderer::fill_text) pen
/// walk still uses to place each rasterized glyph bitmap (fontdue has no
/// GPOS kerning support at all — it never applies it during rasterization
/// either, so this is genuinely raster-accurate for THIS backend's own
/// paint pipeline).
///
/// **No longer** the width [`TextMetrics::measure_text`]/[`TextMetrics::
/// text_bounds`] report — both now delegate to [`uzor::shaper`] (real
/// GPOS kerning via cosmic-text), the SAME canonical source every other
/// shaper-backed backend in this workspace measures through, so a
/// layout decision (margin sizing, label-collision placement) never
/// disagrees by backend. This function stays as the internal
/// rasterization-pen-advance implementation detail only — call it
/// directly (not through the trait) if a caller genuinely needs THIS
/// backend's own raster-accurate advance instead of the cross-backend
/// canonical one.
fn measure_text_width(text: &str, font_info: &FontInfo) -> f64 {
    let mut width = 0.0f32;
    for ch in text.chars() {
        width += rasterize_cached(font_info.family, font_info.bold, font_info.italic, ch, font_info.size)
            .metrics
            .advance_width;
    }
    width as f64
}

// ---------------------------------------------------------------------------
// Arc approximation with cubic beziers
// ---------------------------------------------------------------------------

/// Approximate an arc segment with cubic bezier curves appended to `pb`.
///
/// Uses the standard 4-control-point approximation for each ≤90° segment.
fn arc_to_cubics(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32, start: f32, end: f32, has_current_point: bool) {
    let mut sweep = end - start;
    // Clamp sweep to avoid infinite loops
    if sweep == 0.0 { return; }
    if sweep > 2.0 * PI  { sweep = 2.0 * PI;  }
    if sweep < -2.0 * PI { sweep = -2.0 * PI; }

    let n_segs = ((sweep.abs() / (PI / 2.0)).ceil() as u32).max(1);
    let seg_angle = sweep / n_segs as f32;
    // Cubic bezier magic constant for arc approximation: k = 4/3 * tan(θ/4)
    // (θ/2 here was a real bug — wide sweeps rendered visibly faceted;
    // for θ=90° correct k ≈ 0.5523, the θ/2 form gave 1.333).
    let k = (4.0 / 3.0) * ((seg_angle / 4.0).abs().tan());

    let mut a = start;
    let start_x = cx + r * a.cos();
    let start_y = cy + r * a.sin();
    // Canvas2D spec: line_to from current point to arc start, but if there
    // is no current point (fresh path after begin_path), use move_to instead.
    if has_current_point {
        pb.line_to(start_x, start_y);
    } else {
        pb.move_to(start_x, start_y);
    }

    for _ in 0..n_segs {
        let a1 = a + seg_angle;
        let cos_a  = a.cos();
        let sin_a  = a.sin();
        let cos_a1 = a1.cos();
        let sin_a1 = a1.sin();

        let p0x = cx + r * cos_a;
        let p0y = cy + r * sin_a;
        let p3x = cx + r * cos_a1;
        let p3y = cy + r * sin_a1;

        let cp1x = p0x - k * r * sin_a;
        let cp1y = p0y + k * r * cos_a;
        let cp2x = p3x + k * r * sin_a1;
        let cp2y = p3y - k * r * cos_a1;

        pb.cubic_to(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
        a = a1;
    }
}

// ---------------------------------------------------------------------------
// Ellipse approximation (scaled arc)
// ---------------------------------------------------------------------------

fn ellipse_to_cubics(
    pb: &mut PathBuilder,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    start: f32,
    end: f32,
    has_current_point: bool,
) {
    let mut sweep = end - start;
    if sweep == 0.0 { return; }
    if sweep > 2.0 * PI  { sweep = 2.0 * PI;  }
    if sweep < -2.0 * PI { sweep = -2.0 * PI; }

    let n_segs = ((sweep.abs() / (PI / 2.0)).ceil() as u32).max(1);
    let seg_angle = sweep / n_segs as f32;
    // Same k = 4/3 * tan(θ/4) fix as arc_to_cubics above.
    let k = (4.0 / 3.0) * ((seg_angle / 4.0).abs().tan());

    let mut a = start;
    let start_x = cx + rx * a.cos();
    let start_y = cy + ry * a.sin();
    if has_current_point {
        pb.line_to(start_x, start_y);
    } else {
        pb.move_to(start_x, start_y);
    }

    for _ in 0..n_segs {
        let a1 = a + seg_angle;
        let cos_a  = a.cos();
        let sin_a  = a.sin();
        let cos_a1 = a1.cos();
        let sin_a1 = a1.sin();

        let p0x = cx + rx * cos_a;
        let p0y = cy + ry * sin_a;
        let p3x = cx + rx * cos_a1;
        let p3y = cy + ry * sin_a1;

        let cp1x = p0x - k * rx * sin_a;
        let cp1y = p0y + k * ry * cos_a;
        let cp2x = p3x + k * rx * sin_a1;
        let cp2y = p3y - k * ry * cos_a1;

        pb.cubic_to(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
        a = a1;
    }
}

// ---------------------------------------------------------------------------
// Shadow helpers (M6-P1)
// ---------------------------------------------------------------------------

/// Separable box-blur on an RGBA8 pixmap.
///
/// Applies a horizontal pass then a vertical pass, each with a kernel of
/// width `2*radius+1`.  This approximates a Gaussian blur well enough for
/// drop-shadow purposes at small radii.
fn box_blur_pixmap(pm: &mut Pixmap, radius: u32) {
    if radius == 0 {
        return;
    }
    let w = pm.width() as usize;
    let h = pm.height() as usize;
    let r = radius as usize;
    let klen = 2 * r + 1;

    let data = pm.data_mut();

    // Horizontal pass — operate channel-by-channel (RGBA = 4 channels)
    for row in 0..h {
        for ch in 0..4usize {
            let base = row * w;
            // Sliding window sum
            let mut sum = 0u32;
            // Prime the window for first pixel
            for k in 0..klen {
                let col = k.saturating_sub(r).min(w.saturating_sub(1));
                sum += data[(base + col) * 4 + ch] as u32;
            }
            // Left side: window not yet full on the left
            for col in 0..w {
                let add_col = (col + r).min(w.saturating_sub(1));
                let sub_col = col.saturating_sub(r + 1).min(w.saturating_sub(1));
                if col > 0 {
                    sum = sum + data[(base + add_col) * 4 + ch] as u32
                        - data[(base + sub_col) * 4 + ch] as u32;
                }
                data[(base + col) * 4 + ch] = (sum / klen as u32) as u8;
            }
        }
    }

    // Vertical pass
    for col in 0..w {
        for ch in 0..4usize {
            let mut sum = 0u32;
            for k in 0..klen {
                let row = k.saturating_sub(r).min(h.saturating_sub(1));
                sum += data[(row * w + col) * 4 + ch] as u32;
            }
            for row in 0..h {
                let add_row = (row + r).min(h.saturating_sub(1));
                let sub_row = row.saturating_sub(r + 1).min(h.saturating_sub(1));
                if row > 0 {
                    sum = sum + data[(add_row * w + col) * 4 + ch] as u32
                        - data[(sub_row * w + col) * 4 + ch] as u32;
                }
                data[(row * w + col) * 4 + ch] = (sum / klen as u32) as u8;
            }
        }
    }
}

/// Composite `src` over `dst` using pre-multiplied alpha blending.
///
/// This is a simple software SourceOver: for each pixel,
/// `out = src_alpha * src + (1 - src_alpha) * dst`.
fn draw_pixmap_over(dst: &mut Pixmap, src: &Pixmap) {
    let len = (dst.width() * dst.height()) as usize;
    let dst_data = dst.data_mut();
    let src_data = src.data();
    for i in 0..len {
        let base = i * 4;
        let sa = src_data[base + 3] as u32;
        if sa == 0 {
            continue;
        }
        if sa == 255 {
            dst_data[base]     = src_data[base];
            dst_data[base + 1] = src_data[base + 1];
            dst_data[base + 2] = src_data[base + 2];
            dst_data[base + 3] = src_data[base + 3];
        } else {
            let inv_sa = 255 - sa;
            dst_data[base]     = ((src_data[base]     as u32 * sa + dst_data[base]     as u32 * inv_sa) / 255) as u8;
            dst_data[base + 1] = ((src_data[base + 1] as u32 * sa + dst_data[base + 1] as u32 * inv_sa) / 255) as u8;
            dst_data[base + 2] = ((src_data[base + 2] as u32 * sa + dst_data[base + 2] as u32 * inv_sa) / 255) as u8;
            dst_data[base + 3] = (sa + dst_data[base + 3] as u32 * inv_sa / 255) as u8;
        }
    }
}

// ---------------------------------------------------------------------------
// Backdrop blur helper (draw_blur_background)
// ---------------------------------------------------------------------------

/// Copy a rectangular region `(src_x, src_y, w, h)` from `src` into a new
/// `Pixmap` of size `(w, h)`.  Coordinates are clamped to pixmap bounds;
/// out-of-bounds pixels are filled with transparent black.
///
/// Returns `None` only if `Pixmap::new` fails (zero-sized rect).
fn copy_pixmap_region(src: &Pixmap, src_x: i32, src_y: i32, w: u32, h: u32) -> Option<Pixmap> {
    let mut dst = Pixmap::new(w, h)?;
    let sw = src.width()  as i32;
    let sh = src.height() as i32;
    let src_data = src.data();
    let dst_data = dst.data_mut();
    for dy in 0..h as i32 {
        for dx in 0..w as i32 {
            let sx = src_x + dx;
            let sy = src_y + dy;
            let d_base = (dy as usize * w as usize + dx as usize) * 4;
            if sx >= 0 && sy >= 0 && sx < sw && sy < sh {
                let s_base = (sy as usize * sw as usize + sx as usize) * 4;
                dst_data[d_base]     = src_data[s_base];
                dst_data[d_base + 1] = src_data[s_base + 1];
                dst_data[d_base + 2] = src_data[s_base + 2];
                dst_data[d_base + 3] = src_data[s_base + 3];
            }
            // else: pixel stays zero (transparent black — only at frame edges)
        }
    }
    Some(dst)
}

/// Copy blurred pixels from `scratch` (with `margin` leading border on each
/// side) back into `dst` pixmap at `(dst_x, dst_y)` covering `(w × h)` pixels.
///
/// Pixels outside `dst` bounds are silently skipped.
fn blit_region(dst: &mut Pixmap, scratch: &Pixmap, dst_x: i32, dst_y: i32, w: u32, h: u32, margin: i32) {
    let dw = dst.width()  as i32;
    let dh = dst.height() as i32;
    let sw = scratch.width() as i32;
    let src_data = scratch.data();
    let dst_data = dst.data_mut();
    for row in 0..h as i32 {
        for col in 0..w as i32 {
            let dx = dst_x + col;
            let dy = dst_y + row;
            if dx < 0 || dy < 0 || dx >= dw || dy >= dh { continue; }
            let sx = margin + col;
            let sy = margin + row;
            if sx < 0 || sy < 0 || sx >= sw || sy >= scratch.height() as i32 { continue; }
            let d_base = (dy as usize * dw as usize + dx as usize) * 4;
            let s_base = (sy as usize * sw as usize + sx as usize) * 4;
            dst_data[d_base]     = src_data[s_base];
            dst_data[d_base + 1] = src_data[s_base + 1];
            dst_data[d_base + 2] = src_data[s_base + 2];
            dst_data[d_base + 3] = src_data[s_base + 3];
        }
    }
}

// ---------------------------------------------------------------------------
// Shadow state (M6-P1)
// ---------------------------------------------------------------------------

/// Active drop shadow parameters.  `None` means no shadow.
#[derive(Clone)]
struct ShadowState {
    dx:    f32,
    dy:    f32,
    blur:  f32,
    color: Color,
}

// ---------------------------------------------------------------------------
// Save/restore state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SavedState {
    fill_color:    Color,
    stroke_color:  Color,
    stroke_width:  f32,
    line_cap:      LineCap,
    line_join:     LineJoin,
    global_alpha:  f32,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    transform:     Transform,
    clip:          Option<Mask>,
}

// ---------------------------------------------------------------------------
// TinySkiaCpuRenderContext
// ---------------------------------------------------------------------------

/// CPU-only rendering context backed by [`tiny_skia::Pixmap`] and `fontdue`.
///
/// All drawing happens in software; no GPU or windowing system required.
pub struct TinySkiaCpuRenderContext {
    pixmap:        Pixmap,
    // Drawing state
    fill_color:    Color,
    stroke_color:  Color,
    stroke_width:  f32,
    line_cap:      LineCap,
    line_join:     LineJoin,
    line_dash:     Option<StrokeDash>,
    global_alpha:  f32,
    // Text
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    // Transform
    transform:     Transform,
    // Current path
    path_builder:  Option<PathBuilder>,
    // Whether the current path has a subpath (move_to/line_to called since begin_path)
    path_has_point: bool,
    // Save/restore
    state_stack:   Vec<SavedState>,
    current_clip:  Option<Mask>,
    // M6-P1: Drop shadow
    shadow:        Option<ShadowState>,
    // M6-P3: Blend mode
    blend_mode:    TsBlendMode,
    // Device pixel ratio
    dpr:           f64,
    // Backdrop blur radius (px) used by draw_blur_background.
    // Default 12.  Set via set_backdrop_blur_radius().
    blur_radius:   f32,
    // Offscreen render-target cache — populated targets, keyed by handle.
    // See `uzor::render::offscreen` for the trait contract.
    offscreen_targets: HashMap<OffscreenTargetId, Pixmap>,
    // Monotonic counter for allocating new `OffscreenTargetId`s.
    next_offscreen_id: u64,
    // Stack of entries swapped out by `push_offscreen_target`;
    // `pop_offscreen_target` swaps the top entry back in and stores the
    // painted pixmap under its id. Carries the outer per-frame drawing
    // state (transform / clip / save stack / path) — the recording
    // paints at its own local origin with fresh state, and the outer
    // walk continues afterwards with ITS state intact.
    offscreen_stack: Vec<OffscreenRecording>,
}

/// One level of the offscreen-recording stack for
/// [`TinySkiaCpuRenderContext`].
struct OffscreenRecording {
    id:             OffscreenTargetId,
    pixmap:         Pixmap,
    transform:      Transform,
    current_clip:   Option<Mask>,
    state_stack:    Vec<SavedState>,
    path_builder:   Option<PathBuilder>,
    path_has_point: bool,
}

impl TinySkiaCpuRenderContext {
    /// Create a new context with a pixmap of `width × height` pixels.
    pub fn new(width: u32, height: u32, dpr: f64) -> Self {
        let pixmap = Pixmap::new(width.max(1), height.max(1))
            .unwrap_or_else(|| Pixmap::new(1, 1).expect("1×1 pixmap always succeeds"));
        Self {
            pixmap,
            fill_color:    Color::BLACK,
            stroke_color:  Color::WHITE,
            stroke_width:  1.0,
            line_cap:      LineCap::Butt,
            line_join:     LineJoin::Miter,
            line_dash:     None,
            global_alpha:  1.0,
            font_info:     FontInfo::default(),
            text_align:    TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            transform:     Transform::identity(),
            path_builder:  None,
            path_has_point: false,
            state_stack:   Vec::new(),
            current_clip:  None,
            shadow:        None,
            blend_mode:    TsBlendMode::SourceOver,
            dpr,
            blur_radius:   12.0,
            offscreen_targets: HashMap::new(),
            next_offscreen_id: 0,
            offscreen_stack:   Vec::new(),
        }
    }

    /// Resize the pixmap.  Content is cleared.
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(pm) = Pixmap::new(width.max(1), height.max(1)) {
            self.pixmap      = pm;
            self.current_clip = None;
        }
    }

    /// Set the backdrop blur radius (pixels) used by [`draw_blur_background`].
    ///
    /// Typical values: 8–20 px.  Values below 1 are clamped to 1.
    /// Default is 12 px.
    pub fn set_backdrop_blur_radius(&mut self, radius: f32) {
        self.blur_radius = radius.max(1.0);
    }

    /// Clear the entire pixmap with a solid color.
    pub fn clear(&mut self, color: Color) {
        self.pixmap.fill(color);
    }

    /// Raw RGBA8 pixel data (width × height × 4 bytes, row-major).
    pub fn pixels(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Borrow the underlying `Pixmap`.
    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }

    /// Pixmap width in pixels.
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Pixmap height in pixels.
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn effective_fill_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            with_alpha(self.fill_color, self.global_alpha)
        } else {
            self.fill_color
        }
    }

    fn effective_stroke_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            with_alpha(self.stroke_color, self.global_alpha)
        } else {
            self.stroke_color
        }
    }

    fn fill_paint(&self) -> Paint<'static> {
        Paint {
            shader: Shader::SolidColor(self.effective_fill_color()),
            blend_mode: self.blend_mode,
            anti_alias: true,
            ..Paint::default()
        }
    }

    fn stroke_paint(&self) -> Paint<'static> {
        Paint {
            shader: Shader::SolidColor(self.effective_stroke_color()),
            blend_mode: self.blend_mode,
            anti_alias: true,
            ..Paint::default()
        }
    }

    /// Build a paint for the shadow pass: solid shadow colour, SourceOver blend.
    fn shadow_paint(color: Color) -> Paint<'static> {
        Paint {
            shader: Shader::SolidColor(color),
            blend_mode: TsBlendMode::SourceOver,
            anti_alias: true,
            ..Paint::default()
        }
    }

    /// If a shadow is active, draw `path` translated by the shadow offset onto
    /// a temporary pixmap that is blurred and then composited into `self.pixmap`.
    ///
    /// The blur radius is clamped to [0, 32] to keep the scratch pixmap bounded.
    fn draw_shadow_for_path(&mut self, path: &Path) {
        let Some(ref sh) = self.shadow.clone() else { return };

        let w = self.pixmap.width();
        let h = self.pixmap.height();
        let Some(mut shadow_pm) = Pixmap::new(w, h) else { return };

        // Draw the path shifted by (dx, dy) onto the scratch pixmap.
        let shadow_transform = self.transform.pre_translate(sh.dx, sh.dy);
        shadow_pm.fill_path(
            path,
            &Self::shadow_paint(sh.color),
            FillRule::Winding,
            shadow_transform,
            None,
        );

        // Box-blur the scratch pixmap to simulate Gaussian blur.
        // We apply a horizontal then vertical pass using an integer radius
        // derived from the blur parameter.
        let radius = (sh.blur.round() as u32).clamp(1, 32);
        box_blur_pixmap(&mut shadow_pm, radius);

        // Composite the blurred shadow onto the main pixmap (SourceOver).
        let shadow_data = shadow_pm.data().to_vec();
        if let Some(shadow_src) = Pixmap::from_vec(shadow_data, tiny_skia::IntSize::from_wh(w, h).expect("valid size")) {
            draw_pixmap_over(&mut self.pixmap, &shadow_src);
        }
    }

    /// Same as [`draw_shadow_for_path`] but for a filled rectangle (no path).
    fn draw_shadow_for_rect(&mut self, rect: Rect) {
        let Some(ref sh) = self.shadow.clone() else { return };

        let w = self.pixmap.width();
        let h = self.pixmap.height();
        let Some(mut shadow_pm) = Pixmap::new(w, h) else { return };

        let shadow_transform = self.transform.pre_translate(sh.dx, sh.dy);
        shadow_pm.fill_rect(rect, &Self::shadow_paint(sh.color), shadow_transform, None);

        let radius = (sh.blur.round() as u32).clamp(1, 32);
        box_blur_pixmap(&mut shadow_pm, radius);

        let shadow_data = shadow_pm.data().to_vec();
        if let Some(shadow_src) = Pixmap::from_vec(shadow_data, tiny_skia::IntSize::from_wh(w, h).expect("valid size")) {
            draw_pixmap_over(&mut self.pixmap, &shadow_src);
        }
    }

    fn current_stroke(&self) -> Stroke {
        let mut s = Stroke {
            width: self.stroke_width,
            line_cap: self.line_cap,
            line_join: self.line_join,
            ..Stroke::default()
        };
        if let Some(ref dash) = self.line_dash {
            s.dash = Some(dash.clone());
        }
        s
    }

    fn take_path(&mut self) -> Option<Path> {
        self.path_has_point = false;
        self.path_builder.take()?.finish()
    }

    fn builder(&mut self) -> &mut PathBuilder {
        self.path_builder.get_or_insert_with(PathBuilder::new)
    }

    /// Alpha-blend a glyph coverage byte onto a premultiplied RGBA8 destination pixel.
    fn composite_glyph_pixel(dst: &mut [u8], cr: u8, cg: u8, cb: u8, ca: u8, coverage: u8) {
        let alpha = (ca as u32 * coverage as u32 + 127) / 255;
        if alpha == 0 { return; }
        let inv  = 255 - alpha;
        dst[0] = ((cr as u32 * alpha + dst[0] as u32 * inv + 127) / 255) as u8;
        dst[1] = ((cg as u32 * alpha + dst[1] as u32 * inv + 127) / 255) as u8;
        dst[2] = ((cb as u32 * alpha + dst[2] as u32 * inv + 127) / 255) as u8;
        dst[3] = (alpha + dst[3] as u32 * inv / 255).min(255) as u8;
    }

    /// `fill_text`'s own rotation/shear-aware fallback — see this module's
    /// top-level "Rotated/sheared text fallback" doc comment for why this
    /// exists. Renders `text`'s own glyph OUTLINES as a real vector path
    /// (via `uzor::shaper::text_to_path` + [`parse_glyph_outline_path`])
    /// instead of the fast path's per-glyph raster blit — `Pixmap::
    /// fill_path` applies the FULL active `self.transform` (rotation/shear
    /// included), the same generic path-fill pipeline every other shape in
    /// this backend already goes through.
    ///
    /// `x_off`/`y_off` are the SAME text-align/baseline offsets `fill_text`
    /// itself already computed (so both code paths agree on where "the
    /// pen" sits relative to the caller's own `x`/`y`); `x`/`y` are
    /// `fill_text`'s own original arguments, unmodified.
    fn fill_text_as_path_impl(&mut self, text: &str, font_info: &FontInfo, x_off: f64, y_off: f64, x: f64, y: f64) {
        let svg = uzor::shaper::text_to_path(text, &font_css_string(font_info));
        if svg.is_empty() {
            return;
        }
        let Some(local_path) = parse_glyph_outline_path(&svg) else { return };

        // Position the LOCAL glyph-outline path at the caller's own pen
        // origin BEFORE applying the ambient `self.transform` — mirrors
        // `text_transform`/`combined` in `uzor-render-vello-cpu`/
        // `uzor-render-urx`'s own `fill_text` (same offset convention,
        // same reason: `self.transform` alone is the camera/rotation, the
        // text's own on-screen position is baked into the path first).
        let position = Transform::from_translate((x + x_off) as f32, (y + y_off) as f32);
        let Some(path) = local_path.transform(position) else { return };

        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint = self.fill_paint();
        let transform = self.transform;
        let clip = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }
}

// ---------------------------------------------------------------------------
// Painter trait implementation
// ---------------------------------------------------------------------------

impl Painter for TinySkiaCpuRenderContext {
    fn save(&mut self) {
        self.state_stack.push(SavedState {
            fill_color:    self.fill_color,
            stroke_color:  self.stroke_color,
            stroke_width:  self.stroke_width,
            line_cap:      self.line_cap,
            line_join:     self.line_join,
            global_alpha:  self.global_alpha,
            font_info:     self.font_info.clone(),
            text_align:    self.text_align,
            text_baseline: self.text_baseline,
            transform:     self.transform,
            clip:          self.current_clip.clone(),
        });
    }

    fn restore(&mut self) {
        if let Some(s) = self.state_stack.pop() {
            self.fill_color    = s.fill_color;
            self.stroke_color  = s.stroke_color;
            self.stroke_width  = s.stroke_width;
            self.line_cap      = s.line_cap;
            self.line_join     = s.line_join;
            self.global_alpha  = s.global_alpha;
            self.font_info     = s.font_info;
            self.text_align    = s.text_align;
            self.text_baseline = s.text_baseline;
            self.transform     = s.transform;
            self.current_clip  = s.clip;
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_translate(x as f32, y as f32);
    }

    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.pre_rotate(angle.to_degrees() as f32);
    }

    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_scale(x as f32, y as f32);
    }

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_css_color(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = (alpha as f32).clamp(0.0, 1.0);
    }

    fn set_fill_color_alpha(&mut self, color: &str, alpha: f64) {
        let base = parse_css_color(color);
        let a = (alpha as f32).clamp(0.0, 1.0);
        self.fill_color = with_alpha(base, a);
    }

    fn set_stroke_color(&mut self, color: &str) {
        self.stroke_color = parse_css_color(color);
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width as f32;
    }

    fn set_line_dash(&mut self, pattern: &[f64]) {
        if pattern.is_empty() {
            self.line_dash = None;
        } else {
            let intervals: Vec<f32> = pattern.iter().map(|&v| v as f32).collect();
            self.line_dash = StrokeDash::new(intervals, 0.0);
        }
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap {
            "round"  => LineCap::Round,
            "square" => LineCap::Square,
            _        => LineCap::Butt,
        };
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => LineJoin::Round,
            "bevel" => LineJoin::Bevel,
            _       => LineJoin::Miter,
        };
    }

    fn begin_path(&mut self) {
        self.path_builder = Some(PathBuilder::new());
        self.path_has_point = false;
    }

    fn move_to(&mut self, x: f64, y: f64) {
        self.builder().move_to(x as f32, y as f32);
        self.path_has_point = true;
    }

    fn line_to(&mut self, x: f64, y: f64) {
        self.builder().line_to(x as f32, y as f32);
        self.path_has_point = true;
    }

    fn close_path(&mut self) {
        self.builder().close();
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if let Some(r) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
            self.builder().push_rect(r);
        }
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        let sweep = (end_angle - start_angle).abs();
        if sweep >= std::f64::consts::TAU - 0.001 {
            let r = radius as f32;
            if let Some(rect) = Rect::from_xywh(
                (cx - radius) as f32,
                (cy - radius) as f32,
                r * 2.0,
                r * 2.0,
            ) {
                self.builder().push_oval(rect);
                self.path_has_point = true;
                return;
            }
        }
        let has_point = self.path_has_point;
        arc_to_cubics(
            self.builder(),
            cx as f32, cy as f32, radius as f32,
            start_angle as f32, end_angle as f32,
            has_point,
        );
        self.path_has_point = true;
    }

    fn ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        _rotation: f64,
        start: f64,
        end: f64,
    ) {
        let sweep = (end - start).abs();
        if sweep >= std::f64::consts::TAU - 0.001 {
            if let Some(rect) = Rect::from_xywh(
                (cx - rx) as f32,
                (cy - ry) as f32,
                (rx * 2.0) as f32,
                (ry * 2.0) as f32,
            ) {
                self.builder().push_oval(rect);
                self.path_has_point = true;
                return;
            }
        }
        let has_point = self.path_has_point;
        ellipse_to_cubics(
            self.builder(),
            cx as f32, cy as f32, rx as f32, ry as f32,
            start as f32, end as f32,
            has_point,
        );
        self.path_has_point = true;
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        self.builder().quad_to(cpx as f32, cpy as f32, x as f32, y as f32);
    }

    fn bezier_curve_to(
        &mut self,
        cp1x: f64, cp1y: f64,
        cp2x: f64, cp2y: f64,
        x: f64, y: f64,
    ) {
        self.builder().cubic_to(
            cp1x as f32, cp1y as f32,
            cp2x as f32, cp2y as f32,
            x as f32, y as f32,
        );
    }

    fn stroke(&mut self) {
        let Some(path) = self.take_path() else { return };
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.stroke_paint();
        let stroke    = self.current_stroke();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.stroke_path(&path, &paint, &stroke, transform, clip.as_ref());
    }

    fn fill(&mut self) {
        let Some(path) = self.take_path() else { return };
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.fill_paint();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl TextRenderer for TinySkiaCpuRenderContext {
    fn set_font(&mut self, font: &str) {
        self.font_info = parse_css_font(font);
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.text_baseline = baseline;
    }

    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() { return; }

        let font_info = self.font_info.clone();
        let font      = get_font(font_info.family, font_info.bold, font_info.italic);
        let px        = font_info.size;

        let total_w = measure_text_width(text, &font_info) as f32;
        let x_off   = match self.text_align {
            TextAlign::Center => -(total_w / 2.0),
            TextAlign::Right  => -total_w,
            TextAlign::Left   => 0.0,
        };

        let ascent = font.horizontal_line_metrics(px)
            .map(|m| m.ascent)
            .unwrap_or(px * 0.75);
        let y_off = match self.text_baseline {
            TextBaseline::Top        => ascent,
            TextBaseline::Middle     => ascent / 2.0,
            TextBaseline::Bottom     => 0.0,
            TextBaseline::Alphabetic => 0.0,
        };

        // See this module's own "Rotated/sheared text fallback" doc
        // comment: the raster blit below is rotation/shear-blind by
        // construction (reads only `tx`/`ty`/`sx`/`sy`), so a `ctx.
        // rotate()`/shear'd transform must go through the vector-path
        // fallback instead.
        if has_rotation_or_shear(self.transform) {
            self.fill_text_as_path_impl(text, &font_info, x_off as f64, y_off as f64, x, y);
            return;
        }

        let color = self.effective_fill_color();
        let cr = (color.red()   * 255.0) as u8;
        let cg = (color.green() * 255.0) as u8;
        let cb = (color.blue()  * 255.0) as u8;
        let ca = (color.alpha() * 255.0) as u8;

        let pw = self.pixmap.width()  as i32;
        let ph = self.pixmap.height() as i32;
        let stride = self.pixmap.width() as usize;

        let tx = self.transform.tx;
        let ty = self.transform.ty;
        let sx = self.transform.sx;
        let sy = self.transform.sy;

        let mut pen_x = (x as f32 + x_off) * sx + tx;
        let     pen_y = (y as f32 + y_off) * sy + ty;

        // Glyphs are rasterized at `render_scale` (clamped to >= 1x so small
        // scale factors don't blur the bitmap), so `metrics.advance_width` is
        // already expressed in device pixels at that scale. The pen must
        // therefore un-scale by `render_scale` before re-applying `sx`, or
        // advances get scaled twice (glyph bitmap at Nx, pen walk at N*sx).
        let render_scale = sx.max(sy).max(1.0);

        for ch in text.chars() {
            let render_px = px * render_scale;
            // Cached post-fallback glyph (see [`rasterize_cached`]'s own
            // doc — same NerdFont → … → NotoEmoji chain as before, paid
            // once per distinct glyph instead of per draw call).
            let glyph = rasterize_cached(font_info.family, font_info.bold, font_info.italic, ch, render_px);
            let metrics = glyph.metrics;
            let bitmap = &glyph.bitmap;

            let gw = metrics.width  as i32;
            let gh = metrics.height as i32;

            let gx0 = (pen_x + metrics.xmin as f32).round() as i32;
            let gy0 = (pen_y - metrics.ymin as f32 - gh as f32).round() as i32;

            for row in 0..gh {
                let py_coord = gy0 + row;
                if py_coord < 0 || py_coord >= ph { continue; }
                for col in 0..gw {
                    let px_coord = gx0 + col;
                    if px_coord < 0 || px_coord >= pw { continue; }
                    let coverage = bitmap[(row * gw + col) as usize];
                    if coverage == 0 { continue; }

                    let dst_idx = py_coord as usize * stride + px_coord as usize;
                    let dst_off = dst_idx * 4;
                    let data    = self.pixmap.data_mut();
                    let dst     = &mut data[dst_off..dst_off + 4];
                    Self::composite_glyph_pixel(dst, cr, cg, cb, ca, coverage);
                }
            }

            pen_x += metrics.advance_width * (sx / render_scale);
        }
    }

    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        let saved_fill = self.fill_color;
        self.fill_color = self.stroke_color;
        self.fill_text(text, x, y);
        self.fill_color = saved_fill;
    }
}

// ---------------------------------------------------------------------------
// TextMetrics
// ---------------------------------------------------------------------------

impl TextMetrics for TinySkiaCpuRenderContext {
    /// Delegates to [`uzor::shaper`] (cosmic-text) — see this module's own
    /// top-of-crate divergence note (`measure_text_width`'s doc comment)
    /// for why: fontdue's own per-glyph advance sum never applies GPOS
    /// kerning, so it disagreed with every OTHER backend in this
    /// workspace (all of which already measure `measure_text_glyphs`/
    /// `measure_text_wrapped`/`text_to_path` through the SAME shaper) —
    /// a real cross-backend layout divergence, not just a cosmetic
    /// rendering difference (`uzor-figures`' `guide::labeler` collision
    /// pass feeds a backend's own `measure_text` result directly into a
    /// pass/fail occupancy check, so a few tenths of a pixel of kerning
    /// drift could flip which candidate slot a label claims).
    fn measure_text(&self, text: &str) -> f64 {
        let font_str = font_css_string(&self.font_info);
        let glyphs = uzor::shaper::measure_glyphs(text, &font_str);
        glyphs.last().map(|g| g.x_offset + g.advance).unwrap_or(0.0)
    }

    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        let font_info = parse_css_font(font);
        let px = font_info.size;
        let glyphs = uzor::shaper::measure_glyphs(text, font);
        let w = glyphs.last().map(|g| g.x_offset + g.advance).unwrap_or(0.0);
        let fontdue_font = get_font(font_info.family, font_info.bold, font_info.italic);
        let (ascent, descent) = fontdue_font
            .horizontal_line_metrics(px)
            .map(|m| (m.ascent as f64, (-m.descent) as f64))
            .unwrap_or_else(|| (px as f64 * 0.9, px as f64 * 0.3));
        let h = ascent + descent;
        TextBounds {
            x: 0.0,
            y: -ascent,
            w,
            h,
            ascent,
            descent,
        }
    }

    /// Real cluster shaping via cosmic-text.
    ///
    /// Correctly handles Unicode grapheme clusters (`é` as one cluster),
    /// emoji ZWJ sequences, and returns visual left-to-right order for LTR text.
    /// Results are cached per `(font, text)` pair (unbounded cache for Phase 4).
    fn measure_text_glyphs(&self, text: &str, font: &str) -> Vec<uzor::render::GlyphMetric> {
        uzor::shaper::measure_glyphs(text, font)
    }

    /// Real word-wrap via cosmic-text `Wrap::Word`.
    ///
    /// Delegates to [`uzor::shaper::measure_glyphs_wrapped`], which owns its
    /// own `(font, text, max_width)`-keyed cache separate from the unwrapped
    /// `measure_glyphs`/`text_to_path` cache.
    fn measure_text_wrapped(&self, text: &str, font: &str, max_width: f64) -> Vec<uzor::render::WrappedLine> {
        uzor::shaper::measure_glyphs_wrapped(text, font, max_width)
    }

    /// Glyph outlines as an SVG path `d` string via cosmic-text + swash.
    ///
    /// Delegates to [`uzor::shaper::text_to_path`] which uses a
    /// process-wide [`cosmic_text::SwashCache`] for outline scaling.
    fn text_to_path(&self, text: &str, font: &str) -> String {
        uzor::shaper::text_to_path(text, font)
    }
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

impl Masking for TinySkiaCpuRenderContext {
    fn clip(&mut self) {
        let Some(path) = self.take_path() else { return };
        let w = self.pixmap.width();
        let h = self.pixmap.height();
        if let Some(mut mask) = Mask::new(w, h) {
            mask.fill_path(&path, FillRule::Winding, true, self.transform);
            self.current_clip = Some(mask);
        }
    }
    // push_mask / pop_mask / clip_rect: use default impls (save+clip / restore)

    /// Even-odd fill rule override: builds a mask with `FillRule::EvenOdd`
    /// so two-subpath paths (outer rect + inner shape) produce a ring-shaped clip.
    fn push_clip_svg_path_even_odd(&mut self, d: &str) {
        // emit_svg_path calls begin_path + emits all path commands onto self.
        uzor::render::emit_svg_path(self, d);
        let Some(path) = self.take_path() else { return };
        let w = self.pixmap.width();
        let h = self.pixmap.height();
        if let Some(mut mask) = Mask::new(w, h) {
            mask.fill_path(&path, FillRule::EvenOdd, true, self.transform);
            self.save();
            self.current_clip = Some(mask);
        }
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

impl Effects for TinySkiaCpuRenderContext {
    fn set_shadow(&mut self, dx: f64, dy: f64, blur: f64, color: &str) {
        let parsed = parse_css_color(color);
        let color = if self.global_alpha < 1.0 {
            with_alpha(parsed, self.global_alpha)
        } else {
            parsed
        };
        self.shadow = Some(ShadowState {
            dx:   dx as f32,
            dy:   dy as f32,
            blur: blur.max(0.0) as f32,
            color,
        });
    }

    fn clear_shadow(&mut self) {
        self.shadow = None;
    }

    fn set_blend_mode(&mut self, mode: UzorBlendMode) {
        self.blend_mode = match mode {
            UzorBlendMode::Normal     => TsBlendMode::SourceOver,
            UzorBlendMode::Multiply   => TsBlendMode::Multiply,
            UzorBlendMode::Screen     => TsBlendMode::Screen,
            UzorBlendMode::Overlay    => TsBlendMode::Overlay,
            UzorBlendMode::Darken     => TsBlendMode::Darken,
            UzorBlendMode::Lighten    => TsBlendMode::Lighten,
            UzorBlendMode::ColorDodge => TsBlendMode::ColorDodge,
            UzorBlendMode::ColorBurn  => TsBlendMode::ColorBurn,
            UzorBlendMode::HardLight  => TsBlendMode::HardLight,
            UzorBlendMode::SoftLight  => TsBlendMode::SoftLight,
            UzorBlendMode::Difference => TsBlendMode::Difference,
            UzorBlendMode::Exclusion  => TsBlendMode::Exclusion,
            UzorBlendMode::Plus       => TsBlendMode::Plus,
        };
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers
// ---------------------------------------------------------------------------

impl ShapeHelpers for TinySkiaCpuRenderContext {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) else {
            return;
        };
        if self.shadow.is_some() {
            self.draw_shadow_for_rect(rect);
        }
        let paint     = self.fill_paint();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_rect(rect, &paint, transform, clip.as_ref());
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) else {
            return;
        };
        let mut pb = PathBuilder::new();
        pb.push_rect(rect);
        let Some(path) = pb.finish() else { return };
        let paint     = self.stroke_paint();
        let stroke    = self.current_stroke();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.stroke_path(&path, &paint, &stroke, transform, clip.as_ref());
    }

    fn rounded_rect_corners(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        tl: f64,
        tr: f64,
        br: f64,
        bl: f64,
    ) {
        let max_r = (w / 2.0).min(h / 2.0).max(0.0);
        let tl = (tl as f32).min(max_r as f32).max(0.0);
        let tr = (tr as f32).min(max_r as f32).max(0.0);
        let br = (br as f32).min(max_r as f32).max(0.0);
        let bl = (bl as f32).min(max_r as f32).max(0.0);

        let x  = x as f32;
        let y  = y as f32;
        let w  = w as f32;
        let h  = h as f32;

        let pb = self.builder();
        pb.move_to(x + tl, y);

        pb.line_to(x + w - tr, y);
        if tr > 0.0 {
            arc_to_cubics(pb, x + w - tr, y + tr, tr, -PI / 2.0, 0.0, true);
        }

        pb.line_to(x + w, y + h - br);
        if br > 0.0 {
            arc_to_cubics(pb, x + w - br, y + h - br, br, 0.0, PI / 2.0, true);
        }

        pb.line_to(x + bl, y + h);
        if bl > 0.0 {
            arc_to_cubics(pb, x + bl, y + h - bl, bl, PI / 2.0, PI, true);
        }

        pb.line_to(x, y + tl);
        if tl > 0.0 {
            arc_to_cubics(pb, x + tl, y + tl, tl, PI, PI * 1.5, true);
        }

        pb.close();
        self.path_has_point = true;
    }
}

// ---------------------------------------------------------------------------
// BatchPainter — optimized: single merged Path per call, one stroke/fill
// ---------------------------------------------------------------------------

impl BatchPainter for TinySkiaCpuRenderContext {
    fn draw_line_batch(&mut self, lines: &[LineSegment], color: &str, width: f64) {
        if lines.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut pb = PathBuilder::new();
        for l in lines {
            pb.move_to(l.x1 as f32, l.y1 as f32);
            pb.line_to(l.x2 as f32, l.y2 as f32);
        }
        let Some(path) = pb.finish() else { return };
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.stroke_paint();
        let stroke    = self.current_stroke();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.stroke_path(&path, &paint, &stroke, transform, clip.as_ref());
    }

    fn draw_circle_batch(&mut self, circles: &[CircleBatch], color: &str) {
        if circles.is_empty() {
            return;
        }
        self.set_fill_color(color);
        let mut pb = PathBuilder::new();
        for c in circles {
            let r = c.r as f32;
            if let Some(rect) = Rect::from_xywh(
                (c.cx - c.r) as f32,
                (c.cy - c.r) as f32,
                r * 2.0,
                r * 2.0,
            ) {
                pb.push_oval(rect);
            }
        }
        let Some(path) = pb.finish() else { return };
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.fill_paint();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }

    fn stroke_polyline(&mut self, pts: &[(f64, f64)], color: &str, width: f64) {
        if pts.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0 as f32, pts[0].1 as f32);
        for &(x, y) in &pts[1..] {
            pb.line_to(x as f32, y as f32);
        }
        let Some(path) = pb.finish() else { return };
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.stroke_paint();
        let stroke    = self.current_stroke();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.stroke_path(&path, &paint, &stroke, transform, clip.as_ref());
    }
}

// ---------------------------------------------------------------------------
// GradientPainter
// ---------------------------------------------------------------------------

impl GradientPainter for TinySkiaCpuRenderContext {
    fn fill_linear_gradient(
        &mut self,
        stops: &[(f32, &str)],
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        let Some(path) = self.take_path() else { return };

        if stops.is_empty() {
            return;
        }

        let gradient_stops: Vec<GradientStop> = stops
            .iter()
            .map(|&(pos, color_str)| {
                let color = parse_css_color(color_str);
                let color = if self.global_alpha < 1.0 {
                    with_alpha(color, self.global_alpha)
                } else {
                    color
                };
                GradientStop::new(pos.clamp(0.0, 1.0), color)
            })
            .collect();

        let start = Point::from_xy(x1 as f32, y1 as f32);
        let end   = Point::from_xy(x2 as f32, y2 as f32);

        let shader = LinearGradient::new(
            start,
            end,
            gradient_stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or_else(|| {
            let color = parse_css_color(stops[0].1);
            let color = if self.global_alpha < 1.0 {
                with_alpha(color, self.global_alpha)
            } else {
                color
            };
            Shader::SolidColor(color)
        });

        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }

    fn fill_radial_gradient(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        stops: &[(f32, &str)],
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) {
        let _ = (x, y, w, h);
        let Some(path) = self.take_path() else { return };

        if stops.is_empty() {
            return;
        }

        let gradient_stops: Vec<GradientStop> = stops
            .iter()
            .map(|&(pos, color_str)| {
                let color = parse_css_color(color_str);
                let color = if self.global_alpha < 1.0 {
                    with_alpha(color, self.global_alpha)
                } else {
                    color
                };
                GradientStop::new(pos.clamp(0.0, 1.0), color)
            })
            .collect();

        let center = Point::from_xy(cx as f32, cy as f32);

        let shader = RadialGradient::new(
            center,
            center,
            r as f32,
            gradient_stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or_else(|| {
            let color = parse_css_color(stops[0].1);
            let color = if self.global_alpha < 1.0 {
                with_alpha(color, self.global_alpha)
            } else {
                color
            };
            Shader::SolidColor(color)
        });

        let paint = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }
}

// ---------------------------------------------------------------------------
// UiEffectHelpers — override blur methods (tiny-skia has native box-blur)
// ---------------------------------------------------------------------------

impl UiEffectHelpers for TinySkiaCpuRenderContext {
    fn has_blur_background(&self) -> bool {
        true
    }

    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        let radius = self.blur_radius;
        if radius < 1.0 || width < 1.0 || height < 1.0 {
            return;
        }

        let margin = (radius * 3.0).round() as i32;
        let w = width.round()  as u32;
        let h = height.round() as u32;
        let sw = w.saturating_add((2 * margin as u32).min(u32::MAX - w));
        let sh = h.saturating_add((2 * margin as u32).min(u32::MAX - h));

        if sw == 0 || sh == 0 { return; }

        let src_x = x.round() as i32 - margin;
        let src_y = y.round() as i32 - margin;

        let mut scratch = match copy_pixmap_region(&self.pixmap, src_x, src_y, sw, sh) {
            Some(p) => p,
            None    => return,
        };

        let r = (radius.round() as u32).clamp(1, 128);
        for _ in 0..3 {
            box_blur_pixmap(&mut scratch, r);
        }

        let dst_x = x.round() as i32;
        let dst_y = y.round() as i32;
        blit_region(&mut self.pixmap, &scratch, dst_x, dst_y, w, h, margin);
    }
}

// ---------------------------------------------------------------------------
// BackdropBlur — tiny-skia implements native box-blur
// ---------------------------------------------------------------------------

impl BackdropBlur for TinySkiaCpuRenderContext {
    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        UiEffectHelpers::draw_blur_background(self, x, y, width, height);
    }

    fn has_blur_background(&self) -> bool {
        true
    }

    fn use_convex_glass_buttons(&self) -> bool {
        false
    }

    fn draw_glass_button_3d(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        _is_active: bool,
        color: &str,
    ) {
        UiEffectHelpers::draw_blur_background(self, x, y, width, height);
        self.set_fill_color(color);
        self.fill_rounded_rect(x, y, width, height, radius);
    }
}

// ---------------------------------------------------------------------------
// ImagePainter — tiny-skia does not load URL-based images (`draw_image` stays
// `false`), but `draw_image_rgba` is a REAL blit: the caller's straight-alpha
// RGBA bytes are premultiplied once, wrapped in a `tiny_skia::PixmapRef`, and
// composited via `Pixmap::draw_pixmap`'s own Pattern-shader resampling — the
// exact same transform-composition + `PixmapPaint` shape `draw_cached_target`
// already uses to place a cached offscreen target (see `UzorRenderContext`
// impl below), just with a caller-supplied pixmap instead of a cached one.
// ---------------------------------------------------------------------------

impl ImagePainter for TinySkiaCpuRenderContext {
    fn draw_image(
        &mut self,
        _image_id: &str,
        _x: f64,
        _y: f64,
        _width: f64,
        _height: f64,
    ) -> bool {
        false
    }

    fn draw_image_rgba(
        &mut self,
        data: &[u8],
        img_width: u32,
        img_height: u32,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) {
        if img_width == 0 || img_height == 0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let expected_len = img_width as usize * img_height as usize * 4;
        if data.len() != expected_len {
            return;
        }

        // `tiny_skia::Pixmap`/`PixmapRef` require PREMULTIPLIED alpha, while
        // `ImagePainter::draw_image_rgba`'s own contract (see its doc
        // comment) is straight alpha, row-major, top-to-bottom — premultiply
        // once into a scratch buffer before handing it to the Pattern shader.
        let mut premultiplied = vec![0u8; expected_len];
        for (src, dst) in data.chunks_exact(4).zip(premultiplied.chunks_exact_mut(4)) {
            let a = src[3] as u32;
            dst[0] = ((src[0] as u32 * a + 127) / 255) as u8;
            dst[1] = ((src[1] as u32 * a + 127) / 255) as u8;
            dst[2] = ((src[2] as u32 * a + 127) / 255) as u8;
            dst[3] = src[3];
        }

        let Some(src_pixmap) =
            tiny_skia::PixmapRef::from_bytes(&premultiplied, img_width, img_height)
        else {
            return;
        };

        // Scale from source-pixel space to the target rect's size, then
        // translate to the target rect's origin, then apply whatever
        // canvas transform is currently active — same translate-then-scale
        // composition order `draw_cached_target` uses below for a cached
        // target, so `ctx.scale(..)`/`ctx.translate(..)` affect an RGBA
        // blit exactly the same way they affect every other draw call.
        let scale_x = width as f32 / img_width as f32;
        let scale_y = height as f32 / img_height as f32;
        let transform = self
            .transform
            .pre_translate(x as f32, y as f32)
            .pre_scale(scale_x, scale_y);

        let paint = tiny_skia::PixmapPaint {
            opacity: self.global_alpha,
            quality: tiny_skia::FilterQuality::Bilinear,
            blend_mode: self.blend_mode,
            ..tiny_skia::PixmapPaint::default()
        };

        let clip = self.current_clip.clone();
        self.pixmap
            .draw_pixmap(0, 0, src_pixmap, &paint, transform, clip.as_ref());
    }
}

// ---------------------------------------------------------------------------
// RenderContext compound trait
// ---------------------------------------------------------------------------

impl UzorRenderContext for TinySkiaCpuRenderContext {
    fn dpr(&self) -> f64 {
        self.dpr
    }

    fn image_painter(&mut self) -> Option<&mut dyn ImagePainter> {
        Some(self)
    }

    fn supports_offscreen_targets(&self) -> bool {
        true
    }

    fn push_offscreen_target(&mut self, desc: OffscreenTargetDesc) -> OffscreenTarget {
        let w = desc.width_px.max(1);
        let h = desc.height_px.max(1);
        let fresh = Pixmap::new(w, h)?;

        let id = OffscreenTargetId(self.next_offscreen_id);
        self.next_offscreen_id += 1;

        // Swap the fresh (transparent) offscreen pixmap in; stash the
        // previously-active pixmap (screen or an outer offscreen target,
        // for nested boundaries) plus the outer drawing state.
        let previous = std::mem::replace(&mut self.pixmap, fresh);
        self.offscreen_stack.push(OffscreenRecording {
            id,
            pixmap:         previous,
            transform:      self.transform,
            current_clip:   self.current_clip.take(),
            state_stack:    std::mem::take(&mut self.state_stack),
            path_builder:   self.path_builder.take(),
            path_has_point: std::mem::take(&mut self.path_has_point),
        });

        // The offscreen subtree paints at its own local origin — fresh
        // state, unclipped, identity transform.
        self.transform = Transform::identity();

        Some(id)
    }

    fn pop_offscreen_target(&mut self) {
        let Some(saved) = self.offscreen_stack.pop() else {
            return;
        };
        // The pixmap just painted into `self.pixmap` becomes the cached
        // target content; restore the previously-active surface and the
        // outer drawing state.
        let painted = std::mem::replace(&mut self.pixmap, saved.pixmap);
        self.offscreen_targets.insert(saved.id, painted);
        self.transform      = saved.transform;
        self.current_clip   = saved.current_clip;
        self.state_stack    = saved.state_stack;
        self.path_builder   = saved.path_builder;
        self.path_has_point = saved.path_has_point;
    }

    fn draw_cached_target(&mut self, id: OffscreenTargetId, dst_rect: UzorRect) -> bool {
        let Some(pixmap) = self.offscreen_targets.get(&id) else {
            return false;
        };

        let sw = pixmap.width().max(1) as f32;
        let sh = pixmap.height().max(1) as f32;
        let dw = dst_rect.width as f32;
        let dh = dst_rect.height as f32;
        if dw <= 0.0 || dh <= 0.0 {
            return false;
        }

        let scale_x = dw / sw;
        let scale_y = dh / sh;
        let transform = self
            .transform
            .pre_concat(Transform::from_translate(dst_rect.x as f32, dst_rect.y as f32))
            .pre_concat(Transform::from_scale(scale_x, scale_y));

        let paint = tiny_skia::PixmapPaint {
            opacity: self.global_alpha,
            quality: tiny_skia::FilterQuality::Bilinear,
            ..tiny_skia::PixmapPaint::default()
        };

        let clip = self.current_clip.clone();
        self.pixmap
            .draw_pixmap(0, 0, pixmap.as_ref(), &paint, transform, clip.as_ref());
        true
    }

    fn resize_offscreen_target(&mut self, id: OffscreenTargetId, desc: OffscreenTargetDesc) -> bool {
        let Some(existing) = self.offscreen_targets.get_mut(&id) else {
            return false;
        };
        let w = desc.width_px.max(1);
        let h = desc.height_px.max(1);
        let Some(fresh) = Pixmap::new(w, h) else {
            return false;
        };
        *existing = fresh;
        true
    }

    fn free_offscreen_target(&mut self, id: OffscreenTargetId) {
        self.offscreen_targets.remove(&id);
    }
}

// ---------------------------------------------------------------------------
// RenderContextExt — blur/glass effects (no-op for CPU backend)
// ---------------------------------------------------------------------------

impl RenderContextExt for TinySkiaCpuRenderContext {
    /// CPU backend carries no blur image state.
    type BlurImage = ();

    fn set_blur_image(&mut self, _image: Option<()>, _width: u32, _height: u32) {
        // Not supported in software rasterizer.
    }

    fn set_use_convex_glass_buttons(&mut self, _use_convex: bool) {
        // Not supported in software rasterizer.
    }
}
