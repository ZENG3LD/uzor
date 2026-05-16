//! Vello RenderContext implementation for vello 0.6
//!
//! Wraps vello::Scene to implement the core RenderContext trait.
//! This is the ONLY vello-specific code needed - everything else comes from core.

use std::sync::{Arc, OnceLock};

use vello::kurbo::{self, Affine, BezPath, Cap, Join, Stroke, Shape};
use vello::peniko::{Blob, Brush, Fill, FontData, color::palette};
use vello::{Glyph, Scene};
use uzor::render::{
    BackdropBlur, BatchPainter, BlendMode as UzorBlendMode, CircleBatch, Effects,
    GradientPainter, ImagePainter, LineSegment, Masking, Painter,
    RenderContext as UzorRenderContext, RenderContextExt, ShapeHelpers,
    TextAlign, TextBaseline, TextBounds, TextMetrics, TextRenderer, UiEffectHelpers,
};

// Use skrifa for font metrics
use skrifa::{MetadataProvider, raw::{FileRef, FontRef}};

use uzor::fonts::{self, FontFamily};

/// Cached peniko FontData - created once, reused forever
static CACHED_FONT_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_ITALIC: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

static CACHED_FONT_PT_ROOT_UI: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_JB_MONO_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_JB_MONO_BOLD: OnceLock<FontData> = OnceLock::new();

static CACHED_FALLBACK_NERD_FONT: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_SYMBOLS2: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_COLOR_EMOJI: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_EMOJI: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_DEJAVU: OnceLock<FontData> = OnceLock::new();

fn make_font(bytes: &'static [u8]) -> FontData {
    FontData::new(Blob::new(Arc::new(bytes.to_vec())), 0)
}

/// Get cached font by family + style
pub(crate) fn get_cached_font(family: FontFamily, bold: bool, italic: bool) -> &'static FontData {
    match family {
        FontFamily::PtRootUi => CACHED_FONT_PT_ROOT_UI
            .get_or_init(|| make_font(fonts::font_bytes(family, bold, italic))),
        FontFamily::JetBrainsMono => {
            let _ = italic;
            if bold {
                CACHED_FONT_JB_MONO_BOLD
                    .get_or_init(|| make_font(fonts::font_bytes(family, true, false)))
            } else {
                CACHED_FONT_JB_MONO_REGULAR
                    .get_or_init(|| make_font(fonts::font_bytes(family, false, false)))
            }
        }
        FontFamily::Roboto => match (bold, italic) {
            (true, true) => CACHED_FONT_BOLD_ITALIC
                .get_or_init(|| make_font(fonts::font_bytes(family, true, true))),
            (true, false) => CACHED_FONT_BOLD
                .get_or_init(|| make_font(fonts::font_bytes(family, true, false))),
            (false, true) => CACHED_FONT_ITALIC
                .get_or_init(|| make_font(fonts::font_bytes(family, false, true))),
            (false, false) => CACHED_FONT_REGULAR
                .get_or_init(|| make_font(fonts::font_bytes(family, false, false))),
        },
    }
}

/// Return the static fallback font list in priority order:
/// [DejaVuSans, NotoSansSymbols2, NotoEmoji, NotoColorEmoji, SymbolsNerdFontMono].
///
/// Order rationale:
/// - DejaVuSans first — broad BMP coverage with REAL glyph data (Arrows
///   U+2190–21FF, General Punctuation, Math, Geometric Shapes, Box Drawing,
///   Letterlike, Dingbats partial). Catches the common gaps that subsetted
///   Roboto leaves (U+2192 →, U+2605 ★, U+2713 ✓, U+2630 ☰, ...).
/// - NotoSansSymbols2 — supplementary symbols (U+1xxxx and edge BMP blocks).
/// - NotoEmoji / NotoColorEmoji — emoji ranges (U+1F000+, sparkles ✨, etc.).
/// - NerdFontMono last — it's a patcher that advertises many code points but
///   ships empty / invisible glyphs for non-PUA symbols. Putting it last
///   avoids the trap where it wins charmap.map() and then renders nothing.
pub(crate) fn get_fallback_fonts() -> &'static [FontData] {
    static FALLBACK_LIST: OnceLock<Vec<FontData>> = OnceLock::new();
    FALLBACK_LIST.get_or_init(|| {
        let dv = CACHED_FALLBACK_DEJAVU
            .get_or_init(|| make_font(fonts::DEJAVU_SANS));
        let s2 = CACHED_FALLBACK_SYMBOLS2
            .get_or_init(|| make_font(fonts::NOTO_SANS_SYMBOLS2));
        let em = CACHED_FALLBACK_EMOJI
            .get_or_init(|| make_font(fonts::NOTO_EMOJI));
        let cv = CACHED_FALLBACK_COLOR_EMOJI
            .get_or_init(|| make_font(fonts::NOTO_COLOR_EMOJI));
        let nf = CACHED_FALLBACK_NERD_FONT
            .get_or_init(|| make_font(fonts::SYMBOLS_NERD_FONT_MONO));
        vec![dv.clone(), s2.clone(), em.clone(), cv.clone(), nf.clone()]
    })
}

/// Convert FontData to skrifa FontRef for metrics
pub(crate) fn to_font_ref(font: &FontData) -> Option<FontRef<'_>> {
    let file_ref = FileRef::new(font.data.as_ref()).ok()?;
    match file_ref {
        FileRef::Font(font) => Some(font),
        FileRef::Collection(collection) => collection.get(font.index).ok(),
    }
}

/// A resolved glyph: which FontData to use + the glyph id + pen position.
struct ResolvedGlyph {
    /// Index into get_fallback_fonts(), or None for primary font.
    font_index: Option<usize>,
    glyph_id: u32,
    x: f32,
    /// Vertical offset within the run (always 0.0 for horizontal text).
    y: f32,
    advance: f32,
}

/// Resolve all characters to (font, glyph_id, x, advance) with fallback.
///
/// Returns a vec of resolved glyphs.  Characters not found in any font
/// are rendered with GlyphId(0) from the primary font (tofu box).
fn resolve_glyphs_with_fallback(
    text: &str,
    primary_font_ref: &FontRef<'_>,
    font_size: f32,
) -> Vec<ResolvedGlyph> {
    let size = skrifa::instance::Size::new(font_size);
    let var_loc = skrifa::instance::LocationRef::default();
    let primary_charmap = primary_font_ref.charmap();
    let primary_metrics = primary_font_ref.glyph_metrics(size, var_loc);
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
                y: 0.0,
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
                y: 0.0,
                advance: found_adv,
            });
            pen_x += found_adv;
        }
    }

    result
}

/// Measure total advance width from resolved glyphs.
fn resolved_glyphs_total_width(glyphs: &[ResolvedGlyph]) -> f32 {
    glyphs.last().map_or(0.0, |g| g.x + g.advance)
}

/// Vello 0.6 color type alias
pub type Color = vello::peniko::color::AlphaColor<vello::peniko::color::Srgb>;

/// Parse CSS color string to vello Color.
/// Delegates to the canonical `uzor::render::parse_color` implementation.
pub fn parse_color(color: &str) -> Color {
    let (r, g, b, a) = uzor::render::parse_color(color);
    Color::from_rgba8(r, g, b, a)
}

/// Emit a sequence of `ResolvedGlyph`s to `scene`, issuing one `draw_glyphs`
/// call per contiguous run that uses the same font.
///
/// Glyphs using the primary font use `primary_font`; glyphs using a fallback
/// font use the corresponding entry in `fallbacks`.
/// Fallback index of NotoColorEmoji in the fallback chain.
///
/// [0]=NotoSans, [1]=NotoSansSymbols2, [2]=NotoEmoji, [3]=NotoColorEmoji, [4]=SymbolsNerdFontMono.
/// For COLR fonts vello requires the brush to be WHITE so it uses the font's embedded
/// palette directly; a non-white brush tints/masks the palette colors and causes tofu.
const COLOR_EMOJI_FALLBACK_IDX: usize = 3;

fn draw_resolved_glyphs(
    scene: &mut Scene,
    glyphs: &[ResolvedGlyph],
    primary_font: &FontData,
    fallbacks: &[FontData],
    font_size: f32,
    transform: Affine,
    color: Color,
) {
    if glyphs.is_empty() {
        return;
    }

    let foreground_brush = Brush::Solid(color);
    let emoji_brush = Brush::Solid(vello::peniko::color::palette::css::WHITE);
    let mut i = 0;

    while i < glyphs.len() {
        let run_font_index = glyphs[i].font_index;
        let run_start = i;

        // Find end of this contiguous run
        while i < glyphs.len() && glyphs[i].font_index == run_font_index {
            i += 1;
        }

        let run = &glyphs[run_start..i];
        let is_color_emoji = run_font_index == Some(COLOR_EMOJI_FALLBACK_IDX);
        let font = match run_font_index {
            None => primary_font,
            Some(idx) => {
                if idx < fallbacks.len() {
                    &fallbacks[idx]
                } else {
                    primary_font
                }
            }
        };

        // Use WHITE brush for NotoColorEmoji (COLR font): vello uses the brush as the
        // "application foreground" for palette index 0xFFFF.  A non-white brush tints
        // the embedded palette colors and produces washed-out / invisible glyphs.
        let brush = if is_color_emoji { &emoji_brush } else { &foreground_brush };

        scene
            .draw_glyphs(font)
            .font_size(font_size)
            .transform(transform)
            .brush(brush)
            .hint(!is_color_emoji)
            .draw(
                Fill::NonZero,
                run.iter().map(|g| Glyph {
                    id: g.glyph_id,
                    x: g.x,
                    y: g.y,
                }),
            );
    }
}

/// Active drop shadow for the vello-gpu backend.
///
/// Vello has no native blur/shadow API.  We approximate by drawing an
/// offset, semi-transparent copy of each shape before the real fill/stroke.
/// The `alpha` field encodes the shadow colour's built-in transparency.
#[derive(Clone)]
struct ShadowState {
    dx:    f64,
    dy:    f64,
    /// Shadow colour (alpha pre-extracted into the color's alpha channel).
    color: Color,
}

/// Saved context state for save/restore
#[derive(Clone)]
struct SavedState {
    transform: Affine,
    stroke_color: Color,
    stroke_width: f64,
    fill_color: Color,
    line_dash: Vec<f64>,
    line_cap: Cap,
    line_join: Join,
    global_alpha: f64,
    font_size: f64,
    font_bold: bool,
    font_italic: bool,
    font_family: FontFamily,
    text_align: TextAlign,
    text_baseline: TextBaseline,
    has_clip: bool,
}

/// Controls whether `emit_shadow_for_shape` draws the shadow as a fill or a stroke.
///
/// Stroke intent uses the same width as the upcoming real stroke so the shadow
/// produces a glow *around the outline* rather than flooding the shape interior.
enum ShapeIntent {
    Fill,
    Stroke { width: f64 },
}

/// Vello-specific render context wrapping vello::Scene
pub struct VelloGpuRenderContext<'a> {
    scene: &'a mut Scene,
    transform: Affine,

    // Styling state
    stroke_color: Color,
    stroke_width: f64,
    fill_color: Color,
    line_dash: Vec<f64>,
    line_cap: Cap,
    line_join: Join,
    global_alpha: f64,

    // Path state
    path_builder: Option<BezPath>,

    // Pending clip path (set by clip(), applied on next draw)
    pending_clip: Option<BezPath>,

    // Text state
    font_size: f64,
    font_bold: bool,
    font_italic: bool,
    font_family: FontFamily,
    text_align: TextAlign,
    text_baseline: TextBaseline,

    // State stack for save/restore
    state_stack: Vec<SavedState>,

    // Blur image for glass effects (FrostedGlass/LiquidGlass)
    blur_image: Option<vello::peniko::ImageData>,
    // Screen dimensions for blur image positioning
    screen_width: u32,
    screen_height: u32,
    // Use 3D convex glass button style (vs flat)
    use_convex_glass_buttons: bool,

    // M6-P1: Drop shadow (approximated by offset+alpha copy, no blur)
    shadow: Option<ShadowState>,
    // M6-P3: Blend mode (applied via push_layer when non-Normal)
    blend_mode: UzorBlendMode,
}

impl<'a> VelloGpuRenderContext<'a> {
    /// Create a new render context.
    ///
    /// `chart_rect_x` / `chart_rect_y` define the canvas offset that is applied
    /// to every draw call via the initial transform.
    pub fn new(
        scene: &'a mut Scene,
        chart_rect_x: f64,
        chart_rect_y: f64,
    ) -> Self {
        Self {
            scene,
            transform: Affine::translate((chart_rect_x, chart_rect_y)),
            stroke_color: palette::css::WHITE,
            stroke_width: 1.0,
            fill_color: palette::css::TRANSPARENT,
            line_dash: Vec::new(),
            line_cap: Cap::Butt,
            line_join: Join::Miter,
            global_alpha: 1.0,
            path_builder: None,
            pending_clip: None,
            font_size: 12.0,
            font_bold: false,
            font_italic: false,
            font_family: FontFamily::Roboto,
            text_align: TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            state_stack: Vec::new(),
            blur_image: None,
            screen_width: 0,
            screen_height: 0,
            use_convex_glass_buttons: false,
            shadow: None,
            blend_mode: UzorBlendMode::Normal,
        }
    }

    /// Set blur image for glass effects (FrostedGlass/LiquidGlass)
    ///
    /// When set, `draw_blur_background()` will draw a clipped portion of this image.
    pub fn set_blur_image(&mut self, image: Option<vello::peniko::ImageData>, width: u32, height: u32) {
        self.blur_image = image;
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Set whether to use 3D convex glass button style
    ///
    /// When true and blur is active, hover/active buttons will have 3D glass effect.
    /// When false, buttons will use flat blur + color overlay.
    pub fn set_use_convex_glass_buttons(&mut self, use_convex: bool) {
        self.use_convex_glass_buttons = use_convex;
    }

    fn effective_stroke_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.stroke_color.with_alpha(self.global_alpha as f32)
        } else {
            self.stroke_color
        }
    }

    fn effective_fill_color(&self) -> Color {
        if self.global_alpha < 1.0 {
            self.fill_color.with_alpha(self.global_alpha as f32)
        } else {
            self.fill_color
        }
    }

    fn make_stroke(&self) -> Stroke {
        self.make_stroke_with_width(self.stroke_width)
    }

    fn make_stroke_with_width(&self, width: f64) -> Stroke {
        let mut stroke = Stroke::new(width);
        stroke.join = self.line_join;
        stroke.start_cap = self.line_cap;
        stroke.end_cap = self.line_cap;
        if !self.line_dash.is_empty() {
            stroke.dash_pattern = self.line_dash.clone().into();
        }
        stroke
    }

    /// Parse color string to RGBA [f32; 4] for shader use.
    /// Delegates to the canonical `uzor::render::parse_color` implementation.
    fn parse_color_to_rgba(&self, color: &str) -> [f32; 4] {
        let (r, g, b, a) = uzor::render::parse_color(color);
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0]
    }

    /// Map a `UzorBlendMode` to a vello `peniko::BlendMode`.
    ///
    /// Most modes map to `peniko::Mix`; `Plus` maps to `peniko::Compose::Plus`
    /// (additive compositing rather than colour blending).
    fn blend_to_vello_blend(mode: UzorBlendMode) -> vello::peniko::BlendMode {
        use vello::peniko::{Compose, Mix};
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
            // Plus is an additive compositing operation, not a mix mode.
            UzorBlendMode::Plus       => Compose::Plus.into(),
        }
    }

    /// If a shadow is active, emit a shadow pass for `shape` before the caller
    /// draws the real shape.  Shadow is approximated as an offset, pre-alpha
    /// copy of the shape — vello has no native blur API.
    ///
    /// `intent` controls whether the shadow shape is filled or stroked:
    /// - `ShapeIntent::Fill` reproduces the fill pass (existing behaviour).
    /// - `ShapeIntent::Stroke { width }` strokes the shadow outline so a
    ///   stroke+shadow rect gets a glow *around* the outline, not a filled
    ///   interior (fix for uzor-tessera-paint handoff #2, issue #16).
    fn emit_shadow_for_shape<S: kurbo::Shape>(&mut self, shape: &S, intent: ShapeIntent) {
        let Some(ref sh) = self.shadow.clone() else { return };
        let shadow_transform = self.transform.then_translate(kurbo::Vec2::new(sh.dx, sh.dy));
        match intent {
            ShapeIntent::Fill => {
                self.scene.fill(
                    Fill::NonZero,
                    shadow_transform,
                    sh.color,
                    None,
                    shape,
                );
            }
            ShapeIntent::Stroke { width } => {
                let shadow_stroke = self.make_stroke_with_width(width);
                self.scene.stroke(
                    &shadow_stroke,
                    shadow_transform,
                    sh.color,
                    None,
                    shape,
                );
            }
        }
    }

    /// If blend mode is non-Normal, emit a `push_layer` with the correct blend
    /// mode, run `draw_fn`, then `pop_layer`.  When blend mode is Normal, call
    /// `draw_fn` directly (no extra layer overhead).
    fn with_blend_layer<F: FnOnce(&mut Scene)>(
        scene: &mut Scene,
        mode: UzorBlendMode,
        clip_shape: Option<&kurbo::Rect>,
        draw_fn: F,
    ) {
        if mode == UzorBlendMode::Normal {
            draw_fn(scene);
            return;
        }
        let blend = Self::blend_to_vello_blend(mode);
        // Use provided clip rect or an oversized fallback.
        let bounds = clip_shape
            .copied()
            .unwrap_or_else(|| kurbo::Rect::new(-1e6, -1e6, 1e6, 1e6));
        scene.push_layer(
            vello::peniko::Fill::NonZero,
            blend,
            1.0,
            Affine::IDENTITY,
            &bounds,
        );
        draw_fn(scene);
        scene.pop_layer();
    }

}

// ---------------------------------------------------------------------------
// Painter
// ---------------------------------------------------------------------------

impl<'a> Painter for VelloGpuRenderContext<'a> {
    fn save(&mut self) {
        let state = SavedState {
            transform: self.transform,
            stroke_color: self.stroke_color,
            stroke_width: self.stroke_width,
            fill_color: self.fill_color,
            line_dash: self.line_dash.clone(),
            line_cap: self.line_cap,
            line_join: self.line_join,
            global_alpha: self.global_alpha,
            font_size: self.font_size,
            font_bold: self.font_bold,
            font_italic: self.font_italic,
            font_family: self.font_family,
            text_align: self.text_align,
            text_baseline: self.text_baseline,
            has_clip: self.pending_clip.is_some(),
        };
        self.state_stack.push(state);
        self.pending_clip = None;
    }

    fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            if self.pending_clip.is_some() {
                self.scene.pop_layer();
            }
            self.transform = state.transform;
            self.stroke_color = state.stroke_color;
            self.stroke_width = state.stroke_width;
            self.fill_color = state.fill_color;
            self.line_dash = state.line_dash;
            self.line_cap = state.line_cap;
            self.line_join = state.line_join;
            self.global_alpha = state.global_alpha;
            self.font_size = state.font_size;
            self.font_bold = state.font_bold;
            self.font_italic = state.font_italic;
            self.font_family = state.font_family;
            self.text_align = state.text_align;
            self.text_baseline = state.text_baseline;
            if state.has_clip {
                self.pending_clip = Some(BezPath::new());
            } else {
                self.pending_clip = None;
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
    }

    fn set_line_dash(&mut self, pattern: &[f64]) {
        self.line_dash = pattern.to_vec();
    }

    fn set_line_cap(&mut self, cap: &str) {
        self.line_cap = match cap {
            "round"  => Cap::Round,
            "square" => Cap::Square,
            _        => Cap::Butt,
        };
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => Join::Round,
            "bevel" => Join::Bevel,
            _       => Join::Miter,
        };
    }

    fn begin_path(&mut self) {
        self.path_builder = Some(BezPath::new());
    }

    fn move_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut path) = self.path_builder {
            path.move_to(kurbo::Point::new(x, y));
        }
    }

    fn line_to(&mut self, x: f64, y: f64) {
        if let Some(ref mut path) = self.path_builder {
            path.line_to(kurbo::Point::new(x, y));
        }
    }

    fn close_path(&mut self) {
        if let Some(ref mut path) = self.path_builder {
            path.close_path();
        }
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        if let Some(ref mut path) = self.path_builder {
            path.move_to(kurbo::Point::new(x, y));
            path.line_to(kurbo::Point::new(x + w, y));
            path.line_to(kurbo::Point::new(x + w, y + h));
            path.line_to(kurbo::Point::new(x, y + h));
            path.close_path();
        }
    }

    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) {
        if let Some(ref mut path) = self.path_builder {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy),
                kurbo::Vec2::new(radius, radius),
                start_angle,
                end_angle - start_angle,
                0.0,
            );
            let path_has_elements = !path.elements().is_empty();
            let mut is_first = true;
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(p) => {
                    if is_first && path_has_elements {
                        path.line_to(p);
                    } else {
                        path.move_to(p);
                    }
                    is_first = false;
                }
                kurbo::PathEl::LineTo(p)           => { path.line_to(p); is_first = false; }
                kurbo::PathEl::QuadTo(c, p)        => { path.quad_to(c, p); is_first = false; }
                kurbo::PathEl::CurveTo(c1, c2, p)  => { path.curve_to(c1, c2, p); is_first = false; }
                kurbo::PathEl::ClosePath           => path.close_path(),
            });
        }
    }

    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, _rotation: f64, start: f64, end: f64) {
        if let Some(ref mut path) = self.path_builder {
            let arc = kurbo::Arc::new(
                kurbo::Point::new(cx, cy),
                kurbo::Vec2::new(rx, ry),
                start,
                end - start,
                0.0,
            );
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(p)           => path.move_to(p),
                kurbo::PathEl::LineTo(p)           => path.line_to(p),
                kurbo::PathEl::QuadTo(c, p)        => path.quad_to(c, p),
                kurbo::PathEl::CurveTo(c1, c2, p)  => path.curve_to(c1, c2, p),
                kurbo::PathEl::ClosePath           => path.close_path(),
            });
        }
    }

    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        if let Some(ref mut path) = self.path_builder {
            path.quad_to(kurbo::Point::new(cpx, cpy), kurbo::Point::new(x, y));
        }
    }

    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        if let Some(ref mut path) = self.path_builder {
            path.curve_to(
                kurbo::Point::new(cp1x, cp1y),
                kurbo::Point::new(cp2x, cp2y),
                kurbo::Point::new(x, y),
            );
        }
    }

    fn stroke(&mut self) {
        if let Some(path) = self.path_builder.take() {
            let width = self.stroke_width;
            self.emit_shadow_for_shape(&path, ShapeIntent::Stroke { width });
            let color = self.effective_stroke_color();
            let stroke = self.make_stroke();
            let transform = self.transform;
            let mode = self.blend_mode;
            Self::with_blend_layer(self.scene, mode, None, |scene| {
                scene.stroke(&stroke, transform, color, None, &path);
            });
        }
    }

    fn fill(&mut self) {
        if let Some(path) = self.path_builder.take() {
            self.emit_shadow_for_shape(&path, ShapeIntent::Fill);
            let color = self.effective_fill_color();
            let transform = self.transform;
            let mode = self.blend_mode;
            Self::with_blend_layer(self.scene, mode, None, |scene| {
                scene.fill(Fill::NonZero, transform, color, None, &path);
            });
        }
    }
}

// ---------------------------------------------------------------------------
// TextRenderer
// ---------------------------------------------------------------------------

impl<'a> TextRenderer for VelloGpuRenderContext<'a> {
    fn set_font(&mut self, font: &str) {
        let parsed = fonts::parse_css_font(font);
        self.font_size = parsed.size as f64;
        self.font_bold = parsed.bold;
        self.font_italic = parsed.italic;
        self.font_family = parsed.family;
    }

    fn set_text_align(&mut self, align: TextAlign) {
        self.text_align = align;
    }

    fn set_text_baseline(&mut self, baseline: TextBaseline) {
        self.text_baseline = baseline;
    }

    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        let font_size = self.font_size as f32;
        let fill_color = self.effective_fill_color();
        let primary_font = get_cached_font(self.font_family, self.font_bold, self.font_italic);
        let primary_font_ref = match to_font_ref(primary_font) {
            Some(f) => f,
            None => return,
        };
        let resolved = resolve_glyphs_with_fallback(text, &primary_font_ref, font_size);
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = primary_font_ref.metrics(size, var_loc);
        let text_width = resolved_glyphs_total_width(&resolved);
        let adjusted_x = match self.text_align {
            TextAlign::Left   => x,
            TextAlign::Center => x - text_width as f64 / 2.0,
            TextAlign::Right  => x - text_width as f64,
        };
        let adjusted_y = match self.text_baseline {
            TextBaseline::Top        => y + metrics.ascent as f64,
            TextBaseline::Middle     => y + (metrics.ascent + metrics.descent) as f64 / 2.0,
            TextBaseline::Bottom     => y + metrics.descent as f64,
            TextBaseline::Alphabetic => y,
        };
        let transform = self.transform.then_translate(kurbo::Vec2::new(adjusted_x, adjusted_y));
        let fallbacks = get_fallback_fonts();
        draw_resolved_glyphs(
            &mut self.scene,
            &resolved,
            primary_font,
            fallbacks,
            font_size,
            transform,
            fill_color,
        );
    }

    fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {
        // Text stroking not implemented in vello-gpu backend.
    }

    /// Override: apply baseline adjustment BEFORE rotation for correct text orientation.
    fn fill_text_rotated(&mut self, text: &str, x: f64, y: f64, angle: f64) {
        if angle.abs() < 0.001 {
            self.fill_text(text, x, y);
            return;
        }
        let font_size = self.font_size as f32;
        let primary_font = get_cached_font(self.font_family, self.font_bold, self.font_italic);
        let primary_font_ref = match to_font_ref(primary_font) {
            Some(f) => f,
            None => return,
        };
        let resolved = resolve_glyphs_with_fallback(text, &primary_font_ref, font_size);
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = primary_font_ref.metrics(size, var_loc);
        let text_width = resolved_glyphs_total_width(&resolved);
        let h_offset = match self.text_align {
            TextAlign::Left   => 0.0,
            TextAlign::Center => -(text_width as f64) / 2.0,
            TextAlign::Right  => -(text_width as f64),
        };
        let v_offset = match self.text_baseline {
            TextBaseline::Top        => metrics.ascent as f64,
            TextBaseline::Middle     => (metrics.ascent + metrics.descent) as f64 / 2.0,
            TextBaseline::Bottom     => metrics.descent as f64,
            TextBaseline::Alphabetic => 0.0,
        };
        let text_pos = kurbo::Point::new(x, y + v_offset);
        let transform = self.transform
            * Affine::translate(kurbo::Vec2::new(text_pos.x, text_pos.y))
            * Affine::rotate(angle);
        let fill_color = self.effective_fill_color();
        let fallbacks = get_fallback_fonts();
        let h_off_f32 = h_offset as f32;
        let shifted: Vec<ResolvedGlyph> = resolved
            .into_iter()
            .map(|g| ResolvedGlyph { x: g.x + h_off_f32, ..g })
            .collect();
        draw_resolved_glyphs(
            &mut self.scene,
            &shifted,
            primary_font,
            fallbacks,
            font_size,
            transform,
            fill_color,
        );
    }
}

// ---------------------------------------------------------------------------
// TextMetrics
// ---------------------------------------------------------------------------

impl<'a> TextMetrics for VelloGpuRenderContext<'a> {
    fn measure_text(&self, text: &str) -> f64 {
        let font_size = self.font_size as f32;
        let font = get_cached_font(self.font_family, self.font_bold, self.font_italic);
        let font_ref = match to_font_ref(font) {
            Some(f) => f,
            None => return text.len() as f64 * self.font_size * 0.6,
        };
        let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_size);
        resolved_glyphs_total_width(&glyphs) as f64
    }

    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        let parsed = fonts::parse_css_font(font);
        let font_size = parsed.size;
        let primary_font = get_cached_font(parsed.family, parsed.bold, parsed.italic);
        let Some(font_ref) = to_font_ref(primary_font) else {
            let w = text.chars().count() as f64 * font_size as f64 * 0.6;
            let ascent  = font_size as f64 * 0.9;
            let descent = font_size as f64 * 0.3;
            return TextBounds { x: 0.0, y: -ascent, w, h: ascent + descent, ascent, descent };
        };
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = font_ref.metrics(size, var_loc);
        let ascent  = metrics.ascent  as f64;
        let descent = (-metrics.descent) as f64;
        let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_size);
        let w = resolved_glyphs_total_width(&glyphs) as f64;
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
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

impl<'a> Masking for VelloGpuRenderContext<'a> {
    fn clip(&mut self) {
        if let Some(path) = self.path_builder.take() {
            self.scene.push_clip_layer(
                vello::peniko::Fill::NonZero,
                self.transform,
                &path,
            );
            self.pending_clip = Some(path);
        }
    }
    // push_mask / pop_mask / clip_rect: use default impls (save+clip / restore)
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

impl<'a> Effects for VelloGpuRenderContext<'a> {
    fn set_shadow(&mut self, dx: f64, dy: f64, _blur: f64, color: &str) {
        let shadow_color = parse_color(color);
        self.shadow = Some(ShadowState { dx, dy, color: shadow_color });
    }

    fn clear_shadow(&mut self) {
        self.shadow = None;
    }

    fn set_blend_mode(&mut self, mode: UzorBlendMode) {
        self.blend_mode = mode;
    }
}

// ---------------------------------------------------------------------------
// ShapeHelpers
// ---------------------------------------------------------------------------

impl<'a> ShapeHelpers for VelloGpuRenderContext<'a> {
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let rect = kurbo::Rect::new(x, y, x + w, y + h);
        let width = self.stroke_width;
        self.emit_shadow_for_shape(&rect, ShapeIntent::Stroke { width });
        let color = self.effective_stroke_color();
        let stroke = self.make_stroke();
        self.scene.stroke(&stroke, self.transform, color, None, &rect);
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let rect = kurbo::Rect::new(x, y, x + w, y + h);
        self.emit_shadow_for_shape(&rect, ShapeIntent::Fill);
        let color = self.effective_fill_color();
        let transform = self.transform;
        let mode = self.blend_mode;
        Self::with_blend_layer(self.scene, mode, Some(&rect), |scene| {
            scene.fill(Fill::NonZero, transform, color, None, &rect);
        });
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
// BatchPainter — optimized: single BezPath per call, one scene encode
// ---------------------------------------------------------------------------

impl<'a> BatchPainter for VelloGpuRenderContext<'a> {
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
        let w = self.stroke_width;
        self.emit_shadow_for_shape(&path, ShapeIntent::Stroke { width: w });
        let color = self.effective_stroke_color();
        let stroke = self.make_stroke();
        let transform = self.transform;
        let mode = self.blend_mode;
        Self::with_blend_layer(self.scene, mode, None, |scene| {
            scene.stroke(&stroke, transform, color, None, &path);
        });
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
        self.emit_shadow_for_shape(&path, ShapeIntent::Fill);
        let color = self.effective_fill_color();
        let transform = self.transform;
        let mode = self.blend_mode;
        Self::with_blend_layer(self.scene, mode, None, |scene| {
            scene.fill(Fill::NonZero, transform, color, None, &path);
        });
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
        let w = self.stroke_width;
        self.emit_shadow_for_shape(&path, ShapeIntent::Stroke { width: w });
        let color = self.effective_stroke_color();
        let stroke = self.make_stroke();
        let transform = self.transform;
        let mode = self.blend_mode;
        Self::with_blend_layer(self.scene, mode, None, |scene| {
            scene.stroke(&stroke, transform, color, None, &path);
        });
    }
}

// ---------------------------------------------------------------------------
// GradientPainter
// ---------------------------------------------------------------------------

impl<'a> GradientPainter for VelloGpuRenderContext<'a> {
    fn fill_linear_gradient(&mut self, stops: &[(f32, &str)], x1: f64, y1: f64, x2: f64, y2: f64) {
        if let Some(path) = self.path_builder.take() {
            use vello::peniko::{Gradient, ColorStop};
            let color_stops: Vec<ColorStop> = stops
                .iter()
                .map(|(offset, hex)| ColorStop { offset: *offset, color: parse_color(hex).into() })
                .collect();
            let gradient = Gradient::new_linear(
                kurbo::Point::new(x1, y1),
                kurbo::Point::new(x2, y2),
            ).with_stops(color_stops.as_slice());
            self.scene.fill(Fill::NonZero, self.transform, &gradient, None, &path);
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
        if let Some(path) = self.path_builder.take() {
            use vello::peniko::{Gradient, ColorStop};
            let color_stops: Vec<ColorStop> = stops
                .iter()
                .map(|(offset, hex)| ColorStop { offset: *offset, color: parse_color(hex).into() })
                .collect();
            let gradient = Gradient::new_radial(kurbo::Point::new(cx, cy), r as f32)
                .with_stops(color_stops.as_slice());
            self.scene.fill(Fill::NonZero, self.transform, &gradient, None, &path);
        }
    }
}

// ---------------------------------------------------------------------------
// UiEffectHelpers — override blur methods (vello-gpu has real blur support)
// ---------------------------------------------------------------------------

impl<'a> UiEffectHelpers for VelloGpuRenderContext<'a> {
    fn has_blur_background(&self) -> bool {
        self.blur_image.is_some()
    }

    fn use_convex_glass_buttons(&self) -> bool {
        self.blur_image.is_some() && self.use_convex_glass_buttons
    }

    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if let Some(ref blur_image) = self.blur_image {
            let clip_rect = kurbo::Rect::new(x, y, x + width, y + height);
            self.scene.push_clip_layer(vello::peniko::Fill::NonZero, Affine::IDENTITY, &clip_rect);
            let scale_x = self.screen_width as f64 / blur_image.width as f64;
            let scale_y = self.screen_height as f64 / blur_image.height as f64;
            let image_transform = Affine::scale_non_uniform(scale_x, scale_y);
            let brush = vello::peniko::ImageBrush::new(blur_image.clone());
            self.scene.draw_image(&brush, image_transform);
            self.scene.pop_layer();
        }
    }

    fn draw_glass_button_3d(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        is_active: bool,
        color: &str,
    ) {
        use vello::peniko::{Gradient, ColorStop, Mix};

        let rect = kurbo::RoundedRect::new(x, y, x + width, y + height, radius);
        let theme_color = self.parse_color_to_rgba(color);
        let has_tint = theme_color[3] > 0.01;

        if let Some(ref blur_image) = self.blur_image {
            self.scene.push_clip_layer(vello::peniko::Fill::NonZero, Affine::IDENTITY, &rect);
            let scale_x = self.screen_width as f64 / blur_image.width as f64;
            let scale_y = self.screen_height as f64 / blur_image.height as f64;
            let image_transform = Affine::scale_non_uniform(scale_x, scale_y);
            let brush = vello::peniko::ImageBrush::new(blur_image.clone());
            self.scene.draw_image(&brush, image_transform);
            self.scene.pop_layer();
        }

        let bulge = if is_active { 0.15 } else { 0.25 };
        let (base_r, base_g, base_b) = if has_tint {
            ((theme_color[0] * 255.0) as u8, (theme_color[1] * 255.0) as u8, (theme_color[2] * 255.0) as u8)
        } else {
            (200, 210, 230)
        };
        let lighten = |r: u8, g: u8, b: u8, amount: f32| -> (u8, u8, u8) {
            ((r as f32 + (255.0 - r as f32) * amount).min(255.0) as u8,
             (g as f32 + (255.0 - g as f32) * amount).min(255.0) as u8,
             (b as f32 + (255.0 - b as f32) * amount).min(255.0) as u8)
        };
        let darken = |r: u8, g: u8, b: u8, amount: f32| -> (u8, u8, u8) {
            ((r as f32 * (1.0 - amount)).max(0.0) as u8,
             (g as f32 * (1.0 - amount)).max(0.0) as u8,
             (b as f32 * (1.0 - amount)).max(0.0) as u8)
        };
        let base_alpha = if has_tint { (theme_color[3] * 255.0) as u8 } else { 180 };

        {
            let (top_r, top_g, top_b) = lighten(base_r, base_g, base_b, 0.4 + bulge);
            let (bottom_r, bottom_g, bottom_b) = darken(base_r, base_g, base_b, 0.3);
            let top_color    = Color::from_rgba8(top_r, top_g, top_b, base_alpha);
            let mid_color    = Color::from_rgba8(base_r, base_g, base_b, base_alpha);
            let bottom_color = Color::from_rgba8(bottom_r, bottom_g, bottom_b, base_alpha);
            let gradient = Gradient::new_linear(
                kurbo::Point::new(x + width / 2.0, y),
                kurbo::Point::new(x + width / 2.0, y + height),
            ).with_stops([
                ColorStop { offset: 0.0,  color: top_color.into() },
                ColorStop { offset: 0.35, color: mid_color.into() },
                ColorStop { offset: 0.65, color: mid_color.into() },
                ColorStop { offset: 1.0,  color: bottom_color.into() },
            ]);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &rect);
        }
        {
            let spec_intensity = if is_active { 0.25f32 } else { 0.45 };
            let spec_height = height * 0.4;
            let highlight_rect = kurbo::RoundedRect::new(
                x + width * 0.1, y + 1.0, x + width * 0.9, y + spec_height, radius.min(spec_height / 2.0),
            );
            let spec_gradient = Gradient::new_linear(
                kurbo::Point::new(x + width / 2.0, y),
                kurbo::Point::new(x + width / 2.0, y + spec_height),
            ).with_stops([
                ColorStop { offset: 0.0, color: Color::from_rgba8(255, 255, 255, (spec_intensity * 255.0) as u8).into() },
                ColorStop { offset: 0.5, color: Color::from_rgba8(255, 255, 255, (spec_intensity * 80.0) as u8).into() },
                ColorStop { offset: 1.0, color: Color::from_rgba8(255, 255, 255, 0).into() },
            ]);
            self.scene.push_layer(vello::peniko::Fill::NonZero, Mix::Screen, 1.0, Affine::IDENTITY, &rect);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, &spec_gradient, None, &highlight_rect);
            self.scene.pop_layer();
        }
        {
            let shadow_intensity = if is_active { 0.3f32 } else { 0.2 };
            let shadow_height = height * 0.35;
            let shadow_gradient = Gradient::new_linear(
                kurbo::Point::new(x + width / 2.0, y + height - shadow_height),
                kurbo::Point::new(x + width / 2.0, y + height),
            ).with_stops([
                ColorStop { offset: 0.0, color: Color::from_rgba8(0, 0, 0, 0).into() },
                ColorStop { offset: 0.5, color: Color::from_rgba8(0, 0, 0, (shadow_intensity * 80.0) as u8).into() },
                ColorStop { offset: 1.0, color: Color::from_rgba8(0, 0, 0, (shadow_intensity * 150.0) as u8).into() },
            ]);
            self.scene.push_layer(vello::peniko::Fill::NonZero, Mix::Multiply, 1.0, Affine::IDENTITY, &rect);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, &shadow_gradient, None, &rect);
            self.scene.pop_layer();
        }
        {
            let rim_intensity = if is_active { 0.15f32 } else { 0.3 };
            let rim_stroke = Stroke::new(1.5);
            let (rim_r, rim_g, rim_b) = lighten(base_r, base_g, base_b, 0.6);
            let rim_color = Color::from_rgba8(rim_r, rim_g, rim_b, (rim_intensity * 255.0) as u8);
            self.scene.push_layer(vello::peniko::Fill::NonZero, Mix::Screen, 0.7, Affine::IDENTITY, &rect);
            self.scene.stroke(&rim_stroke, Affine::IDENTITY, rim_color, None, &rect);
            self.scene.pop_layer();
        }
        {
            let inner_stroke = Stroke::new(1.0);
            let inner_rect = kurbo::RoundedRect::new(
                x + 1.0, y + 1.0, x + width - 1.0, y + height - 1.0, (radius - 1.0).max(0.0),
            );
            let (hl_r, hl_g, hl_b) = lighten(base_r, base_g, base_b, 0.5);
            let highlight_color = Color::from_rgba8(hl_r, hl_g, hl_b, if is_active { 30 } else { 50 });
            self.scene.stroke(&inner_stroke, Affine::IDENTITY, highlight_color, None, &inner_rect);
        }
    }
}

// ---------------------------------------------------------------------------
// BackdropBlur — vello-gpu has full GPU blur support
// ---------------------------------------------------------------------------

impl<'a> BackdropBlur for VelloGpuRenderContext<'a> {
    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        UiEffectHelpers::draw_blur_background(self, x, y, width, height);
    }

    fn has_blur_background(&self) -> bool {
        self.blur_image.is_some()
    }

    fn use_convex_glass_buttons(&self) -> bool {
        self.blur_image.is_some() && self.use_convex_glass_buttons
    }

    fn draw_glass_button_3d(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        radius: f64,
        is_active: bool,
        color: &str,
    ) {
        UiEffectHelpers::draw_glass_button_3d(self, x, y, width, height, radius, is_active, color);
    }
}

// ---------------------------------------------------------------------------
// ImagePainter
// ---------------------------------------------------------------------------

impl<'a> ImagePainter for VelloGpuRenderContext<'a> {
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

    fn draw_image_rgba(&mut self, data: &[u8], img_width: u32, img_height: u32, x: f64, y: f64, width: f64, height: f64) {
        if data.len() != (img_width * img_height * 4) as usize || img_width == 0 || img_height == 0 {
            return;
        }
        let blob = Blob::new(Arc::new(data.to_vec()));
        let image_data = vello::peniko::ImageData {
            data: blob,
            format: vello::peniko::ImageFormat::Rgba8,
            alpha_type: vello::peniko::ImageAlphaType::Alpha,
            width: img_width,
            height: img_height,
        };
        let brush = vello::peniko::ImageBrush::new(image_data);
        let scale_x = width / img_width as f64;
        let scale_y = height / img_height as f64;
        let image_transform = self.transform * Affine::translate((x, y)) * Affine::scale_non_uniform(scale_x, scale_y);
        self.scene.draw_image(&brush, image_transform);
    }
}

// ---------------------------------------------------------------------------
// RenderContext compound trait
// ---------------------------------------------------------------------------

impl<'a> UzorRenderContext for VelloGpuRenderContext<'a> {
    fn dpr(&self) -> f64 {
        1.0
    }
}

impl<'a> RenderContextExt for VelloGpuRenderContext<'a> {
    type BlurImage = vello::peniko::ImageData;

    fn set_blur_image(&mut self, image: Option<Self::BlurImage>, width: u32, height: u32) {
        self.blur_image = image;
        self.screen_width = width;
        self.screen_height = height;
    }

    fn set_use_convex_glass_buttons(&mut self, use_convex: bool) {
        self.use_convex_glass_buttons = use_convex;
    }
}
