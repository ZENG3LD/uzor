//! `VelloCpuRenderContext` — CPU-only `RenderContext` implementation.
//!
//! Uses `vello_cpu::RenderContext` for all rasterization. Zero GPU dependency.
//!
//! ## Type-system note
//!
//! `vello_cpu` 0.0.6 re-exports `kurbo` (0.13), `peniko` (0.6), and `color`
//! (0.3) from `vello_common 0.0.6`.  These differ from the `vello 0.6` versions
//! used by the GPU backend (`uzor-render-vello-gpu`), so this crate does NOT
//! depend on `uzor-backend-vello-common` and implements all state management
//! inline to avoid cross-version type conflicts.

use std::sync::{Arc, OnceLock};

use vello_cpu::kurbo::{self, Affine, BezPath, Cap, Join, Rect, Shape, Stroke};
use vello_cpu::peniko::{
    Blob, ColorStop, ColorStops, Extend, Fill, FontData, Gradient, LinearGradientPosition, Mix,
    Compose,
};
use vello_cpu::{Glyph, RenderContext as VelloCpuCtx, RenderMode, RenderSettings};

use skrifa::{
    MetadataProvider,
    raw::{FileRef, FontRef},
};

use uzor::fonts;
use uzor::render::{
    BatchPainter, BlendMode as UzorBlendMode, CircleBatch,
    Effects, GradientPainter, LineSegment, Masking, Painter,
    RenderContext as UzorRenderContext, RenderContextExt, ShapeHelpers,
    TextBounds, TextMetrics, TextRenderer, TextAlign, TextBaseline,
};

// ---------------------------------------------------------------------------
// Cached vello_cpu FontData (one per process)
// ---------------------------------------------------------------------------

static FONT_REGULAR:     OnceLock<FontData> = OnceLock::new();
static FONT_BOLD:        OnceLock<FontData> = OnceLock::new();
static FONT_ITALIC:      OnceLock<FontData> = OnceLock::new();
static FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

static FONT_PT_ROOT_UI:       OnceLock<FontData> = OnceLock::new();
static FONT_JB_MONO_REGULAR:  OnceLock<FontData> = OnceLock::new();
static FONT_JB_MONO_BOLD:     OnceLock<FontData> = OnceLock::new();

static FONT_FALLBACK_NERD_FONT:    OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_SYMBOLS2:     OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_COLOR_EMOJI:  OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_EMOJI:        OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_CJK_SC:       OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_ARABIC:       OnceLock<FontData> = OnceLock::new();
static FONT_FALLBACK_DEVANAGARI:   OnceLock<FontData> = OnceLock::new();

use uzor::fonts::FontFamily;

fn get_font(family: FontFamily, bold: bool, italic: bool) -> &'static FontData {
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

fn get_fallback_fonts() -> &'static [FontData] {
    static FALLBACK_LIST: OnceLock<Vec<FontData>> = OnceLock::new();
    FALLBACK_LIST.get_or_init(|| {
        let nf   = FONT_FALLBACK_NERD_FONT.get_or_init(|| make_font(fonts::SYMBOLS_NERD_FONT_MONO));
        let s2   = FONT_FALLBACK_SYMBOLS2.get_or_init(|| make_font(fonts::NOTO_SANS_SYMBOLS2));
        let cjk  = FONT_FALLBACK_CJK_SC.get_or_init(|| make_font(fonts::NOTO_SANS_CJK_SC));
        let ar   = FONT_FALLBACK_ARABIC.get_or_init(|| make_font(fonts::NOTO_SANS_ARABIC));
        let deva = FONT_FALLBACK_DEVANAGARI.get_or_init(|| make_font(fonts::NOTO_SANS_DEVANAGARI));
        let cv   = FONT_FALLBACK_COLOR_EMOJI.get_or_init(|| make_font(fonts::NOTO_COLOR_EMOJI));
        let em   = FONT_FALLBACK_EMOJI.get_or_init(|| make_font(fonts::NOTO_EMOJI));
        // Order: [0]=NerdFont, [1]=Symbols2, [2]=CjkSc, [3]=Arabic, [4]=Devanagari,
        //        [5]=NotoColorEmoji, [6]=NotoEmoji
        // Text script fonts (CJK/Arabic/Devanagari) placed BEFORE emoji so ordinary
        // script codepoints resolve to text outlines rather than emoji glyphs.
        vec![
            nf.clone(), s2.clone(), cjk.clone(), ar.clone(), deva.clone(),
            cv.clone(), em.clone(),
        ]
    })
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
// Resolved glyph with fallback font tracking
// ---------------------------------------------------------------------------

struct ResolvedGlyph {
    /// None = primary font; Some(i) = fallback index i.
    font_index: Option<usize>,
    glyph_id: u32,
    x: f32,
    advance: f32,
}

fn resolve_glyphs_with_fallback(
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
                if let Some(fb_ref) = to_font_ref(fb_font) {
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

fn resolved_total_width(glyphs: &[ResolvedGlyph]) -> f32 {
    glyphs.last().map_or(0.0, |g| g.x + g.advance)
}

// ---------------------------------------------------------------------------
// Color parsing (CSS → vello_cpu / peniko color)
// ---------------------------------------------------------------------------

/// vello_cpu color type (premul-capable, sRGB)
type Color = vello_cpu::color::AlphaColor<vello_cpu::color::Srgb>;

fn parse_color(s: &str) -> Color {
    let (r, g, b, a) = uzor::render::parse_color(s);
    Color::from_rgba8(r, g, b, a)
}

// ---------------------------------------------------------------------------
// CSS font parsing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct FontInfo {
    size:   f64,
    bold:   bool,
    italic: bool,
    family: FontFamily,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self { size: 12.0, bold: false, italic: false, family: FontFamily::Roboto }
    }
}

fn parse_css_font(font_str: &str) -> FontInfo {
    let parsed = fonts::parse_css_font(font_str);
    FontInfo {
        size:   parsed.size as f64,
        bold:   parsed.bold,
        italic: parsed.italic,
        family: parsed.family,
    }
}

// ---------------------------------------------------------------------------
// Text metrics via skrifa
// ---------------------------------------------------------------------------

fn measure_text_width(text: &str, font_info: &FontInfo) -> f64 {
    let font = get_font(font_info.family, font_info.bold, font_info.italic);
    let Some(font_ref) = to_font_ref(font) else {
        return text.len() as f64 * font_info.size * 0.6;
    };
    let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_info.size as f32);
    resolved_total_width(&glyphs) as f64
}

// ---------------------------------------------------------------------------
// Shadow state (M6-P1)
// ---------------------------------------------------------------------------

/// Active drop shadow for vello-cpu.  Approximated as an offset, alpha copy.
#[derive(Clone)]
struct ShadowState {
    dx:    f64,
    dy:    f64,
    color: Color,
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
    /// Blend mode at this save level.
    blend_mode:   UzorBlendMode,
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
    // vello_cpu 0.0.9 split per-render resources (image-registry + glyph caches)
    // out of the context so the rasteriser can own them across frames.
    resources:    vello_cpu::Resources,
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

    // M6-P1: Drop shadow (approximated as offset copy, blur not native)
    shadow:       Option<ShadowState>,
    // M6-P3: Blend mode
    blend_mode:   UzorBlendMode,
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
            resources:    vello_cpu::Resources::new(),
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
            shadow:       None,
            blend_mode:   UzorBlendMode::Normal,
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
            // vello_cpu 0.0.9: render_to_buffer now takes &mut Resources first.
            ctx.render_to_buffer(&mut self.resources, buffer, width, height, RenderMode::OptimizeSpeed);
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
        let blend = Self::blend_to_vello_cpu(self.blend_mode);
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_blend_mode(blend);
            ctx.set_paint(color);
        }
    }

    /// Apply stroke paint (stroke color × global alpha) to the vello_cpu context.
    fn apply_stroke_paint(&mut self) {
        let color = self.effective_stroke_color();
        let blend = Self::blend_to_vello_cpu(self.blend_mode);
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_blend_mode(blend);
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
            blend_mode:   self.blend_mode,
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
        self.blend_mode   = s.blend_mode;
        Some(s)
    }

    /// Convert a `UzorBlendMode` to a `vello_cpu::peniko::BlendMode`.
    fn blend_to_vello_cpu(mode: UzorBlendMode) -> vello_cpu::peniko::BlendMode {
        match mode {
            UzorBlendMode::Normal     => Mix::Normal.into(),
            UzorBlendMode::Multiply   => Mix::Multiply.into(),
            UzorBlendMode::Screen     => Mix::Screen.into(),
            UzorBlendMode::Overlay    => Mix::Overlay.into(),
            UzorBlendMode::Darken     => Mix::Darken.into(),
            UzorBlendMode::Lighten    => Mix::Lighten.into(),
            UzorBlendMode::ColorDodge => Mix::ColorDodge.into(),
            UzorBlendMode::ColorBurn  => Mix::ColorBurn.into(),
            UzorBlendMode::HardLight  => Mix::HardLight.into(),
            UzorBlendMode::SoftLight  => Mix::SoftLight.into(),
            UzorBlendMode::Difference => Mix::Difference.into(),
            UzorBlendMode::Exclusion  => Mix::Exclusion.into(),
            UzorBlendMode::Plus       => Compose::Plus.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

impl Painter for VelloCpuRenderContext {
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

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_color(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
    }

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

    fn stroke(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        let stroke = self.current_stroke();
        // M6-P1: shadow pass
        if let Some(ref sh) = self.shadow.clone() {
            let shadow_transform = transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
            if let Some(ref mut ctx) = self.render_ctx {
                ctx.set_transform(shadow_transform);
                ctx.set_blend_mode(vello_cpu::peniko::BlendMode::default());
                ctx.set_paint(sh.color);
                ctx.set_stroke(stroke.clone());
                ctx.stroke_path(&path);
            }
        }
        self.apply_stroke_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_stroke(stroke);
            ctx.stroke_path(&path);
        }
    }

    fn fill(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        // M6-P1: shadow pass
        if let Some(ref sh) = self.shadow.clone() {
            let shadow_transform = transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
            if let Some(ref mut ctx) = self.render_ctx {
                ctx.set_transform(shadow_transform);
                ctx.set_fill_rule(Fill::NonZero);
                ctx.set_blend_mode(vello_cpu::peniko::BlendMode::default());
                ctx.set_paint(sh.color);
                ctx.fill_path(&path);
            }
        }
        self.apply_fill_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_fill_rule(Fill::NonZero);
            ctx.fill_path(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl TextRenderer for VelloCpuRenderContext {
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
        let primary_font = get_font(font_info.family, font_info.bold, font_info.italic);
        let font_size    = font_info.size as f32;

        let text_width = measure_text_width(text, &font_info);
        let x_off = match self.text_align {
            TextAlign::Center => -text_width / 2.0,
            TextAlign::Right  => -text_width,
            _                 => 0.0,
        };
        let y_off = match self.text_baseline {
            TextBaseline::Top    => font_info.size * 0.8,
            TextBaseline::Middle => font_info.size * 0.35,
            TextBaseline::Bottom => 0.0,
            _                    => font_info.size * 0.35,
        };

        let Some(primary_ref) = to_font_ref(primary_font) else { return };
        let resolved = resolve_glyphs_with_fallback(text, &primary_ref, font_size);
        let fallbacks = get_fallback_fonts();

        let text_transform = Affine::translate((x + x_off, y + y_off));
        let combined = self.transform * text_transform;

        // Fallback index 5 = NotoColorEmoji (COLR font) in chain:
        // [0]=NerdFont, [1]=Symbols2, [2]=CjkSc, [3]=Arabic, [4]=Devanagari,
        // [5]=NotoColorEmoji, [6]=NotoEmoji.
        // vello_cpu requires WHITE paint for COLR runs so the embedded palette is used directly.
        const COLOR_EMOJI_FALLBACK_IDX: usize = 5;
        let fill_color = self.effective_fill_color();
        let white = Color::from_rgba8(255, 255, 255, 255);

        // vello_cpu 0.0.9: glyph_run now takes &mut Resources first. Split-borrow
        // self.render_ctx + self.resources so both can be referenced inside the loop.
        let resources = &mut self.resources;
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(combined);

            let mut i = 0;
            while i < resolved.len() {
                let run_font_index = resolved[i].font_index;
                let run_start = i;
                while i < resolved.len() && resolved[i].font_index == run_font_index {
                    i += 1;
                }
                let run = &resolved[run_start..i];
                let is_color_emoji = run_font_index == Some(COLOR_EMOJI_FALLBACK_IDX);
                let font = match run_font_index {
                    None => primary_font,
                    Some(idx) if idx < fallbacks.len() => &fallbacks[idx],
                    _ => primary_font,
                };
                ctx.set_paint(if is_color_emoji { white } else { fill_color });
                let glyphs = run.iter().map(|g| Glyph { id: g.glyph_id, x: g.x, y: 0.0 });
                ctx.glyph_run(resources, font)
                    .font_size(font_size)
                    .hint(false)
                    .normalized_coords(&[])
                    .fill_glyphs(glyphs);
            }

            ctx.set_transform(self.transform);
        }
    }

    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {
        // vello_cpu glyph_run() draws filled glyphs only; stroke_glyphs
        // requires a different code path.  Deliberate no-op.
    }
}

// ---------------------------------------------------------------------------
// TextMetrics
// ---------------------------------------------------------------------------

impl TextMetrics for VelloCpuRenderContext {
    fn measure_text(&self, text: &str) -> f64 {
        measure_text_width(text, &self.font_info)
    }

    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        let info = parse_css_font(font);
        let font_size = info.size as f32;
        let primary_font = get_font(info.family, info.bold, info.italic);
        let Some(font_ref) = to_font_ref(primary_font) else {
            let w = text.chars().count() as f64 * info.size * 0.6;
            let ascent  = info.size * 0.9;
            let descent = info.size * 0.3;
            return TextBounds { x: 0.0, y: -ascent, w, h: ascent + descent, ascent, descent };
        };
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = font_ref.metrics(size, var_loc);
        let ascent  = metrics.ascent  as f64;
        let descent = (-metrics.descent) as f64;
        let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_size);
        let w = resolved_total_width(&glyphs) as f64;
        TextBounds {
            x: 0.0,
            y: -ascent,
            w,
            h: ascent + descent,
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

    fn text_to_path(&self, text: &str, font: &str) -> String {
        uzor::shaper::text_to_path(text, font)
    }
}

// ---------------------------------------------------------------------------
// Masking — clip() overridden; clip_rect/push_mask/pop_mask use defaults
// ---------------------------------------------------------------------------

impl Masking for VelloCpuRenderContext {
    fn clip(&mut self) {
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        self.clip_active = true;
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.push_clip_path(&path);
        }
    }

    /// Even-odd fill rule override: clips using `Fill::EvenOdd` so two-subpath
    /// paths (outer rect CW + inner shape CCW) produce a ring-shaped clip.
    fn push_clip_svg_path_even_odd(&mut self, d: &str) {
        uzor::render::emit_svg_path(self, d);
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;
        self.clip_active = true;
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_fill_rule(Fill::EvenOdd);
            ctx.push_clip_path(&path);
            ctx.set_fill_rule(Fill::NonZero);
        }
        self.save();
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

impl Effects for VelloCpuRenderContext {
    fn set_shadow(&mut self, dx: f64, dy: f64, _blur: f64, color: &str) {
        // Vello CPU has a DropShadow filter (pub(crate) only).
        // Approximated as an offset, pre-alpha copy — matches vello-gpu.
        let shadow_color = parse_color(color);
        self.shadow = Some(ShadowState { dx, dy, color: shadow_color });
    }

    fn clear_shadow(&mut self) {
        self.shadow = None;
    }

    fn set_blend_mode(&mut self, mode: UzorBlendMode) {
        self.blend_mode = mode;
        let blend = Self::blend_to_vello_cpu(mode);
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_blend_mode(blend);
        }
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers — fill_rect and stroke_rect overridden; rounded uses defaults
// ---------------------------------------------------------------------------

impl ShapeHelpers for VelloCpuRenderContext {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let r = Rect::new(x, y, x + w, y + h);
        let transform = self.transform;
        // M6-P1: shadow pass
        if let Some(ref sh) = self.shadow.clone() {
            let shadow_transform = transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
            if let Some(ref mut ctx) = self.render_ctx {
                ctx.set_transform(shadow_transform);
                ctx.set_blend_mode(vello_cpu::peniko::BlendMode::default());
                ctx.set_paint(sh.color);
                ctx.fill_rect(&r);
            }
        }
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
        let tl = tl.clamp(0.0, max_r);
        let tr = tr.clamp(0.0, max_r);
        let br = br.clamp(0.0, max_r);
        let bl = bl.clamp(0.0, max_r);

        self.begin_path();
        self.move_to(x + tl, y);
        self.line_to(x + w - tr, y);
        self.arc(x + w - tr, y + tr, tr, -std::f64::consts::FRAC_PI_2, 0.0);
        self.line_to(x + w, y + h - br);
        self.arc(x + w - br, y + h - br, br, 0.0, std::f64::consts::FRAC_PI_2);
        self.line_to(x + bl, y + h);
        self.arc(x + bl, y + h - bl, bl, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        self.line_to(x, y + tl);
        self.arc(x + tl, y + tl, tl, std::f64::consts::PI, std::f64::consts::PI * 1.5);
        self.close_path();
    }
}

// ---------------------------------------------------------------------------
// BatchPainter — optimized: single merged BezPath per call
// ---------------------------------------------------------------------------

impl BatchPainter for VelloCpuRenderContext {
    fn draw_line_batch(&mut self, lines: &[LineSegment], color: &str, width: f64) {
        if lines.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut path = BezPath::new();
        for l in lines {
            path.move_to(kurbo::Point::new(l.x1, l.y1));
            path.line_to(kurbo::Point::new(l.x2, l.y2));
        }
        let transform = self.transform;
        let stroke = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_stroke(stroke);
            ctx.stroke_path(&path);
        }
    }

    fn draw_circle_batch(&mut self, circles: &[CircleBatch], color: &str) {
        if circles.is_empty() {
            return;
        }
        self.set_fill_color(color);
        let mut path = BezPath::new();
        for c in circles {
            let circle = kurbo::Circle::new(kurbo::Point::new(c.cx, c.cy), c.r);
            path.extend(circle.path_elements(0.1));
        }
        let transform = self.transform;
        self.apply_fill_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_fill_rule(Fill::NonZero);
            ctx.fill_path(&path);
        }
    }

    fn stroke_polyline(&mut self, pts: &[(f64, f64)], color: &str, width: f64) {
        if pts.is_empty() {
            return;
        }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut path = BezPath::new();
        path.move_to(kurbo::Point::new(pts[0].0, pts[0].1));
        for &(x, y) in &pts[1..] {
            path.line_to(kurbo::Point::new(x, y));
        }
        let transform = self.transform;
        let stroke = self.current_stroke();
        self.apply_stroke_paint();
        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_stroke(stroke);
            ctx.stroke_path(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// GradientPainter
// ---------------------------------------------------------------------------

impl GradientPainter for VelloCpuRenderContext {
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
        let Some(path) = self.path.clone() else { return };
        let transform = self.transform;

        let color_stops: ColorStops = ColorStops::from(
            stops
                .iter()
                .map(|(offset, hex)| ColorStop::from((*offset, parse_color(hex))))
                .collect::<Vec<ColorStop>>()
                .as_slice(),
        );

        let gradient = Gradient::new_radial(kurbo::Point::new(cx, cy), r as f32)
            .with_stops(color_stops.as_slice())
            .with_extend(Extend::Pad);

        if let Some(ref mut ctx) = self.render_ctx {
            ctx.set_transform(transform);
            ctx.set_fill_rule(Fill::NonZero);
            ctx.set_paint(gradient);
            ctx.fill_path(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// UiEffectHelpers — all defaults (no blur support on vello-cpu)
// ---------------------------------------------------------------------------

impl uzor::render::UiEffectHelpers for VelloCpuRenderContext {}

// ---------------------------------------------------------------------------
// RenderContext (dpr only)
// ---------------------------------------------------------------------------

impl UzorRenderContext for VelloCpuRenderContext {
    fn dpr(&self) -> f64 {
        self.dpr
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
