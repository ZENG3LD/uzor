//! Vello RenderContext implementation for vello 0.6
//!
//! Wraps vello::Scene to implement the core RenderContext trait.
//! This is the ONLY vello-specific code needed - everything else comes from core.

use std::sync::{Arc, OnceLock};

use vello::kurbo::{self, Affine, BezPath, Cap, Join, Stroke, Shape};
use vello::peniko::{Blob, Brush, Fill, FontData, color::palette};
use vello::{Glyph, Scene};
use uzor::render::{RenderContext as UzorRenderContext, RenderContextExt, TextAlign, TextBaseline};

// Use skrifa for font metrics
use skrifa::{MetadataProvider, raw::{FileRef, FontRef}};

/// Embedded Roboto fonts for text rendering
static ROBOTO_REGULAR: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8] = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

/// Embedded Unicode fallback fonts
static NOTO_SYMBOLS2: &[u8] = include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf");
static NOTO_EMOJI: &[u8] = include_bytes!("../fonts/NotoEmoji-Regular.ttf");

/// Cached peniko FontData - created once, reused forever
static CACHED_FONT_REGULAR: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_ITALIC: OnceLock<FontData> = OnceLock::new();
static CACHED_FONT_BOLD_ITALIC: OnceLock<FontData> = OnceLock::new();

static CACHED_FALLBACK_SYMBOLS2: OnceLock<FontData> = OnceLock::new();
static CACHED_FALLBACK_EMOJI: OnceLock<FontData> = OnceLock::new();

/// Get cached font by style
pub(crate) fn get_cached_font(bold: bool, italic: bool) -> &'static FontData {
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

/// Return the static fallback font list: [NotoSansSymbols2, NotoEmoji].
pub(crate) fn get_fallback_fonts() -> &'static [FontData] {
    static FALLBACK_LIST: OnceLock<Vec<FontData>> = OnceLock::new();
    FALLBACK_LIST.get_or_init(|| {
        let s2 = CACHED_FALLBACK_SYMBOLS2.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(NOTO_SYMBOLS2.to_vec())), 0)
        });
        let em = CACHED_FALLBACK_EMOJI.get_or_init(|| {
            FontData::new(Blob::new(Arc::new(NOTO_EMOJI.to_vec())), 0)
        });
        vec![s2.clone(), em.clone()]
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

    let brush = Brush::Solid(color);
    let mut i = 0;

    while i < glyphs.len() {
        let run_font_index = glyphs[i].font_index;
        let run_start = i;

        // Find end of this contiguous run
        while i < glyphs.len() && glyphs[i].font_index == run_font_index {
            i += 1;
        }

        let run = &glyphs[run_start..i];
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

        scene
            .draw_glyphs(font)
            .font_size(font_size)
            .transform(transform)
            .brush(&brush)
            .hint(true)
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
    text_align: TextAlign,
    text_baseline: TextBaseline,
    has_clip: bool,
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
            text_align: TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            state_stack: Vec::new(),
            blur_image: None,
            screen_width: 0,
            screen_height: 0,
            use_convex_glass_buttons: false,
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
        let mut stroke = Stroke::new(self.stroke_width);
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

}

impl<'a> UzorRenderContext for VelloGpuRenderContext<'a> {
    fn dpr(&self) -> f64 {
        1.0
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
            "round" => Cap::Round,
            "square" => Cap::Square,
            _ => Cap::Butt,
        };
    }

    fn set_line_join(&mut self, join: &str) {
        self.line_join = match join {
            "round" => Join::Round,
            "bevel" => Join::Bevel,
            _ => Join::Miter,
        };
    }

    fn set_fill_color(&mut self, color: &str) {
        self.fill_color = parse_color(color);
    }

    fn set_global_alpha(&mut self, alpha: f64) {
        self.global_alpha = alpha.clamp(0.0, 1.0);
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
            // Check if path already has elements (we're continuing a path)
            let path_has_elements = !path.elements().is_empty();
            let mut is_first = true;
            arc.to_path(0.1).into_iter().for_each(|el| match el {
                kurbo::PathEl::MoveTo(p) => {
                    // Skip MoveTo if continuing an existing path - use LineTo instead
                    // This prevents arc from breaking the path into subpaths
                    if is_first && path_has_elements {
                        path.line_to(p);
                    } else {
                        path.move_to(p);
                    }
                    is_first = false;
                }
                kurbo::PathEl::LineTo(p) => { path.line_to(p); is_first = false; }
                kurbo::PathEl::QuadTo(c, p) => { path.quad_to(c, p); is_first = false; }
                kurbo::PathEl::CurveTo(c1, c2, p) => { path.curve_to(c1, c2, p); is_first = false; }
                kurbo::PathEl::ClosePath => path.close_path(),
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
                kurbo::PathEl::MoveTo(p) => path.move_to(p),
                kurbo::PathEl::LineTo(p) => path.line_to(p),
                kurbo::PathEl::QuadTo(c, p) => path.quad_to(c, p),
                kurbo::PathEl::CurveTo(c1, c2, p) => path.curve_to(c1, c2, p),
                kurbo::PathEl::ClosePath => path.close_path(),
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
            let color = self.effective_stroke_color();
            let stroke = self.make_stroke();
            self.scene.stroke(&stroke, self.transform, color, None, &path);
        }
    }

    fn fill(&mut self) {
        if let Some(path) = self.path_builder.take() {
            let color = self.effective_fill_color();
            self.scene.fill(Fill::NonZero, self.transform, color, None, &path);
        }
    }

    fn fill_linear_gradient(&mut self, stops: &[(f32, &str)], x1: f64, y1: f64, x2: f64, y2: f64) {
        if let Some(path) = self.path_builder.take() {
            use vello::peniko::{Gradient, ColorStop};

            let start = kurbo::Point::new(x1, y1);
            let end = kurbo::Point::new(x2, y2);

            let color_stops: Vec<ColorStop> = stops
                .iter()
                .map(|(offset, hex)| {
                    let color = parse_color(hex);
                    ColorStop { offset: *offset, color: color.into() }
                })
                .collect();

            let gradient = Gradient::new_linear(start, end).with_stops(color_stops.as_slice());
            self.scene.fill(Fill::NonZero, self.transform, &gradient, None, &path);
        }
    }

    fn clip(&mut self) {
        // Take the current path and use it as a clip
        if let Some(path) = self.path_builder.take() {
            // Push a clip layer using the path
            self.scene.push_clip_layer(
                vello::peniko::Fill::NonZero,
                self.transform,
                &path,
            );
            // Store that we have an active clip (for restore to pop_layer)
            self.pending_clip = Some(path);
        }
    }

    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let rect = kurbo::Rect::new(x, y, x + w, y + h);
        let color = self.effective_stroke_color();
        let stroke = self.make_stroke();
        self.scene.stroke(&stroke, self.transform, color, None, &rect);
    }

    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let rect = kurbo::Rect::new(x, y, x + w, y + h);
        let color = self.effective_fill_color();
        self.scene.fill(Fill::NonZero, self.transform, color, None, &rect);
    }

    fn set_font(&mut self, font: &str) {
        // Reset style flags before parsing
        self.font_bold = false;
        self.font_italic = false;

        let font_lower = font.to_lowercase();
        for part in font_lower.split_whitespace() {
            if part.ends_with("px") {
                if let Ok(size) = part.trim_end_matches("px").parse::<f64>() {
                    self.font_size = size;
                }
            } else if part == "bold" {
                self.font_bold = true;
            } else if part == "italic" {
                self.font_italic = true;
            }
        }
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

        // Get cached font based on style
        let primary_font = get_cached_font(self.font_bold, self.font_italic);
        let primary_font_ref = match to_font_ref(primary_font) {
            Some(f) => f,
            None => return,
        };

        // Resolve all glyphs with fallback
        let resolved = resolve_glyphs_with_fallback(text, &primary_font_ref, font_size);

        // Get baseline metrics from primary font for vertical alignment
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = primary_font_ref.metrics(size, var_loc);

        let text_width = resolved_glyphs_total_width(&resolved);

        // Adjust x for text alignment
        let adjusted_x = match self.text_align {
            TextAlign::Left => x,
            TextAlign::Center => x - text_width as f64 / 2.0,
            TextAlign::Right => x - text_width as f64,
        };

        // Adjust y for text baseline
        let adjusted_y = match self.text_baseline {
            TextBaseline::Top => y + metrics.ascent as f64,
            TextBaseline::Middle => y + (metrics.ascent + metrics.descent) as f64 / 2.0,
            TextBaseline::Bottom => y + metrics.descent as f64,
            TextBaseline::Alphabetic => y,
        };

        let transform = self.transform.then_translate(kurbo::Vec2::new(adjusted_x, adjusted_y));
        let fallbacks = get_fallback_fonts();

        // Draw glyphs grouped by font to allow fallback fonts
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
        // Text stroking not commonly needed
    }

    fn measure_text(&self, text: &str) -> f64 {
        let font_size = self.font_size as f32;

        // Get cached font based on style
        let font = get_cached_font(self.font_bold, self.font_italic);
        let font_ref = match to_font_ref(font) {
            Some(f) => f,
            None => {
                // Fallback: approximate
                return text.len() as f64 * self.font_size * 0.6;
            }
        };

        let glyphs = resolve_glyphs_with_fallback(text, &font_ref, font_size);
        resolved_glyphs_total_width(&glyphs) as f64
    }

    fn save(&mut self) {
        // Save current state to stack, including whether there's an active clip
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
            text_align: self.text_align,
            text_baseline: self.text_baseline,
            has_clip: self.pending_clip.is_some(),
        };
        self.state_stack.push(state);
        // Reset pending_clip for the new save level
        self.pending_clip = None;
    }

    fn restore(&mut self) {
        // Restore state from stack
        if let Some(state) = self.state_stack.pop() {
            // Pop clip from CURRENT save level (if any)
            if self.pending_clip.is_some() {
                self.scene.pop_layer();
            }

            // Restore all state
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
            self.text_align = state.text_align;
            self.text_baseline = state.text_baseline;

            // Restore clip tracking from outer level
            if state.has_clip {
                // Use a dummy BezPath to mark that outer clip is still active
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

    /// Override fill_text_rotated to correctly handle baseline adjustment before rotation.
    ///
    /// The default implementation in RenderContext trait does:
    ///   1. translate(x, y)
    ///   2. rotate(angle)
    ///   3. fill_text(0, 0) - which applies baseline adjustment in rotated space
    ///
    /// This causes text to drift when rotated because the baseline vertical offset
    /// gets applied in the rotated coordinate system.
    ///
    /// Our fix: apply baseline adjustment BEFORE rotation, so it's always vertical.
    fn fill_text_rotated(&mut self, text: &str, x: f64, y: f64, angle: f64) {
        if angle.abs() < 0.001 {
            self.fill_text(text, x, y);
            return;
        }

        let font_size = self.font_size as f32;

        // Get cached font based on style
        let primary_font = get_cached_font(self.font_bold, self.font_italic);
        let primary_font_ref = match to_font_ref(primary_font) {
            Some(f) => f,
            None => return,
        };

        // Resolve all glyphs with fallback
        let resolved = resolve_glyphs_with_fallback(text, &primary_font_ref, font_size);

        // Get baseline metrics from primary font for vertical alignment
        let size = skrifa::instance::Size::new(font_size);
        let var_loc = skrifa::instance::LocationRef::default();
        let metrics = primary_font_ref.metrics(size, var_loc);

        let text_width = resolved_glyphs_total_width(&resolved);

        // Horizontal offset (applied in rotated space - this is correct)
        let h_offset = match self.text_align {
            TextAlign::Left => 0.0,
            TextAlign::Center => -(text_width as f64) / 2.0,
            TextAlign::Right => -(text_width as f64),
        };

        // Vertical baseline offset — applied to anchor point BEFORE rotation
        let v_offset = match self.text_baseline {
            TextBaseline::Top => metrics.ascent as f64,
            TextBaseline::Middle => (metrics.ascent + metrics.descent) as f64 / 2.0,
            TextBaseline::Bottom => metrics.descent as f64,
            TextBaseline::Alphabetic => 0.0,
        };

        // Build transform:
        // 1. Start with base transform (includes chart offset)
        // 2. Translate to text position
        // 3. Rotate around that position
        let text_pos = kurbo::Point::new(x, y + v_offset);
        let transform = self.transform
            * Affine::translate(kurbo::Vec2::new(text_pos.x, text_pos.y))
            * Affine::rotate(angle);

        let fill_color = self.effective_fill_color();
        let fallbacks = get_fallback_fonts();

        // Shift all resolved glyph x positions by h_offset to account for alignment
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

    // =========================================================================
    // Blur Background (FrostedGlass/LiquidGlass effects)
    // =========================================================================

    fn draw_blur_background(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if let Some(ref blur_image) = self.blur_image {
            // Create clip rect
            let clip_rect = kurbo::Rect::new(x, y, x + width, y + height);

            // Push clip layer (transform applies to clip shape, not content)
            self.scene.push_clip_layer(vello::peniko::Fill::NonZero, Affine::IDENTITY, &clip_rect);

            // Draw blur image at full screen size (clipped to rect)
            // The image transform positions it at origin with screen dimensions
            let scale_x = self.screen_width as f64 / blur_image.width as f64;
            let scale_y = self.screen_height as f64 / blur_image.height as f64;
            let image_transform = Affine::scale_non_uniform(scale_x, scale_y);

            // Create ImageBrush from ImageData and draw
            let brush = vello::peniko::ImageBrush::new(blur_image.clone());
            self.scene.draw_image(&brush, image_transform);

            // Pop clip layer
            self.scene.pop_layer();
        }
    }

    fn draw_image_rgba(&mut self, data: &[u8], img_width: u32, img_height: u32, x: f64, y: f64, width: f64, height: f64) {
        if data.len() != (img_width * img_height * 4) as usize || img_width == 0 || img_height == 0 {
            return;
        }

        // Create vello ImageData from raw RGBA bytes
        let blob = Blob::new(Arc::new(data.to_vec()));
        let image_data = vello::peniko::ImageData {
            data: blob,
            format: vello::peniko::ImageFormat::Rgba8,
            alpha_type: vello::peniko::ImageAlphaType::Alpha,
            width: img_width,
            height: img_height,
        };
        let brush = vello::peniko::ImageBrush::new(image_data);

        // Transform: apply context transform, then translate to position, then scale from source size to target size
        let scale_x = width / img_width as f64;
        let scale_y = height / img_height as f64;
        let image_transform = self.transform * Affine::translate((x, y)) * Affine::scale_non_uniform(scale_x, scale_y);

        self.scene.draw_image(&brush, image_transform);
    }

    fn has_blur_background(&self) -> bool {
        self.blur_image.is_some()
    }

    fn use_convex_glass_buttons(&self) -> bool {
        self.blur_image.is_some() && self.use_convex_glass_buttons
    }

    /// Draw 3D convex glass button effect
    ///
    /// Creates iOS-style raised glass button using layered effects:
    /// 1. Blur backdrop (from blur_image)
    /// 2. Vertical gradient overlay (convex bulge simulation)
    /// 3. Top specular highlight stripe
    /// 4. Bottom inner shadow
    /// 5. Edge fresnel rim glow
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

        // Create rounded rect shape for clipping
        let rect = kurbo::RoundedRect::new(x, y, x + width, y + height, radius);

        // Parse theme color for tinting
        let theme_color = self.parse_color_to_rgba(color);
        let has_tint = theme_color[3] > 0.01;

        // =====================================================================
        // Layer 1: Blur backdrop (same as draw_blur_background but with clip)
        // =====================================================================
        if let Some(ref blur_image) = self.blur_image {
            self.scene.push_clip_layer(vello::peniko::Fill::NonZero, Affine::IDENTITY, &rect);

            let scale_x = self.screen_width as f64 / blur_image.width as f64;
            let scale_y = self.screen_height as f64 / blur_image.height as f64;
            let image_transform = Affine::scale_non_uniform(scale_x, scale_y);

            let brush = vello::peniko::ImageBrush::new(blur_image.clone());
            self.scene.draw_image(&brush, image_transform);

            self.scene.pop_layer();
        }

        // Bulge intensity: less when active (pressed down)
        let bulge = if is_active { 0.15 } else { 0.25 };

        // Base color for the button (from active/hover color)
        let (base_r, base_g, base_b) = if has_tint {
            (
                (theme_color[0] * 255.0) as u8,
                (theme_color[1] * 255.0) as u8,
                (theme_color[2] * 255.0) as u8,
            )
        } else {
            (200, 210, 230) // Default glass blue-ish
        };

        // Helper to lighten a color
        let lighten = |r: u8, g: u8, b: u8, amount: f32| -> (u8, u8, u8) {
            (
                (r as f32 + (255.0 - r as f32) * amount).min(255.0) as u8,
                (g as f32 + (255.0 - g as f32) * amount).min(255.0) as u8,
                (b as f32 + (255.0 - b as f32) * amount).min(255.0) as u8,
            )
        };

        // Helper to darken a color
        let darken = |r: u8, g: u8, b: u8, amount: f32| -> (u8, u8, u8) {
            (
                (r as f32 * (1.0 - amount)).max(0.0) as u8,
                (g as f32 * (1.0 - amount)).max(0.0) as u8,
                (b as f32 * (1.0 - amount)).max(0.0) as u8,
            )
        };

        // Alpha from theme color (active_bg_opacity = 0.4, hover_bg_opacity = 0.5)
        // This makes the button semi-transparent so blur background shows through
        let base_alpha = if has_tint {
            (theme_color[3] * 255.0) as u8
        } else {
            180 // Default semi-transparent
        };

        // =====================================================================
        // Layer 2: Main button fill with 3D convex gradient (COLORED!)
        // Top = lighter version of active color, Bottom = darker version
        // Uses alpha from theme to blend with blur background
        // =====================================================================
        {
            let (top_r, top_g, top_b) = lighten(base_r, base_g, base_b, 0.4 + bulge);
            let (bottom_r, bottom_g, bottom_b) = darken(base_r, base_g, base_b, 0.3);

            let top_color = Color::from_rgba8(top_r, top_g, top_b, base_alpha);
            let mid_color = Color::from_rgba8(base_r, base_g, base_b, base_alpha);
            let bottom_color = Color::from_rgba8(bottom_r, bottom_g, bottom_b, base_alpha);

            let start = kurbo::Point::new(x + width / 2.0, y);
            let end = kurbo::Point::new(x + width / 2.0, y + height);

            let gradient = Gradient::new_linear(start, end)
                .with_stops([
                    ColorStop { offset: 0.0, color: top_color.into() },
                    ColorStop { offset: 0.35, color: mid_color.into() },
                    ColorStop { offset: 0.65, color: mid_color.into() },
                    ColorStop { offset: 1.0, color: bottom_color.into() },
                ]);

            self.scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &rect);
        }

        // =====================================================================
        // Layer 3: Top specular highlight (white, screen blend)
        // =====================================================================
        {
            let spec_intensity = if is_active { 0.25 } else { 0.45 };
            let spec_height = height * 0.4;

            let highlight_rect = kurbo::RoundedRect::new(
                x + width * 0.1,
                y + 1.0,
                x + width * 0.9,
                y + spec_height,
                radius.min(spec_height / 2.0),
            );

            let start = kurbo::Point::new(x + width / 2.0, y);
            let end = kurbo::Point::new(x + width / 2.0, y + spec_height);

            let spec_gradient = Gradient::new_linear(start, end)
                .with_stops([
                    ColorStop { offset: 0.0, color: Color::from_rgba8(255, 255, 255, (spec_intensity * 255.0) as u8).into() },
                    ColorStop { offset: 0.5, color: Color::from_rgba8(255, 255, 255, (spec_intensity * 80.0) as u8).into() },
                    ColorStop { offset: 1.0, color: Color::from_rgba8(255, 255, 255, 0).into() },
                ]);

            self.scene.push_layer(vello::peniko::Fill::NonZero, Mix::Screen, 1.0, Affine::IDENTITY, &rect);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, &spec_gradient, None, &highlight_rect);
            self.scene.pop_layer();
        }

        // =====================================================================
        // Layer 4: Bottom inner shadow (darkens the base color)
        // =====================================================================
        {
            let shadow_intensity = if is_active { 0.3 } else { 0.2 };
            let shadow_height = height * 0.35;

            let start = kurbo::Point::new(x + width / 2.0, y + height - shadow_height);
            let end = kurbo::Point::new(x + width / 2.0, y + height);

            let shadow_gradient = Gradient::new_linear(start, end)
                .with_stops([
                    ColorStop { offset: 0.0, color: Color::from_rgba8(0, 0, 0, 0).into() },
                    ColorStop { offset: 0.5, color: Color::from_rgba8(0, 0, 0, (shadow_intensity * 80.0) as u8).into() },
                    ColorStop { offset: 1.0, color: Color::from_rgba8(0, 0, 0, (shadow_intensity * 150.0) as u8).into() },
                ]);

            self.scene.push_layer(vello::peniko::Fill::NonZero, Mix::Multiply, 1.0, Affine::IDENTITY, &rect);
            self.scene.fill(Fill::NonZero, Affine::IDENTITY, &shadow_gradient, None, &rect);
            self.scene.pop_layer();
        }

        // =====================================================================
        // Layer 5: Fresnel rim glow (lighter version of base color)
        // =====================================================================
        {
            let rim_intensity = if is_active { 0.15 } else { 0.3 };
            let rim_stroke = Stroke::new(1.5);

            let (rim_r, rim_g, rim_b) = lighten(base_r, base_g, base_b, 0.6);
            let rim_color = Color::from_rgba8(rim_r, rim_g, rim_b, (rim_intensity * 255.0) as u8);

            self.scene.push_layer(vello::peniko::Fill::NonZero, Mix::Screen, 0.7, Affine::IDENTITY, &rect);
            self.scene.stroke(&rim_stroke, Affine::IDENTITY, rim_color, None, &rect);
            self.scene.pop_layer();
        }

        // =====================================================================
        // Layer 6: Very subtle inner highlight stroke at top
        // =====================================================================
        {
            let inner_stroke = Stroke::new(1.0);
            let inner_rect = kurbo::RoundedRect::new(
                x + 1.0, y + 1.0,
                x + width - 1.0, y + height - 1.0,
                (radius - 1.0).max(0.0),
            );
            let (hl_r, hl_g, hl_b) = lighten(base_r, base_g, base_b, 0.5);
            let highlight_color = Color::from_rgba8(hl_r, hl_g, hl_b, if is_active { 30 } else { 50 });
            self.scene.stroke(&inner_stroke, Affine::IDENTITY, highlight_color, None, &inner_rect);
        }
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
