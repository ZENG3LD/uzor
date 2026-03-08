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
    Color, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap, Rect,
    Shader, Stroke, StrokeDash, Transform,
};

use uzor_core::render::{RenderContext as UzorRenderContext, RenderContextExt, TextAlign, TextBaseline};

// ---------------------------------------------------------------------------
// Embedded fonts
// ---------------------------------------------------------------------------

static ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

// ---------------------------------------------------------------------------
// Cached fontdue fonts (one per process)
// ---------------------------------------------------------------------------

static FONT_REGULAR: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_BOLD: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_ITALIC: OnceLock<fontdue::Font> = OnceLock::new();
static FONT_BOLD_ITALIC: OnceLock<fontdue::Font> = OnceLock::new();

fn get_font(bold: bool, italic: bool) -> &'static fontdue::Font {
    fn make_font(bytes: &[u8]) -> fontdue::Font {
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .expect("embedded font is valid")
    }
    match (bold, italic) {
        (true, true)   => FONT_BOLD_ITALIC.get_or_init(|| make_font(ROBOTO_BOLD_ITALIC)),
        (true, false)  => FONT_BOLD.get_or_init(|| make_font(ROBOTO_BOLD)),
        (false, true)  => FONT_ITALIC.get_or_init(|| make_font(ROBOTO_ITALIC)),
        (false, false) => FONT_REGULAR.get_or_init(|| make_font(ROBOTO_REGULAR)),
    }
}

// ---------------------------------------------------------------------------
// CSS color parsing
// ---------------------------------------------------------------------------

fn parse_css_color(s: &str) -> Color {
    let s = s.trim();

    // rgba(r, g, b, a)
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|x| x.strip_suffix(')')) {
        let p: Vec<&str> = inner.split(',').map(str::trim).collect();
        if p.len() == 4 {
            let r = p[0].parse::<u8>().unwrap_or(0);
            let g = p[1].parse::<u8>().unwrap_or(0);
            let b = p[2].parse::<u8>().unwrap_or(0);
            let a = p[3].parse::<f64>().map(|v| {
                if v <= 1.0 { (v * 255.0) as u8 } else { v as u8 }
            }).unwrap_or(255);
            return Color::from_rgba8(r, g, b, a);
        }
    }

    // rgb(r, g, b)
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        let p: Vec<&str> = inner.split(',').map(str::trim).collect();
        if p.len() == 3 {
            let r = p[0].parse::<u8>().unwrap_or(0);
            let g = p[1].parse::<u8>().unwrap_or(0);
            let b = p[2].parse::<u8>().unwrap_or(0);
            return Color::from_rgba8(r, g, b, 255);
        }
    }

    let hex = s.trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0) * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0) * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0) * 17;
            Color::from_rgba8(r, g, b, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            Color::from_rgba8(r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Color::from_rgba8(r, g, b, a)
        }
        _ => match s {
            "transparent" => Color::from_rgba8(0,   0,   0,   0),
            "white"       => Color::from_rgba8(255, 255, 255, 255),
            "black"       => Color::from_rgba8(0,   0,   0,   255),
            "red"         => Color::from_rgba8(255, 0,   0,   255),
            "green"       => Color::from_rgba8(0,   128, 0,   255),
            "blue"        => Color::from_rgba8(0,   0,   255, 255),
            "yellow"      => Color::from_rgba8(255, 255, 0,   255),
            "orange"      => Color::from_rgba8(255, 165, 0,   255),
            "gray" | "grey" => Color::from_rgba8(128, 128, 128, 255),
            _             => Color::from_rgba8(0,   0,   0,   255),
        },
    }
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

#[derive(Clone, Debug)]
struct FontInfo {
    size:   f32,
    bold:   bool,
    italic: bool,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self { size: 12.0, bold: false, italic: false }
    }
}

fn parse_css_font(font_str: &str) -> FontInfo {
    let mut info = FontInfo::default();
    for part in font_str.to_lowercase().split_whitespace() {
        match part {
            "bold"   => info.bold = true,
            "italic" => info.italic = true,
            s if s.ends_with("px") => {
                if let Ok(sz) = s.trim_end_matches("px").parse::<f32>() {
                    info.size = sz;
                }
            }
            _ => {}
        }
    }
    info
}

// ---------------------------------------------------------------------------
// Text width measurement via fontdue
// ---------------------------------------------------------------------------

fn measure_text_width(text: &str, font_info: &FontInfo) -> f64 {
    let font = get_font(font_info.bold, font_info.italic);
    let mut width = 0.0f32;
    for ch in text.chars() {
        let (metrics, _) = font.rasterize(ch, font_info.size);
        width += metrics.advance_width;
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
            anti_alias: true,
            ..Paint::default()
        }
    }

    fn stroke_paint(&self) -> Paint<'static> {
        Paint {
            shader: Shader::SolidColor(self.effective_stroke_color()),
            anti_alias: true,
            ..Paint::default()
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
        let paint     = self.fill_paint();
        let transform = self.transform;
        let clip      = self.current_clip.clone();
        self.pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip.as_ref());
    }

    fn stroke(&mut self) {
        let Some(path) = self.take_path() else { return };
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
        let font      = get_font(font_info.bold, font_info.italic);
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
            let (metrics, bitmap) = font.rasterize(ch, render_px);
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
