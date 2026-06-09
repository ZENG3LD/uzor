//! `UrxRenderContext` — implements the full `uzor::RenderContext` supertrait
//! composition, emits `urx_core::Scene::DrawCommand` events.
//!
//! State management mirrors the Canvas2D semantics used by every other
//! uzor backend (vello-cpu / vello-hybrid / tiny-skia): `save()/restore()`
//! push/pop a frame of {transform, fill_color, stroke_color, stroke_width,
//! line cap/join, global_alpha, font, text align/baseline, blend, clip
//! depth}. `begin_path()` resets the current path; `fill()/stroke()` emit
//! a `FillPath`/`StrokePath` command carrying the buffered path + current
//! style + current transform. Backend reads the `Scene` once and
//! rasterises in painter's order — no statefulness leaks across `Scene`.

use kurbo::{
    Affine as KAffine, BezPath, Cap, Join, Point as KPoint, Rect as KRect, Shape, Vec2,
};
use peniko::{
    Brush as PenikoBrush, Color, ColorStop, ColorStops, Extend, Gradient,
    LinearGradientPosition, RadialGradientPosition,
};

use uzor_urx_core::scene::{
    DrawCommand, FillRule, LineCap as UrxLineCap, LineJoin as UrxLineJoin, Scene,
    Stroke as UrxStroke,
};

use uzor::fonts::{self, FontFamily};
use uzor::render::{
    BatchPainter, BlendMode as UzorBlendMode, CircleBatch, Effects, GlyphMetric, GradientPainter,
    LineSegment, Masking, Painter, RenderContext as UzorRenderContext, RenderContextExt,
    ShapeHelpers, TextAlign, TextBaseline, TextBounds, TextMetrics, TextRenderer,
    UiEffectHelpers,
};

// ── Font info (Canvas2D-style font shorthand) ───────────────────────────────

#[derive(Clone, Debug)]
struct FontInfo {
    size:   f32,
    bold:   bool,
    italic: bool,
    family: FontFamily,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self { size: 12.0, bold: false, italic: false, family: FontFamily::Roboto }
    }
}

fn parse_css_font(s: &str) -> FontInfo {
    let p = fonts::parse_css_font(s);
    FontInfo { size: p.size, bold: p.bold, italic: p.italic, family: p.family }
}

// ── Color parsing ───────────────────────────────────────────────────────────

fn parse_color(s: &str) -> Color {
    let (r, g, b, a) = uzor::render::parse_color(s);
    Color::from_rgba8(r, g, b, a)
}

fn apply_alpha(c: Color, alpha: f64) -> Color {
    if alpha >= 1.0 { c } else { c.multiply_alpha(alpha as f32) }
}

// ── Saved state frame ──────────────────────────────────────────────────────

#[derive(Clone)]
struct SavedState {
    transform:     KAffine,
    fill_color:    Color,
    stroke_color:  Color,
    stroke_width:  f64,
    line_cap:      Cap,
    line_join:     Join,
    global_alpha:  f64,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    blend_mode:    UzorBlendMode,
    /// How many `PushClipRect`/`PushClipRoundedRect` ops were emitted at
    /// this save level. `restore()` emits matching `PopClip`s.
    clip_pushes:   u32,
    /// Active fill gradient (next `fill()` consumes it once, then it's
    /// cleared back to the solid colour). Mirrors the Canvas2D
    /// "createLinearGradient" pattern.
    fill_gradient: Option<Gradient>,
}

#[derive(Clone)]
struct ShadowState {
    dx:    f64,
    dy:    f64,
    color: Color,
}

// ── UrxRenderContext ────────────────────────────────────────────────────────

/// `uzor::RenderContext` impl that buffers draw events into an
/// `urx_core::Scene`. Backend-agnostic — choose the backend at submit time.
///
/// ## Frame lifecycle
///
/// ```rust,ignore
/// let mut ctx = UrxRenderContext::new(dpr);
/// ctx.begin_frame(width, height);
/// // consumer paints via the standard RenderContext trait surface
/// let scene = ctx.take_scene();
/// // hand to any URX backend
/// ```
pub struct UrxRenderContext {
    scene: Scene,
    width:  u32,
    height: u32,
    dpr:    f64,

    // Drawing state
    transform:     KAffine,
    fill_color:    Color,
    stroke_color:  Color,
    stroke_width:  f64,
    line_cap:      Cap,
    line_join:     Join,
    global_alpha:  f64,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    blend_mode:    UzorBlendMode,

    /// Current Canvas2D-style path buffer.
    path: BezPath,

    /// Active fill gradient — consumed by the next `fill()` then cleared.
    fill_gradient: Option<Gradient>,

    /// Drop shadow (optional). Emitted as a translated pre-pass before the
    /// main draw on `fill_rect` / `fill` / `fill_text` ops.
    shadow: Option<ShadowState>,

    /// Clip stack depth at each save level — `clip()` increments the top,
    /// `restore()` pops matching `PopClip` ops to balance.
    clip_pushes: u32,

    state_stack: Vec<SavedState>,
}

impl UrxRenderContext {
    pub fn new(dpr: f64) -> Self {
        Self {
            scene:         Scene::new(),
            width:         0,
            height:        0,
            dpr,
            transform:     KAffine::IDENTITY,
            fill_color:    Color::from_rgba8(0, 0, 0, 255),
            stroke_color:  Color::from_rgba8(0, 0, 0, 255),
            stroke_width:  1.0,
            line_cap:      Cap::Butt,
            line_join:     Join::Miter,
            global_alpha:  1.0,
            font_info:     FontInfo::default(),
            text_align:    TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            blend_mode:    UzorBlendMode::Normal,
            path:          BezPath::new(),
            fill_gradient: None,
            shadow:        None,
            clip_pushes:   0,
            state_stack:   Vec::new(),
        }
    }

    /// Defensive guard for any path-op that kurbo requires be preceded
    /// by a `MoveTo` (line_to, quad_to, curve_to, close_path, arc
    /// segment append). Canvas2D semantics tolerate calling these on
    /// an empty / just-closed path — they implicitly start a fresh
    /// subpath at the given fallback point (or at the op's own
    /// target). kurbo panics with "BezPath must begin with MoveTo",
    /// so we open the subpath ourselves.
    ///
    /// Cheap when the subpath is already open (one `last()` peek +
    /// pattern match) — only walks the elements when the path is
    /// non-empty AND the last element is a `ClosePath`.
    fn ensure_subpath_open(&mut self, fallback: KPoint) {
        let needs_move = match self.path.elements().last() {
            None => true,
            Some(kurbo::PathEl::ClosePath) => true,
            _ => false,
        };
        if needs_move {
            self.path.move_to(fallback);
        }
    }

    /// Reset for a new frame. Discards any buffered draws + state.
    pub fn begin_frame(&mut self, width: u32, height: u32) {
        self.scene.reset();
        self.width  = width;
        self.height = height;
        self.transform = KAffine::IDENTITY;
        self.path.truncate(0);
        self.fill_gradient = None;
        self.shadow = None;
        self.clip_pushes = 0;
        self.state_stack.clear();
    }

    /// Take ownership of the buffered `Scene` and reset the inner one. The
    /// returned scene is what the URX backend rasterises this frame.
    pub fn take_scene(&mut self) -> Scene {
        std::mem::replace(&mut self.scene, Scene::new())
    }

    /// Read-only access (for tests / inspection).
    pub fn scene(&self) -> &Scene { &self.scene }

    pub fn size(&self) -> (u32, u32) { (self.width, self.height) }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn effective_fill_brush(&mut self) -> PenikoBrush {
        if let Some(g) = self.fill_gradient.take() {
            // Gradients carry their stops' alpha already; respect global_alpha
            // by scaling each stop. Cheap clone — gradients are small.
            if self.global_alpha < 1.0 {
                let stops_vec: Vec<ColorStop> = g
                    .stops
                    .iter()
                    .map(|s| ColorStop {
                        offset: s.offset,
                        color:  s.color.multiply_alpha(self.global_alpha as f32),
                    })
                    .collect();
                let mut g2 = g.clone();
                g2.stops = ColorStops::from(stops_vec.as_slice());
                PenikoBrush::Gradient(g2)
            } else {
                PenikoBrush::Gradient(g)
            }
        } else {
            PenikoBrush::Solid(apply_alpha(self.fill_color, self.global_alpha))
        }
    }

    fn effective_stroke_brush(&self) -> PenikoBrush {
        PenikoBrush::Solid(apply_alpha(self.stroke_color, self.global_alpha))
    }

    fn current_stroke(&self) -> UrxStroke {
        UrxStroke {
            width:       self.stroke_width as f32,
            miter_limit: 4.0,
            cap:         to_urx_cap(self.line_cap),
            join:        to_urx_join(self.line_join),
        }
    }

    /// Emit a fill rect (with optional rounded radii). Honours active
    /// shadow + fill gradient.
    fn emit_fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radii: Option<[f32; 4]>) {
        let rect = KRect::new(x, y, x + w, y + h);
        if let Some(sh) = self.shadow.clone() {
            self.scene.push(DrawCommand::FillRect {
                rect,
                radii,
                brush: PenikoBrush::Solid(apply_alpha(sh.color, self.global_alpha)),
                transform: self.transform.then_translate(Vec2::new(sh.dx, sh.dy)),
            });
        }
        let brush = self.effective_fill_brush();
        self.scene.push(DrawCommand::FillRect {
            rect,
            radii,
            brush,
            transform: self.transform,
        });
    }

    fn emit_stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let rect = KRect::new(x, y, x + w, y + h);
        let stroke = self.current_stroke();
        let brush = self.effective_stroke_brush();
        self.scene.push(DrawCommand::StrokeRect {
            rect,
            radii: None,
            stroke,
            brush,
            transform: self.transform,
        });
    }
}

// ── Cap/Join translation ────────────────────────────────────────────────────

fn to_urx_cap(c: Cap) -> UrxLineCap {
    match c {
        Cap::Butt   => UrxLineCap::Butt,
        Cap::Round  => UrxLineCap::Round,
        Cap::Square => UrxLineCap::Square,
    }
}

fn to_urx_join(j: Join) -> UrxLineJoin {
    match j {
        Join::Miter => UrxLineJoin::Miter,
        Join::Round => UrxLineJoin::Round,
        Join::Bevel => UrxLineJoin::Bevel,
    }
}

// ── Painter ─────────────────────────────────────────────────────────────────

impl Painter for UrxRenderContext {
    fn save(&mut self) {
        self.state_stack.push(SavedState {
            transform:     self.transform,
            fill_color:    self.fill_color,
            stroke_color:  self.stroke_color,
            stroke_width:  self.stroke_width,
            line_cap:      self.line_cap,
            line_join:     self.line_join,
            global_alpha:  self.global_alpha,
            font_info:     self.font_info.clone(),
            text_align:    self.text_align,
            text_baseline: self.text_baseline,
            blend_mode:    self.blend_mode,
            clip_pushes:   self.clip_pushes,
            fill_gradient: self.fill_gradient.clone(),
        });
        // Track new pushes at the new level — popped on restore.
        self.clip_pushes = 0;
    }

    fn restore(&mut self) {
        // Pop any clips pushed since the last save.
        for _ in 0..self.clip_pushes {
            self.scene.push(DrawCommand::PopClip);
        }
        if let Some(s) = self.state_stack.pop() {
            self.transform     = s.transform;
            self.fill_color    = s.fill_color;
            self.stroke_color  = s.stroke_color;
            self.stroke_width  = s.stroke_width;
            self.line_cap      = s.line_cap;
            self.line_join     = s.line_join;
            self.global_alpha  = s.global_alpha;
            self.font_info     = s.font_info;
            self.text_align    = s.text_align;
            self.text_baseline = s.text_baseline;
            self.blend_mode    = s.blend_mode;
            self.clip_pushes   = s.clip_pushes;
            self.fill_gradient = s.fill_gradient;
        }
    }

    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_translate(Vec2::new(x, y));
    }
    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.then_rotate(angle);
    }
    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.then_scale_non_uniform(x, y);
    }

    fn set_fill_color(&mut self, color: &str) { self.fill_color = parse_color(color); }
    fn set_stroke_color(&mut self, color: &str) { self.stroke_color = parse_color(color); }
    fn set_stroke_width(&mut self, width: f64) { self.stroke_width = width; }
    fn set_global_alpha(&mut self, alpha: f64) { self.global_alpha = alpha.clamp(0.0, 1.0); }
    fn set_line_dash(&mut self, _pattern: &[f64]) {
        // urx_core::Stroke has no dash field yet — consumers requesting
        // dashes through Painter currently get a solid stroke. Tracked
        // upstream; when DrawCommand learns dashes, wire here.
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

    fn begin_path(&mut self) { self.path.truncate(0); }
    fn move_to(&mut self, x: f64, y: f64) { self.path.move_to(KPoint::new(x, y)); }
    fn line_to(&mut self, x: f64, y: f64) {
        // Canvas2D tolerates `lineTo` on an empty path (starts a
        // subpath at that point). kurbo panics — open the subpath.
        self.ensure_subpath_open(KPoint::new(x, y));
        self.path.line_to(KPoint::new(x, y));
    }
    fn close_path(&mut self) {
        // No-op if no subpath is open — Canvas2D semantics.
        if !self.path.elements().is_empty()
            && !matches!(self.path.elements().last(), Some(kurbo::PathEl::ClosePath))
        {
            self.path.close_path();
        }
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.path.move_to(KPoint::new(x, y));
        self.path.line_to(KPoint::new(x + w, y));
        self.path.line_to(KPoint::new(x + w, y + h));
        self.path.line_to(KPoint::new(x, y + h));
        self.path.close_path();
    }
    fn arc(&mut self, cx: f64, cy: f64, radius: f64, start: f64, end: f64) {
        // kurbo::Arc → BezPath path-elements appended to the current path.
        // `append_iter` yields LineTo/CurveTo without a leading MoveTo —
        // valid for "continue current subpath", panics on an empty/just-
        // closed path. Emit MoveTo to the arc's starting point when
        // needed (Canvas2D semantics: arc on a fresh path starts a new
        // subpath at the first arc point).
        let arc = kurbo::Arc::new(
            KPoint::new(cx, cy),
            Vec2::new(radius, radius),
            start,
            end - start,
            0.0,
        );
        self.ensure_subpath_open(KPoint::new(
            cx + radius * start.cos(),
            cy + radius * start.sin(),
        ));
        for el in arc.append_iter(0.1) {
            self.path.push(el);
        }
    }
    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, _rot: f64, start: f64, end: f64) {
        let arc = kurbo::Arc::new(
            KPoint::new(cx, cy),
            Vec2::new(rx, ry),
            start,
            end - start,
            0.0,
        );
        self.ensure_subpath_open(KPoint::new(
            cx + rx * start.cos(),
            cy + ry * start.sin(),
        ));
        for el in arc.append_iter(0.1) {
            self.path.push(el);
        }
    }
    fn quadratic_curve_to(&mut self, cpx: f64, cpy: f64, x: f64, y: f64) {
        // kurbo `quad_to` requires an open subpath. Canvas2D starts
        // one implicitly at the control point's previous position;
        // we fall back to the curve start (close enough — only fires
        // when the consumer skipped `move_to`).
        self.ensure_subpath_open(KPoint::new(cpx, cpy));
        self.path.quad_to(KPoint::new(cpx, cpy), KPoint::new(x, y));
    }
    fn bezier_curve_to(&mut self, cp1x: f64, cp1y: f64, cp2x: f64, cp2y: f64, x: f64, y: f64) {
        self.ensure_subpath_open(KPoint::new(cp1x, cp1y));
        self.path.curve_to(
            KPoint::new(cp1x, cp1y),
            KPoint::new(cp2x, cp2y),
            KPoint::new(x, y),
        );
    }

    fn stroke(&mut self) {
        if self.path.elements().is_empty() { return; }
        let stroke = self.current_stroke();
        let brush = self.effective_stroke_brush();
        self.scene.push(DrawCommand::StrokePath {
            path:      self.path.clone(),
            stroke,
            brush,
            transform: self.transform,
        });
    }

    fn fill(&mut self) {
        if self.path.elements().is_empty() { return; }
        if let Some(sh) = self.shadow.clone() {
            self.scene.push(DrawCommand::FillPath {
                path:      self.path.clone(),
                rule:      FillRule::NonZero,
                brush:     PenikoBrush::Solid(apply_alpha(sh.color, self.global_alpha)),
                transform: self.transform.then_translate(Vec2::new(sh.dx, sh.dy)),
            });
        }
        let brush = self.effective_fill_brush();
        self.scene.push(DrawCommand::FillPath {
            path:      self.path.clone(),
            rule:      FillRule::NonZero,
            brush,
            transform: self.transform,
        });
    }
}

// ── ShapeHelpers (only fill_rect + stroke_rect overridden) ─────────────────

impl ShapeHelpers for UrxRenderContext {
    fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.emit_fill_rect(x, y, w, h, None);
    }
    fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.emit_stroke_rect(x, y, w, h);
    }
    // fill_rounded_rect / stroke_rounded_rect override — use FillRect/StrokeRect
    // with radii so the backend can do rounded-rect-AA directly instead of
    // tessellating to a generic path.
    fn fill_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        let r = radius.clamp(0.0, (w / 2.0).min(h / 2.0)) as f32;
        self.emit_fill_rect(x, y, w, h, Some([r, r, r, r]));
    }
    fn stroke_rounded_rect(&mut self, x: f64, y: f64, w: f64, h: f64, radius: f64) {
        let r = radius.clamp(0.0, (w / 2.0).min(h / 2.0)) as f32;
        let rect = KRect::new(x, y, x + w, y + h);
        let stroke = self.current_stroke();
        let brush = self.effective_stroke_brush();
        self.scene.push(DrawCommand::StrokeRect {
            rect,
            radii: Some([r, r, r, r]),
            stroke,
            brush,
            transform: self.transform,
        });
    }
}

// ── Masking ────────────────────────────────────────────────────────────────

impl Masking for UrxRenderContext {
    fn clip(&mut self) {
        // urx_core's clip vocabulary is rect-only today; convert the current
        // path's AABB to a rect clip. Generic-path clipping is a future
        // DrawCommand extension — when added, swap the AABB approximation
        // for a true `PushClipPath` op.
        if self.path.elements().is_empty() { return; }
        use kurbo::Shape;
        let bbox = self.path.bounding_box();
        self.scene.push(DrawCommand::PushClipRect {
            rect:      bbox,
            transform: self.transform,
        });
        self.clip_pushes = self.clip_pushes.saturating_add(1);
    }

    fn clip_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.scene.push(DrawCommand::PushClipRect {
            rect:      KRect::new(x, y, x + width, y + height),
            transform: self.transform,
        });
        self.clip_pushes = self.clip_pushes.saturating_add(1);
    }
}

// ── Effects ────────────────────────────────────────────────────────────────

impl Effects for UrxRenderContext {
    fn set_shadow(&mut self, dx: f64, dy: f64, _blur: f64, color: &str) {
        // Blur is approximated as a translated copy (matches the
        // vello-cpu/gpu approach until urx_core gains a blur op).
        self.shadow = Some(ShadowState { dx, dy, color: parse_color(color) });
    }
    fn clear_shadow(&mut self) { self.shadow = None; }
    fn set_blend_mode(&mut self, mode: UzorBlendMode) { self.blend_mode = mode; }
}

// ── Gradient ───────────────────────────────────────────────────────────────

fn build_gradient_stops(stops: &[(f32, &str)]) -> ColorStops {
    let v: Vec<ColorStop> = stops
        .iter()
        .map(|(o, hex)| ColorStop { offset: *o, color: parse_color(hex).into() })
        .collect();
    ColorStops::from(v.as_slice())
}

impl GradientPainter for UrxRenderContext {
    fn fill_linear_gradient(
        &mut self,
        stops: &[(f32, &str)],
        x1: f64, y1: f64, x2: f64, y2: f64,
    ) {
        // Stash the gradient so the next `fill()` (or fill_rect) consumes it
        // — matches the Canvas2D `createLinearGradient → ctx.fillStyle =
        // gradient → ctx.fill()` pattern that uzor's other backends honour.
        let kind = LinearGradientPosition {
            start: KPoint::new(x1, y1),
            end:   KPoint::new(x2, y2),
        };
        let g = Gradient {
            kind:   kind.into(),
            stops:  build_gradient_stops(stops),
            extend: Extend::Pad,
            ..Gradient::default()
        };
        self.fill_gradient = Some(g);
    }

    fn fill_radial_gradient(
        &mut self,
        cx: f64, cy: f64, r: f64,
        stops: &[(f32, &str)],
        _x: f64, _y: f64, _w: f64, _h: f64,
    ) {
        let kind = RadialGradientPosition {
            start_center: KPoint::new(cx, cy),
            start_radius: 0.0,
            end_center:   KPoint::new(cx, cy),
            end_radius:   r as f32,
        };
        let g = Gradient {
            kind:   kind.into(),
            stops:  build_gradient_stops(stops),
            extend: Extend::Pad,
            ..Gradient::default()
        };
        self.fill_gradient = Some(g);
    }
}

// ── BatchPainter (uses defaults; defaults are fine — they unroll to N
// individual `stroke()`/`fill()` calls which we already handle) ─────────────

impl BatchPainter for UrxRenderContext {
    // All methods have default impls in the trait that delegate to single
    // ops; we accept them as-is. A future optimisation can emit one big
    // FillPath per batch instead.
    fn draw_line_batch(&mut self, lines: &[LineSegment], color: &str, width: f64) {
        if lines.is_empty() { return; }
        self.set_stroke_color(color);
        self.set_stroke_width(width);
        let mut path = BezPath::new();
        for l in lines {
            path.move_to(KPoint::new(l.x1, l.y1));
            path.line_to(KPoint::new(l.x2, l.y2));
        }
        let stroke = self.current_stroke();
        let brush = self.effective_stroke_brush();
        self.scene.push(DrawCommand::StrokePath {
            path,
            stroke,
            brush,
            transform: self.transform,
        });
    }
    fn draw_circle_batch(&mut self, circles: &[CircleBatch], color: &str) {
        if circles.is_empty() { return; }
        self.set_fill_color(color);
        let mut path = BezPath::new();
        for c in circles {
            let circle = kurbo::Circle::new(KPoint::new(c.cx, c.cy), c.r);
            // kurbo::Shape::path_elements yields the iterator we extend with.
            path.extend(circle.into_path(0.1));
        }
        let brush = self.effective_fill_brush();
        self.scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush,
            transform: self.transform,
        });
    }
}

// ── TextRenderer ───────────────────────────────────────────────────────────

impl TextRenderer for UrxRenderContext {
    fn set_font(&mut self, font: &str) { self.font_info = parse_css_font(font); }
    fn set_text_align(&mut self, align: TextAlign) { self.text_align = align; }
    fn set_text_baseline(&mut self, baseline: TextBaseline) { self.text_baseline = baseline; }

    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() { return; }
        // urx_core::DrawCommand::GlyphRun expects pre-shaped glyph_ids, but
        // uzor::shaper exposes cluster-level metrics (GlyphMetric has
        // `x_offset/advance`, not `glyph_id`). To get a working text path
        // through the URX pipeline today, render text as a vector outline
        // path via `shaper::text_to_path` (SVG path string) → kurbo::BezPath
        // → DrawCommand::FillPath. This honours the current fill brush +
        // transform exactly like any other shape.
        //
        // The outline coordinates are in font-em-units scaled by font size
        // already (uzor::shaper does that internally), with baseline at y=0.
        // We apply the requested baseline / text-align via an extra
        // translation on top of the user transform.
        let font_str = font_string(&self.font_info);
        let total_w  = self.measure_text(text);
        let x_off = match self.text_align {
            TextAlign::Center => -total_w / 2.0,
            TextAlign::Right  => -total_w,
            _ => 0.0,
        };
        let y_off = match self.text_baseline {
            TextBaseline::Top    => self.font_info.size as f64 * 0.8,
            TextBaseline::Middle => self.font_info.size as f64 * 0.35,
            TextBaseline::Bottom => 0.0,
            _ => self.font_info.size as f64 * 0.35,
        };

        let svg = uzor::shaper::text_to_path(text, &font_str);
        if svg.is_empty() { return; }
        let path = match BezPath::from_svg(&svg) {
            Ok(p) => p,
            Err(_) => return,
        };

        let text_xform = KAffine::translate((x + x_off, y + y_off));
        let combined   = self.transform * text_xform;

        if let Some(sh) = self.shadow.clone() {
            self.scene.push(DrawCommand::FillPath {
                path: path.clone(),
                rule: FillRule::NonZero,
                brush: PenikoBrush::Solid(apply_alpha(sh.color, self.global_alpha)),
                transform: combined.then_translate(Vec2::new(sh.dx, sh.dy)),
            });
        }
        let brush = self.effective_fill_brush();
        self.scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush,
            transform: combined,
        });
    }
}

fn font_string(info: &FontInfo) -> String {
    let family = match info.family {
        FontFamily::Roboto        => "Roboto",
        FontFamily::PtRootUi      => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    };
    let mut parts: Vec<String> = Vec::with_capacity(4);
    if info.italic { parts.push("italic".into()); }
    if info.bold   { parts.push("bold".into()); }
    parts.push(format!("{}px", info.size));
    parts.push(family.into());
    parts.join(" ")
}

// ── TextMetrics ────────────────────────────────────────────────────────────

impl TextMetrics for UrxRenderContext {
    fn measure_text(&self, text: &str) -> f64 {
        let m = uzor::shaper::measure_glyphs(text, &font_string(&self.font_info));
        m.last().map(|g| g.x_offset + g.advance).unwrap_or(0.0)
    }

    fn text_bounds(&self, text: &str, font: &str) -> TextBounds {
        let info = parse_css_font(font);
        let m = uzor::shaper::measure_glyphs(text, &font_string(&info));
        let w = m.last().map(|g| g.x_offset + g.advance).unwrap_or(0.0);
        let ascent  = info.size as f64 * 0.9;
        let descent = info.size as f64 * 0.3;
        TextBounds {
            x: 0.0, y: -ascent, w, h: ascent + descent, ascent, descent,
        }
    }

    fn measure_text_glyphs(&self, text: &str, font: &str) -> Vec<GlyphMetric> {
        uzor::shaper::measure_glyphs(text, font)
    }

    fn text_to_path(&self, text: &str, font: &str) -> String {
        uzor::shaper::text_to_path(text, font)
    }
}

// ── UiEffectHelpers (all methods have default impls — empty block) ─────────

impl UiEffectHelpers for UrxRenderContext {}

// ── RenderContext supertrait ───────────────────────────────────────────────

impl UzorRenderContext for UrxRenderContext {
    fn dpr(&self) -> f64 { self.dpr }
}

impl RenderContextExt for UrxRenderContext {
    type BlurImage = ();
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::render::Painter;

    #[test]
    fn fill_rect_emits_one_fillrect() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.set_fill_color("#ff0000");
        ShapeHelpers::fill_rect(&mut ctx, 10.0, 20.0, 30.0, 40.0);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        match &scene.commands[0] {
            DrawCommand::FillRect { rect, .. } => {
                assert_eq!(rect.x0, 10.0);
                assert_eq!(rect.y0, 20.0);
                assert_eq!(rect.x1, 40.0);
                assert_eq!(rect.y1, 60.0);
            }
            other => panic!("expected FillRect, got {:?}", other),
        }
    }

    #[test]
    fn save_restore_pops_pushed_clip() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.save();
        ctx.clip_rect(0.0, 0.0, 50.0, 50.0);
        ctx.restore();
        let scene = ctx.take_scene();
        // Should be exactly 2 ops: PushClipRect + PopClip.
        assert_eq!(scene.commands.len(), 2);
        assert!(matches!(scene.commands[0], DrawCommand::PushClipRect { .. }));
        assert!(matches!(scene.commands[1], DrawCommand::PopClip));
    }

    #[test]
    fn path_fill_emits_fillpath() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.set_fill_color("#0000ff");
        ctx.begin_path();
        ctx.move_to(0.0, 0.0);
        ctx.line_to(10.0, 0.0);
        ctx.line_to(10.0, 10.0);
        ctx.close_path();
        Painter::fill(&mut ctx);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], DrawCommand::FillPath { .. }));
    }

    #[test]
    fn transform_translate_propagates_to_emitted_op() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.translate(5.0, 7.0);
        ctx.set_fill_color("#00ff00");
        ShapeHelpers::fill_rect(&mut ctx, 0.0, 0.0, 10.0, 10.0);
        let scene = ctx.take_scene();
        match &scene.commands[0] {
            DrawCommand::FillRect { transform, .. } => {
                // The translate ends up as the last column of the Affine.
                let coeffs = transform.as_coeffs();
                assert_eq!(coeffs[4], 5.0);
                assert_eq!(coeffs[5], 7.0);
            }
            _ => panic!("expected FillRect"),
        }
    }

    // ── Defensive subpath-open invariants (2026-06-09 owner-driven) ──
    //
    // kurbo's BezPath panics on any append after an empty or just-
    // closed path. Canvas2D semantics tolerate every path-op as a
    // fresh-subpath starter. UrxRenderContext bridges the two —
    // these tests pin that bridge.

    // Direct-call tests on the inherent impl block (the UzorRenderContext
    // trait impls invoke the same path-op methods). UrxRenderContext
    // exposes them via the trait — UFCS isn't available because the
    // RenderContext supertrait composition causes ambiguity; we exercise
    // via the trait object instead.

    #[test]
    fn arc_on_empty_path_does_not_panic() {
        // Before the fix this panicked with "BezPath must begin with MoveTo".
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        let rc: &mut dyn uzor::render::RenderContext = &mut ctx;
        rc.begin_path();
        rc.arc(50.0, 50.0, 20.0, 0.0, std::f64::consts::PI);
        rc.stroke();
        let scene = ctx.take_scene();
        assert!(scene.commands.iter().any(|c| matches!(c, DrawCommand::StrokePath { .. })));
    }

    #[test]
    fn ellipse_on_empty_path_does_not_panic() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        let rc: &mut dyn uzor::render::RenderContext = &mut ctx;
        rc.begin_path();
        rc.ellipse(50.0, 50.0, 20.0, 30.0, 0.0, 0.0, std::f64::consts::TAU);
        rc.fill();
        let _ = ctx.take_scene();
    }

    #[test]
    fn line_to_on_empty_path_does_not_panic() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        let rc: &mut dyn uzor::render::RenderContext = &mut ctx;
        rc.begin_path();
        rc.line_to(10.0, 20.0);
        rc.line_to(30.0, 40.0);
        rc.stroke();
        let _ = ctx.take_scene();
    }

    #[test]
    fn close_path_on_empty_is_silent_noop() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        let rc: &mut dyn uzor::render::RenderContext = &mut ctx;
        rc.begin_path();
        rc.close_path();
        let _ = ctx.take_scene();
    }

    #[test]
    fn bezier_after_close_does_not_panic() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        let rc: &mut dyn uzor::render::RenderContext = &mut ctx;
        rc.begin_path();
        rc.move_to(10.0, 10.0);
        rc.line_to(20.0, 20.0);
        rc.close_path();
        rc.bezier_curve_to(30.0, 30.0, 40.0, 40.0, 50.0, 50.0);
        rc.stroke();
        let _ = ctx.take_scene();
    }
}
