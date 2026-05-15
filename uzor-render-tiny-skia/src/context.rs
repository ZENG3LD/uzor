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
    BlendMode as UzorBlendMode, RenderContext as UzorRenderContext, RenderContextExt, TextAlign,
    TextBaseline,
};

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
static FONT_NERD_FONT: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_SYMBOLS: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_COLOR_EMOJI: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_EMOJI: OnceLock<fontdue::Font> = OnceLock::new();

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
// Text width measurement via fontdue
// ---------------------------------------------------------------------------

fn measure_text_width(text: &str, font_info: &FontInfo) -> f64 {
    let font = get_font(font_info.family, font_info.bold, font_info.italic);
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (metrics, _) = font.rasterize(ch, font_info.size);
        let advance = if metrics.width == 0 && !ch.is_whitespace() {
            let (nf_metrics, _) = get_nerd_font().rasterize(ch, font_info.size);
            if nf_metrics.width > 0 {
                nf_metrics.advance_width
            } else {
                let (fb_metrics, _) = get_symbols_font().rasterize(ch, font_info.size);
                if fb_metrics.width > 0 {
                    fb_metrics.advance_width
                } else {
                    let (cv_metrics, _) = get_color_emoji_font().rasterize(ch, font_info.size);
                    if cv_metrics.width > 0 {
                        cv_metrics.advance_width
                    } else {
                        let (em_metrics, _) = get_emoji_font().rasterize(ch, font_info.size);
                        em_metrics.advance_width
                    }
                }
            }
        } else {
            metrics.advance_width
        };
        width += advance;
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
    // Cubic bezier magic constant for arc approximation
    let k = (4.0 / 3.0) * ((seg_angle / 2.0).abs().tan());

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
    let k = (4.0 / 3.0) * ((seg_angle / 2.0).abs().tan());

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
        }
    }

    /// Resize the pixmap.  Content is cleared.
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(pm) = Pixmap::new(width.max(1), height.max(1)) {
            self.pixmap      = pm;
            self.current_clip = None;
        }
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
}

// ---------------------------------------------------------------------------
// RenderContext trait implementation
// ---------------------------------------------------------------------------

impl UzorRenderContext for TinySkiaCpuRenderContext {
    // -----------------------------------------------------------------------
    // Dimensions
    // -----------------------------------------------------------------------

    fn dpr(&self) -> f64 {
        self.dpr
    }

    // -----------------------------------------------------------------------
    // Stroke style
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // Fill style
    // -----------------------------------------------------------------------

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_css_color(color);
    }

    fn set_fill_color_alpha(&mut self, color: &str, alpha: f64) {
        let base = parse_css_color(color);
        let a = (alpha as f32).clamp(0.0, 1.0);
        self.fill_color = with_alpha(base, a);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = (alpha as f32).clamp(0.0, 1.0);
    }

    // -----------------------------------------------------------------------
    // Path operations
    // -----------------------------------------------------------------------

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
        // Full circle — use native push_circle for perfect results
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
        // Full ellipse — use native push_oval
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

    // -----------------------------------------------------------------------
    // Stroke / fill the current path
    // -----------------------------------------------------------------------

    fn fill(&mut self) {
        let Some(path) = self.take_path() else { return };
        // M6-P1: draw shadow before the actual fill
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.fill_paint();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }

    fn fill_linear_gradient(
        &mut self,
        stops: &[(f32, &str)],
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        let Some(path) = self.take_path() else { return };

        // Need at least one stop to draw anything.
        if stops.is_empty() {
            return;
        }

        // Build tiny-skia GradientStop list from (offset, color_hex) pairs.
        let gradient_stops: Vec<GradientStop> = stops
            .iter()
            .map(|&(pos, color_str)| {
                let color = parse_css_color(color_str);
                // Apply global alpha to each stop's color.
                let color = if self.global_alpha < 1.0 {
                    with_alpha(color, self.global_alpha)
                } else {
                    color
                };
                GradientStop::new(pos.clamp(0.0, 1.0), color)
            })
            .collect();

        // tiny-skia requires the start and end points to differ; if they are
        // identical the gradient is degenerate — fall back to a solid fill with
        // the first stop color so rendering is always correct.
        let start = Point::from_xy(x1 as f32, y1 as f32);
        let end   = Point::from_xy(x2 as f32, y2 as f32);

        // LinearGradient::new() already returns Option<Shader<'static>>.
        // It returns Shader::SolidColor when start == end (degenerate) or
        // when only one stop is given.
        let shader = LinearGradient::new(
            start,
            end,
            gradient_stops,
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or_else(|| {
            // Fallback: solid fill with first stop color.
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
        // Discard the bounding rect — we paint whatever path was built before
        // this call, matching how fill_linear_gradient works.
        let _ = (x, y, w, h);
        let Some(path) = self.take_path() else { return };

        if stops.is_empty() {
            return;
        }

        // Build tiny-skia GradientStop list.
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

        // For a simple radial gradient start == end (same centre point).
        // tiny-skia `RadialGradient::new(start, end, radius, ...)` with
        // start == end implements the standard "SkRadialGradient" behaviour.
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
            // Degenerate (r ≤ 0 or no stops) — solid fallback with first stop.
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

    fn stroke(&mut self) {
        let Some(path) = self.take_path() else { return };
        // M6-P1: draw shadow before the actual stroke
        if self.shadow.is_some() {
            self.draw_shadow_for_path(&path);
        }
        let paint     = self.stroke_paint();
        let stroke    = self.current_stroke();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.stroke_path(&path, &paint, &stroke, transform, clip.as_ref());
    }

    fn clip(&mut self) {
        let Some(path) = self.take_path() else { return };
        let w = self.pixmap.width();
        let h = self.pixmap.height();
        if let Some(mut mask) = Mask::new(w, h) {
            mask.fill_path(&path, FillRule::Winding, true, self.transform);
            self.current_clip = Some(mask);
        }
    }

    // -----------------------------------------------------------------------
    // Shape helpers
    // -----------------------------------------------------------------------

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let Some(rect) = Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) else {
            return;
        };
        // M6-P1: draw shadow before the actual fill
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

    // -----------------------------------------------------------------------
    // Per-corner rounded rectangle
    // -----------------------------------------------------------------------

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

        // Build the path directly into path_builder (mirrors rounded_rect geometry).
        // Start at the top edge, just right of the top-left corner arc.
        let pb = self.builder();
        pb.move_to(x + tl, y);

        // Top edge → top-right arc
        pb.line_to(x + w - tr, y);
        if tr > 0.0 {
            arc_to_cubics(pb, x + w - tr, y + tr, tr, -PI / 2.0, 0.0, true);
        }

        // Right edge → bottom-right arc
        pb.line_to(x + w, y + h - br);
        if br > 0.0 {
            arc_to_cubics(pb, x + w - br, y + h - br, br, 0.0, PI / 2.0, true);
        }

        // Bottom edge → bottom-left arc
        pb.line_to(x + bl, y + h);
        if bl > 0.0 {
            arc_to_cubics(pb, x + bl, y + h - bl, bl, PI / 2.0, PI, true);
        }

        // Left edge → top-left arc
        pb.line_to(x, y + tl);
        if tl > 0.0 {
            arc_to_cubics(pb, x + tl, y + tl, tl, PI, PI * 1.5, true);
        }

        pb.close();
        self.path_has_point = true;
    }

    // -----------------------------------------------------------------------
    // Text rendering
    // -----------------------------------------------------------------------

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

        // Measure total advance width for alignment offset
        let total_w = measure_text_width(text, &font_info) as f32;
        let x_off   = match self.text_align {
            TextAlign::Center => -(total_w / 2.0),
            TextAlign::Right  => -total_w,
            TextAlign::Left   => 0.0,
        };

        // Use real font metrics for vertical alignment
        let ascent = font.horizontal_line_metrics(px)
            .map(|m| m.ascent)
            .unwrap_or(px * 0.75);
        let y_off = match self.text_baseline {
            TextBaseline::Top        => ascent,
            TextBaseline::Middle     => ascent / 2.0,
            TextBaseline::Bottom     => 0.0,
            TextBaseline::Alphabetic => 0.0,
        };

        // Effective fill color components
        let color = self.effective_fill_color();
        let cr = (color.red()   * 255.0) as u8;
        let cg = (color.green() * 255.0) as u8;
        let cb = (color.blue()  * 255.0) as u8;
        let ca = (color.alpha() * 255.0) as u8;

        let pw = self.pixmap.width()  as i32;
        let ph = self.pixmap.height() as i32;
        let stride = self.pixmap.width() as usize;

        // Extract translation and uniform scale from the current transform
        let tx = self.transform.tx;
        let ty = self.transform.ty;
        let sx = self.transform.sx;
        let sy = self.transform.sy;

        // Starting pen position in pixmap space
        let mut pen_x = (x as f32 + x_off) * sx + tx;
        let     pen_y = (y as f32 + y_off) * sy + ty;

        for ch in text.chars() {
            let render_px = px * sx.max(sy).max(1.0);
            let (primary_metrics, primary_bitmap) = font.rasterize(ch, render_px);

            // Select which font's rasterization result to use.  If the primary
            // font returns a zero-width glyph for a non-whitespace character it
            // means the codepoint is not covered — try the fallback fonts in order.
            let (metrics, bitmap) = if primary_metrics.width == 0 && !ch.is_whitespace() {
                let (nf_metrics, nf_bitmap) = get_nerd_font().rasterize(ch, render_px);
                if nf_metrics.width > 0 {
                    (nf_metrics, nf_bitmap)
                } else {
                    let (sym_metrics, sym_bitmap) = get_symbols_font().rasterize(ch, render_px);
                    if sym_metrics.width > 0 {
                        (sym_metrics, sym_bitmap)
                    } else {
                        let (cv_metrics, cv_bitmap) = get_color_emoji_font().rasterize(ch, render_px);
                        if cv_metrics.width > 0 {
                            (cv_metrics, cv_bitmap)
                        } else {
                            let (em_metrics, em_bitmap) = get_emoji_font().rasterize(ch, render_px);
                            if em_metrics.width > 0 {
                                (em_metrics, em_bitmap)
                            } else {
                                // No font covers this codepoint — use primary result
                                // (advance_width may still be non-zero for spacing).
                                (primary_metrics, primary_bitmap)
                            }
                        }
                    }
                }
            } else {
                (primary_metrics, primary_bitmap)
            };

            let gw = metrics.width  as i32;
            let gh = metrics.height as i32;

            // Glyph top-left corner in pixmap coordinates.
            // fontdue's ymin is the offset from the baseline to the bottom of the glyph.
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

            pen_x += metrics.advance_width * sx;
        }
    }

    fn stroke_text(&mut self, text: &str, x: f64, y: f64) {
        // tiny-skia has no native text stroke path API.
        // Render with fill using the stroke color so callers get visible output.
        let saved_fill = self.fill_color;
        self.fill_color = self.stroke_color;
        self.fill_text(text, x, y);
        self.fill_color = saved_fill;
    }

    fn measure_text(&self, text: &str) -> f64 {
        measure_text_width(text, &self.font_info)
    }

    // -----------------------------------------------------------------------
    // Transform operations
    // -----------------------------------------------------------------------

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
        // `pre_rotate` expects degrees
        self.transform = self.transform.pre_rotate(angle.to_degrees() as f32);
    }

    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_scale(x as f32, y as f32);
    }

    // -----------------------------------------------------------------------
    // M6-P1: Drop shadow
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // M6-P2: Mask layers
    //
    // Default impl (save + clip) is correct for tiny-skia: the clip() impl
    // writes into current_clip (a tiny_skia::Mask) and save/restore snapshots
    // it.  No override needed.
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // M6-P3: Blend mode
    // -----------------------------------------------------------------------

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
