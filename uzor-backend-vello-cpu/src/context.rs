//! `VelloCpuRenderContext` — CPU-only `RenderContext` implementation.
//!
//! Uses `vello_cpu::RenderContext` for all rasterization. Zero GPU dependency.
//!
//! ## Type-system note
//!
//! `vello_cpu` 0.0.6 re-exports `kurbo` (0.13), `peniko` (0.6), and `color`
//! (0.3) from `vello_common 0.0.6`.  These differ from the `vello 0.6` versions
//! used by the GPU backend (`uzor-backend-vello-gpu`), so this crate does NOT
//! depend on `uzor-backend-vello-common` and implements all state management
//! inline to avoid cross-version type conflicts.

use std::sync::{Arc, OnceLock};

use vello_cpu::kurbo::{self, Affine, BezPath, Cap, Join, Rect, Shape, Stroke};
use vello_cpu::peniko::{
    Blob, ColorStop, ColorStops, Extend, Fill, FontData, Gradient, LinearGradientPosition,
};
use vello_cpu::{Glyph, RenderContext as VelloCpuCtx, RenderMode, RenderSettings};

use skrifa::{
    MetadataProvider,
    raw::{FileRef, FontRef},
};

use uzor::render::{RenderContext as UzorRenderContext, RenderContextExt, TextAlign, TextBaseline};

// ---------------------------------------------------------------------------
// Embedded fonts
// ---------------------------------------------------------------------------

static ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

// ---------------------------------------------------------------------------
// Cached vello_cpu FontData (one per process)
// ---------------------------------------------------------------------------

static FONT_REGULAR:     OnceLock<FontData> = OnceLock::new();
static FONT_BOLD:        OnceLock<FontData> = OnceLock::new();
static FONT_ITALIC:      OnceLock<FontData> = OnceLock::new();
static FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

fn get_font(bold: bool, italic: bool) -> &'static FontData {
    match (bold, italic) {
        (true,  true)  => FONT_BOLD_ITALIC.get_or_init(|| make_font(ROBOTO_BOLD_ITALIC)),
        (true,  false) => FONT_BOLD.get_or_init(|| make_font(ROBOTO_BOLD)),
        (false, true)  => FONT_ITALIC.get_or_init(|| make_font(ROBOTO_ITALIC)),
        (false, false) => FONT_REGULAR.get_or_init(|| make_font(ROBOTO_REGULAR)),
    }
}

fn make_font(bytes: &'static [u8]) -> FontData {
    FontData::new(Blob::new(Arc::new(bytes) as Arc<dyn AsRef<[u8]> + Send + Sync>), 0)
}

fn to_font_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(f)   => Some(f),
        FileRef::Collection(col) => col.get(font.index).ok(),
    }
}

// ---------------------------------------------------------------------------
// Color parsing (CSS → vello_cpu / peniko color)
// ---------------------------------------------------------------------------

/// vello_cpu color type (premul-capable, sRGB)
type Color = vello_cpu::color::AlphaColor<vello_cpu::color::Srgb>;

fn parse_color(s: &str) -> Color {
    let s = s.trim();

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

// ---------------------------------------------------------------------------
// CSS font parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct FontInfo {
    size:   f64,
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
                if let Ok(sz) = s.trim_end_matches("px").parse::<f64>() {
                    info.size = sz;
                }
            }
            _ => {}
        }
    }
    info
}

// ---------------------------------------------------------------------------
// Text metrics via skrifa
// ---------------------------------------------------------------------------

fn measure_text_width(text: &str, font_info: &FontInfo) -> f64 {
    let font = get_font(font_info.bold, font_info.italic);
    let Some(font_ref) = to_font_ref(font) else {
        return text.len() as f64 * font_info.size * 0.6;
    };
    let font_size = skrifa::instance::Size::new(font_info.size as f32);
    let var_loc   = skrifa::instance::LocationRef::default();
    let charmap   = font_ref.charmap();
    let metrics   = font_ref.glyph_metrics(font_size, var_loc);
    let w: f32 = text.chars().map(|ch| {
        let gid = charmap.map(ch).unwrap_or_default();
        metrics.advance_width(gid).unwrap_or_default()
    }).sum();
    w as f64
}

// ---------------------------------------------------------------------------
// Save/restore state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SavedState {
    transform:    Affine,
    stroke_color: Color,
    stroke_width: f64,
    fill_color:   Color,
    line_cap:     Cap,
    line_join:    Join,
    global_alpha: f64,
    font_info:    FontInfo,
    text_align:   TextAlign,
    text_baseline: TextBaseline,
    /// Whether `clip()` was called at this save level (so we pop it on restore).
    has_clip:     bool,
}

// ---------------------------------------------------------------------------
// VelloCpuRenderContext
// ---------------------------------------------------------------------------

/// CPU-only rendering context backed by `vello_cpu`.
///
/// Uses the sparse-strips rasterization algorithm with optional SIMD
/// acceleration.  No GPU context, no wgpu dependency.
///
/// ## Frame lifecycle
///
/// ```rust,ignore
/// let mut ctx = VelloCpuRenderContext::new(1.0); // dpr
/// ctx.begin_frame(800, 600);
/// ctx.set_fill_color("#1e1e1e");
/// ctx.fill_rect(0.0, 0.0, 800.0, 600.0);
/// ctx.render_to_pixmap_rgba8(&mut rgba8_buffer, 800, 600);
/// ```
pub struct VelloCpuRenderContext {
    // vello_cpu renderer — re-created on size change, reset each frame
    render_ctx:   Option<VelloCpuCtx>,
    width:        u32,
    height:       u32,
    dpr:          f64,

    // Drawing state
    transform:    Affine,
    stroke_color: Color,
    stroke_width: f64,
    fill_color:   Color,
    line_cap:     Cap,
    line_join:    Join,
    global_alpha: f64,
    font_info:    FontInfo,
    text_align:   TextAlign,
    text_baseline: TextBaseline,

    // Current Canvas2D-style path
    path:         Option<BezPath>,

    // Whether a clip path is active at the current innermost save level
    clip_active:  bool,

    // Save/restore stack
    state_stack:  Vec<SavedState>,
}

impl VelloCpuRenderContext {
    /// Create a new context.
    ///
    /// The underlying `vello_cpu::RenderContext` is lazily allocated on the
    /// first call to [`begin_frame`](Self::begin_frame).
    ///
    /// `dpr` — device pixel ratio.
    pub fn new(dpr: f64) -> Self {
        Self {
            render_ctx:   None,
            width:        0,
            height:       0,
            dpr,
            transform:    Affine::IDENTITY,
            stroke_color: Color::from_rgba8(255, 255, 255, 255),
            stroke_width: 1.0,
            fill_color:   Color::from_rgba8(0, 0, 0, 0),
            line_cap:     Cap::Butt,
            line_join:    Join::Miter,
            global_alpha: 1.0,
            font_info:    FontInfo::default(),
            text_align:   TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            path:         None,
            clip_active:  false,
            state_stack:  Vec::new(),
        }
    }

    /// Begin a new frame.
    ///
    /// Re-creates the `vello_cpu::RenderContext` only when `width` or `height`
    /// changes; otherwise calls `reset()` to clear draw commands without
    /// reallocating internal strip buffers.
    ///
    /// Also resets per-frame drawing state (transform, clip, save stack).
    pub fn begin_frame(&mut self, width: u32, height: u32) {
        let w16 = width.min(u16::MAX as u32) as u16;
        let h16 = height.min(u16::MAX as u32) as u16;

        let needs_new = self.render_ctx.is_none()
            || self.width  != width
            || self.height != height;

        if needs_new {
            let settings = RenderSettings {
                level:       vello_cpu::Level::new(),
                num_threads: 0,
                render_mode: RenderMode::OptimizeSpeed,
            };
            self.render_ctx = Some(VelloCpuCtx::new_with(w16, h16, settings));
            self.width  = width;
            self.height = height;
        } else if let Some(ref mut ctx) = self.render_ctx {
            ctx.reset();
        }

        // Reset per-frame state
        self.transform   = Affine::IDENTITY;
        self.clip_active = false;
        self.state_stack.clear();
        self.path        = None;
    }

    /// Render the completed frame into a premultiplied RGBA8 pixel buffer.
    ///
    /// `buffer` must have exactly `width * height * 4` bytes.
    /// The pixel format is `[R, G, B, A]` premultiplied — suitable for most
    /// image APIs and for manual conversion to softbuffer's `0x00RRGGBB` u32.
    ///
    /// Calls `flush()` internally (required when the `multithreading` feature
    /// is active), then rasterizes.
    pub fn render_to_pixmap_rgba8(&mut self, buffer: &mut [u8], width: u16, height: u16) {
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.flush();
            ctx.render_to_buffer(buffer, width, height, RenderMode::OptimizeSpeed);
        }
    }

    /// Render directly into a softbuffer-compatible `u32` buffer.
    ///
    /// Renders to a temporary RGBA8 buffer and converts to `0x00RRGGBB`
    /// format expected by `softbuffer`.
    ///
    /// `out` must have at least `width * height` elements.
    pub fn render_to_softbuffer(&mut self, out: &mut [u32]) {
        let w = self.width as u16;
        let h = self.height as u16;
        let pixel_count = self.width as usize * self.height as usize;
        if out.len() < pixel_count {
            return;
        }
        let mut rgba8 = vec![0u8; pixel_count * 4];
        self.render_to_pixmap_rgba8(&mut rgba8, w, h);
        // Convert PremulRGBA8 [R,G,B,A] → softbuffer 0x00RRGGBB
        for (src, dst) in rgba8.chunks_exact(4).zip(out.iter_mut()) {
            *dst = ((src[0] as u32) << 16)
                 | ((src[1] as u32) <<  8)
                 |  (src[2] as u32);
        }
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Build a `kurbo::Stroke` from the current stroke state.
    fn current_stroke(&self) -> Stroke {
        Stroke {
            width:       self.stroke_width,
            join:        self.line_join,
            miter_limit: 4.0,
            start_cap:   self.line_cap,
            end_cap:     self.line_cap,
            dash_pattern: Default::default(),
            dash_offset:  0.0,
        }
    }

    /// Apply fill paint (fill color × global alpha) to the vello_cpu context.
    fn apply_fill_paint(&mut self) {
        let color = self.effective_fill_color();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_paint(color);
        }
    }

    /// Apply stroke paint (stroke color × global alpha) to the vello_cpu context.
    fn apply_stroke_paint(&mut self) {
        let color = self.effective_stroke_color();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_paint(color);
        }
    }

    fn effective_fill_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.fill_color.with_alpha(self.global_alpha as f32)
        } else {
            self.fill_color
        }
    }

    fn effective_stroke_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.stroke_color.with_alpha(self.global_alpha as f32)
        } else {
            self.stroke_color
        }
    }

    fn push_save_state(&mut self, has_clip: bool) {
        self.state_stack.push(SavedState {
            transform:    self.transform,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            fill_color:   self.fill_color,
            line_cap:     self.line_cap,
            line_join:    self.line_join,
            global_alpha: self.global_alpha,
            font_info:    self.font_info.clone(),
            text_align:   self.text_align,
            text_baseline: self.text_baseline,
            has_clip,
        });
    }

    fn pop_save_state(&mut self) -> Option<SavedState> {
        let s = self.state_stack.pop()?;
        self.transform    = s.transform;
        self.stroke_color = s.stroke_color;
        self.stroke_width = s.stroke_width;
        self.fill_color   = s.fill_color;
        self.line_cap     = s.line_cap;
        self.line_join    = s.line_join;
        self.global_alpha = s.global_alpha;
        self.font_info    = s.font_info.clone();
        self.text_align   = s.text_align;
        self.text_baseline = s.text_baseline;
        Some(s)
    }
}

// ---------------------------------------------------------------------------
// RenderContext trait implementation
// ---------------------------------------------------------------------------

impl UzorRenderContext for VelloCpuRenderContext {
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
        self.stroke_color = parse_color(color);
    }

    fn set_stroke_width(&mut self, width: f64) {
        self.stroke_width = width;
        let stroke = self.current_stroke();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_stroke(stroke);
        }
    }

    fn set_line_dash(&mut self, _pattern: &[f64]) {
        // vello_cpu's Stroke has a dash_pattern field (smallvec-based), but
        // constructing a dashed stroke requires the Stroke builder pattern.
        // Dash support is a known limitation of this backend; calls are silently
        // accepted to keep the interface compatible.
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap {
            "round"  => Cap::Round,
            "square" => Cap::Square,
            _        => Cap::Butt,
        };
        let stroke = self.current_stroke();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_stroke(stroke);
        }
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => Join::Round,
            "bevel" => Join::Bevel,
            _       => Join::Miter,
        };
        let stroke = self.current_stroke();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_stroke(stroke);
        }
    }

    // -----------------------------------------------------------------------
    // Fill style
    // -----------------------------------------------------------------------

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_color(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

    // -----------------------------------------------------------------------
    // Path operations
    // -----------------------------------------------------------------------

    fn begin_path(&mut self) {
        self.path = Some(BezPath::new());
    }

    fn move_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.move_to(kurbo::Point::new(x, y));
        }
    }

    fn line_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.line_to(kurbo::Point::new(x, y));
        }
    }

    fn close_path(&mut self) {
        if let Some(ref mut p) = self.path {
            p.close_path();
        }
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if let Some(ref mut p) = self.path {
            p.move_to(kurbo::Point::new(x, y));
            p.line_to(kurbo::Point::new(x + w, y));
            p.line_to(kurbo::Point::new(x + w, y + h));
            p.line_to(kurbo::Point::new(x, y + h));
            p.close_path();
        }
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        if let Some(ref mut p) = self.path {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy),
                kurbo::Vec2::new(radius, radius),
                start_angle,
                end_angle - start_angle,
                0.0,
            );
            let path_has_elements = !p.elements().is_empty();
            let mut is_first = true;
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(pt) => {
                    if is_first && path_has_elements {
                        p.line_to(pt);
                    } else {
                        p.move_to(pt);
                    }
                    is_first = false;
                }
                kurbo::PathEl::LineTo(pt) => { p.line_to(pt); is_first = false; }
                kurbo::PathEl::QuadTo(c, pt) => { p.quad_to(c, pt); is_first = false; }
                kurbo::PathEl::CurveTo(c1, c2, pt) => {
                    p.curve_to(c1, c2, pt);
                    is_first = false;
                }
                kurbo::PathEl::ClosePath => p.close_path(),
            });
        }
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
        if let Some(ref mut p) = self.path {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy),
                kurbo::Vec2::new(rx, ry),
                start,
                end - start,
                0.0,
            );
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(pt)          => p.move_to(pt),
                kurbo::PathEl::LineTo(pt)          => p.line_to(pt),
                kurbo::PathEl::QuadTo(c, pt)       => p.quad_to(c, pt),
                kurbo::PathEl::CurveTo(c1, c2, pt) => p.curve_to(c1, c2, pt),
                kurbo::PathEl::ClosePath           => p.close_path(),
            });
        }
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        if let Some(ref mut p) = self.path {
            p.quad_to(kurbo::Point::new(cpx, cpy), kurbo::Point::new(x, y));
        }
    }

    fn bezier_curve_to(
        &mut self,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x: f64,
        y: f64,
    ) {
        if let Some(ref mut p) = self.path {
            p.curve_to(
                kurbo::Point::new(cp1x, cp1y),
                kurbo::Point::new(cp2x, cp2y),
                kurbo::Point::new(x, y),
            );
        }
    }

    // -----------------------------------------------------------------------
    // Stroke / fill the current path
    // -----------------------------------------------------------------------

    fn fill(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        self.apply_fill_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_fill_rule(Fill::NonZero);
            ctx.fill_path(&path);
        }
    }

    fn fill_linear_gradient(
        &mut self,
        stops: &[(f32, &str)],
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;

        let color_stops: ColorStops = ColorStops::from(
            stops
                .iter()
                .map(|(offset, hex)| ColorStop::from((*offset, parse_color(hex))))
                .collect::<Vec<ColorStop>>()
                .as_slice(),
        );

        let gradient = Gradient {
            kind: LinearGradientPosition {
                start: kurbo::Point::new(x1, y1),
                end: kurbo::Point::new(x2, y2),
            }
            .into(),
            stops: color_stops,
            extend: Extend::Pad,
            ..Default::default()
        };

        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_fill_rule(Fill::NonZero);
            ctx.set_paint(gradient);
            ctx.fill_path(&path);
        }
    }

    fn stroke(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        let stroke = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_stroke(stroke);
            ctx.stroke_path(&path);
        }
    }

    fn clip(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        self.clip_active = true;
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.push_clip_path(&path);
        }
    }

    // -----------------------------------------------------------------------
    // Shape helpers
    // -----------------------------------------------------------------------

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let r = Rect::new(x, y, x + w, y + h);
        let transform = self.transform;
        self.apply_fill_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.fill_rect(&r);
        }
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let r = Rect::new(x, y, x + w, y + h);
        let transform = self.transform;
        let stroke = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_stroke(stroke);
            ctx.stroke_rect(&r);
        }
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
        let font_size = font_info.size as f32;

        // Alignment offsets
        let text_width = measure_text_width(text, &font_info);
        let x_off = match self.text_align {
            TextAlign::Center => -text_width / 2.0,
            TextAlign::Right  => -text_width,
            _                 => 0.0,
        };
        // Approximate ascent for baseline alignment
        let y_off = match self.text_baseline {
            TextBaseline::Top    => font_info.size * 0.8,
            TextBaseline::Middle => font_info.size * 0.35,
            TextBaseline::Bottom => 0.0,
            _                    => font_info.size * 0.35,
        };

        // Build glyph list via skrifa
        let Some(font_ref) = to_font_ref(font) else { return };
        let sz      = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let charmap = font_ref.charmap();
        let metrics = font_ref.glyph_metrics(sz, var_loc);

        let mut glyphs: Vec<Glyph> = Vec::with_capacity(text.len());
        let mut pen_x = 0.0f32;
        for ch in text.chars() {
            let gid = charmap.map(ch).unwrap_or_default();
            glyphs.push(Glyph { id: gid.to_u32(), x: pen_x, y: 0.0 });
            pen_x += metrics.advance_width(gid).unwrap_or_default();
        }

        // Compose final transform: context transform + text position offset
        let text_transform = Affine::translate((x + x_off, y + y_off));
        let combined = self.transform * text_transform;

        self.apply_fill_paint();

        if let Some(ref mut ctx) = self.render_ctx {
            // Set the combined transform before starting the glyph run.
            // vello_cpu's glyph_run picks up the current transform at call time.
            ctx.set_transform(combined);

            ctx.glyph_run(font)
                .font_size(font_size)
                .hint(false)
                .normalized_coords(&[])
                .fill_glyphs(glyphs.into_iter());

            // Restore the context transform (set_transform is absolute)
            ctx.set_transform(self.transform);
        }
    }

    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {
        // vello_cpu glyph_run() draws filled glyphs only (fill_glyphs /
        // stroke_glyphs exist but stroke_glyphs requires a different path).
        // For now this is a deliberate no-op; callers can overlay stroke on
        // top of fill_text if needed.
    }

    fn measure_text(&self, text: &str) -> f64 {
        measure_text_width(text, &self.font_info)
    }

    // -----------------------------------------------------------------------
    // Transform operations
    // -----------------------------------------------------------------------

    fn save(&mut self) {
        let has_clip = self.clip_active;
        self.push_save_state(has_clip);
        self.clip_active = false;
    }

    fn restore(&mut self) {
        if let Some(saved) = self.pop_save_state() {
            // Pop clip path if one was pushed during this save level
            if self.clip_active {
                if let Some(ref mut ctx) = self.render_ctx {
                    ctx.pop_clip_path();
                }
            }
            self.clip_active = saved.has_clip;
            // Sync vello_cpu's transform to the restored value
            let transform = self.transform;
            if let Some(ref mut ctx) = self.render_ctx {
                ctx.set_transform(transform);
            }
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_translate((x, y).into());
    }

    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.then_rotate(angle);
    }

    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_scale_non_uniform(x, y);
    }
}

// ---------------------------------------------------------------------------
// RenderContextExt — blur/glass effects (no-op for CPU backend)
// ---------------------------------------------------------------------------

impl RenderContextExt for VelloCpuRenderContext {
    /// CPU backend carries no blur image state.
    type BlurImage = ();

    fn set_blur_image(&mut self, _image: Option<()>, _width: u32, _height: u32) {
        // CPU backend does not support blur backgrounds.
    }

    fn set_use_convex_glass_buttons(&mut self, _use_convex: bool) {
        // CPU backend does not support convex glass buttons.
    }
}
