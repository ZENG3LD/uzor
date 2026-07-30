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

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use kurbo::{
    Affine as KAffine, BezPath, Cap, Join, Point as KPoint, Rect as KRect, Shape, Vec2,
};
use peniko::{
    Brush as PenikoBrush, Color, ColorStop, ColorStops, Extend, Gradient,
    LinearGradientPosition, RadialGradientPosition,
};

use uzor_urx_core::scene::{
    Dash as UrxDash, DrawCommand, FillRule, FontId, Glyph, LineCap as UrxLineCap,
    LineJoin as UrxLineJoin, Scene, Stroke as UrxStroke,
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
    /// Current dash pattern (Canvas2D's `setLineDash`) — `None` is a
    /// plain solid stroke. Local/user-space lengths, same coordinate
    /// space `stroke_width` and every path coordinate already use — see
    /// `uzor_urx_core::scene::Dash`'s own doc comment for why that
    /// space is the correct one (the emitted `DrawCommand`'s own
    /// `transform` field scales the pattern the same way it scales the
    /// rest of the geometry).
    line_dash:     Option<Vec<f64>>,
    global_alpha:  f64,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    blend_mode:    UzorBlendMode,
    /// How many `PushClipRect`/`PushClipRoundedRect` ops were emitted at
    /// this save level. `restore()` emits matching `PopClip`s.
    clip_pushes:   u32,
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
    /// See `SavedState::line_dash`'s own doc comment.
    line_dash:     Option<Vec<f64>>,
    global_alpha:  f64,
    font_info:     FontInfo,
    text_align:    TextAlign,
    text_baseline: TextBaseline,
    blend_mode:    UzorBlendMode,

    /// Current Canvas2D-style path buffer.
    path: BezPath,

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
            line_dash:     None,
            global_alpha:  1.0,
            font_info:     FontInfo::default(),
            text_align:    TextAlign::Left,
            text_baseline: TextBaseline::Middle,
            blend_mode:    UzorBlendMode::Normal,
            path:          BezPath::new(),
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

    fn effective_fill_brush(&self) -> PenikoBrush {
        PenikoBrush::Solid(apply_alpha(self.fill_color, self.global_alpha))
    }

    fn effective_stroke_brush(&self) -> PenikoBrush {
        PenikoBrush::Solid(apply_alpha(self.stroke_color, self.global_alpha))
    }

    /// Alpha-scale a freshly-built [`Gradient`] by `global_alpha` (a
    /// gradient's stops carry their own per-stop alpha already;
    /// `global_alpha` composes on top, same as `effective_fill_brush`'s
    /// solid-color path applies it via [`apply_alpha`]). Used ONLY by
    /// `GradientPainter`'s two entry points below — unlike the old
    /// `fill_gradient`-stash design, no state survives past the single
    /// call that builds this brush (see `fill_linear_gradient`'s own doc
    /// comment for why the stash was removed entirely).
    fn gradient_brush(&self, g: Gradient) -> PenikoBrush {
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
    }

    /// Shared draw core for `fill()` and the two `GradientPainter` entry
    /// points below — emits the shadow pre-pass (if any) then the main
    /// `FillPath` for the CURRENT path buffer, using `brush`. No-op on
    /// an empty path (Canvas2D semantics: filling nothing draws
    /// nothing). Does NOT clear `self.path` — matches `fill()`'s
    /// pre-existing behaviour (only `begin_path()` clears it, so a
    /// caller CAN fill the same path twice, e.g. once solid then once
    /// with a gradient, same as Canvas2D's own `fill()` contract).
    fn emit_fill_path(&mut self, brush: PenikoBrush) {
        if self.path.elements().is_empty() { return; }
        if let Some(sh) = self.shadow.clone() {
            self.scene.push(DrawCommand::FillPath {
                path:      self.path.clone(),
                rule:      FillRule::NonZero,
                brush:     PenikoBrush::Solid(apply_alpha(sh.color, self.global_alpha)),
                transform: self.transform.then_translate(Vec2::new(sh.dx, sh.dy)),
            });
        }
        self.scene.push(DrawCommand::FillPath {
            path: self.path.clone(),
            rule: FillRule::NonZero,
            brush,
            transform: self.transform,
        });
    }

    fn current_stroke(&self) -> UrxStroke {
        UrxStroke {
            width:       self.stroke_width as f32,
            miter_limit: 4.0,
            cap:         to_urx_cap(self.line_cap),
            join:        to_urx_join(self.line_join),
            dash:        self.line_dash.as_ref().map(|pattern| UrxDash {
                pattern: pattern.iter().map(|&v| v as f32).collect(),
                // Canvas2D's `lineDashOffset` has no `Painter`-trait
                // setter (only `set_line_dash(pattern)` exists) — every
                // dash always starts at phase 0 until the trait grows
                // one.
                phase: 0.0,
            }),
        }
    }

    /// Emit a fill rect (with optional rounded radii). Honours the
    /// active shadow + current solid `fill_color` — a gradient fill
    /// goes through `GradientPainter::fill_linear_gradient`/
    /// `fill_radial_gradient` instead, which paint the CURRENT PATH
    /// directly (Canvas2D's `rect()` + `fill_linear_gradient()` idiom,
    /// see that method's own doc comment), not this rect-shorthand path.
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
            line_dash:     self.line_dash.clone(),
            global_alpha:  self.global_alpha,
            font_info:     self.font_info.clone(),
            text_align:    self.text_align,
            text_baseline: self.text_baseline,
            blend_mode:    self.blend_mode,
            clip_pushes:   self.clip_pushes,
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
            self.line_dash     = s.line_dash;
            self.global_alpha  = s.global_alpha;
            self.font_info     = s.font_info;
            self.text_align    = s.text_align;
            self.text_baseline = s.text_baseline;
            self.blend_mode    = s.blend_mode;
            self.clip_pushes   = s.clip_pushes;
        }
    }

    // Canvas-style incremental CTM: a later `translate`/`rotate`/`scale`
    // call is expressed in the LOCAL frame the earlier calls already
    // established (`self * Op`, kurbo's `pre_*` family), NOT the outer/
    // world frame (`Op * self`, `then_*`) — see `uzor-render-tiny-skia`'s
    // identically-shaped `Painter::translate`/`rotate`/`scale` (the
    // reference-correct backend) and this crate's own new
    // `translate_then_rotate_matches_local_frame_composition` test below.
    fn translate(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_translate(Vec2::new(x, y));
    }
    fn rotate(&mut self, angle: f64) {
        self.transform = self.transform.pre_rotate(angle);
    }
    fn scale(&mut self, x: f64, y: f64) {
        self.transform = self.transform.pre_scale_non_uniform(x, y);
    }

    fn set_fill_color(&mut self, color: &str) { self.fill_color = parse_color(color); }
    fn set_stroke_color(&mut self, color: &str) { self.stroke_color = parse_color(color); }
    fn set_stroke_width(&mut self, width: f64) { self.stroke_width = width; }
    fn set_global_alpha(&mut self, alpha: f64) { self.global_alpha = alpha.clamp(0.0, 1.0); }
    fn set_line_dash(&mut self, pattern: &[f64]) {
        // Canvas2D semantics: an empty pattern clears dashing back to a
        // solid stroke (mirrors `uzor-render-tiny-skia::set_line_dash`'s
        // identical `is_empty()` check — the reference-correct backend
        // this crate's own `current_stroke` doc comments already point
        // to elsewhere in this file).
        self.line_dash = if pattern.is_empty() { None } else { Some(pattern.to_vec()) };
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
        let brush = self.effective_fill_brush();
        self.emit_fill_path(brush);
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
    /// Paints the CURRENT path (built via `begin_path()`/`rect()`/etc.,
    /// same as `fill()`) with a linear gradient — immediately, not
    /// stashed for a later `fill()` call. This matches EVERY other uzor
    /// backend's own `GradientPainter` impl (`uzor-render-tiny-skia`,
    /// `uzor-render-vello-{cpu,gpu,hybrid}` all take the path, tessellate/
    /// rasterise it, and consume it right here — see their own
    /// `fill_linear_gradient` bodies) and the documented contract
    /// `uzor::core::render::svg::draw_svg_multicolor` relies on ("vello's
    /// fill()/stroke()/fill_linear_gradient() consume the path").
    ///
    /// A prior version of this method stashed the gradient into a
    /// `self.fill_gradient: Option<Gradient>` field for the NEXT `fill()`
    /// call to consume — which meant a caller that (correctly, per every
    /// other backend's contract) never called a separate `fill()`
    /// afterward painted NOTHING here, and the stashed gradient then
    /// leaked into whatever fill-brush-consuming call came next (a
    /// `fill_rect`/`fill()`/glyph draw with its own, unrelated solid
    /// color got silently painted with this gradient instead). That
    /// field is gone entirely now — there is no pending state left to
    /// leak, structurally, not just by convention.
    fn fill_linear_gradient(
        &mut self,
        stops: &[(f32, &str)],
        x1: f64, y1: f64, x2: f64, y2: f64,
    ) {
        if stops.is_empty() { return; }
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
        let brush = self.gradient_brush(g);
        self.emit_fill_path(brush);
    }

    /// Same immediate-consume contract as `fill_linear_gradient` above —
    /// see that method's own doc comment for the full reasoning.
    fn fill_radial_gradient(
        &mut self,
        cx: f64, cy: f64, r: f64,
        stops: &[(f32, &str)],
        _x: f64, _y: f64, _w: f64, _h: f64,
    ) {
        if stops.is_empty() { return; }
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
        let brush = self.gradient_brush(g);
        self.emit_fill_path(brush);
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
        let stroke = self.current_stroke();
        let brush = self.effective_stroke_brush();
        let mut path = BezPath::new();
        for line in lines {
            path.move_to(KPoint::new(line.x1, line.y1));
            path.line_to(KPoint::new(line.x2, line.y2));
        }
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

// ── GlyphRun bridging (URX text-gamma follow-up, 2026-07-24) ───────────────
//
// `fill_text` used to ALWAYS render text as a vector-outline path
// (`DrawCommand::FillPath`) — see the historical note kept on
// `fill_path_segment`'s own doc comment for why that was the original
// choice and why it's no longer a structural blocker. It now emits
// `DrawCommand::GlyphRun` (the SAME hinted-swash atlas path the crate-
// level parity suite already proves byte-tight CPU-vs-native, glyph
// fixture 0.000%) whenever it safely can, falling back to the outline
// path only for the two cases that genuinely cannot map onto a
// glyph-id run — both counted, never silent (see `emit_text_segments`
// and `fill_text`'s own transform check).

/// Process-wide cache: `uzor::shaper::ShaperFontId` (cosmic-text's own
/// per-face identity) -> `uzor_urx_glyph::FontId` (URX's own glyph-atlas
/// registry identity). `uzor_urx_glyph::register_font` mints a NEW,
/// distinct `FontId` on every call (not idempotent by content — see its
/// own doc comment) — without this cache, drawing the same font twice
/// would register it twice, breaking `GlyphKey`-based atlas/LRU cache
/// reuse across draws (every `GlyphKey` embeds the `FontId`, so two
/// registrations of the byte-identical font would never hit each
/// other's cached rasterisations).
fn font_id_cache() -> &'static Mutex<HashMap<uzor::shaper::ShaperFontId, FontId>> {
    static CACHE: OnceLock<Mutex<HashMap<uzor::shaper::ShaperFontId, FontId>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve (registering into `uzor_urx_glyph`'s registry on first use)
/// the `FontId` for a shaped segment's font. `None` only if the
/// process-wide font database somehow no longer has the face, or
/// `register_font` rejects bytes that JUST successfully shaped text via
/// cosmic-text moments earlier (should not happen either way — a
/// defensive `None`, not an expected outcome).
///
/// Holds the cache's lock across the ENTIRE check-then-register-then-
/// insert sequence (never releases it between the miss check and the
/// insert) — a `check, unlock, register, re-lock, insert` version of
/// this function would race two concurrent callers seeing the SAME
/// miss and each registering (and caching) their OWN distinct `FontId`
/// for the byte-identical font, silently defeating the whole point of
/// this cache (confirmed empirically: an earlier check-then-insert
/// version of this function passed in isolation but flaked under
/// `cargo test`'s default parallel-thread execution).
fn resolve_font_id(shaper_font: uzor::shaper::ShaperFontId) -> Option<FontId> {
    let mut cache = font_id_cache().lock().ok()?;
    if let Some(&id) = cache.get(&shaper_font) {
        return Some(id);
    }
    let bytes = uzor::shaper::font_bytes_for(shaper_font)?;
    let id = uzor_urx_glyph::register_font(bytes).ok()?;
    cache.insert(shaper_font, id);
    Some(id)
}

/// Family names in this workspace's EMBEDDED font set whose glyphs a
/// monochrome-coverage rasteriser (`uzor_urx_glyph::draw_glyph_run`,
/// `swash::zeno::Format::Alpha`-only — see its own module doc: "COLR/
/// CBDT/CBLC colour glyphs — deferred") cannot represent. Only
/// `uzor_fonts::NOTO_COLOR_EMOJI` qualifies (the embedded `NOTO_EMOJI`
/// — no "Color" — is a plain monochrome outline font and renders
/// through `GlyphRun` exactly like any other face).
const COLOR_GLYPH_FAMILIES: &[&str] = &["Noto Color Emoji"];

fn is_color_glyph_font(shaper_font: uzor::shaper::ShaperFontId) -> bool {
    uzor::shaper::font_family_for(shaper_font)
        .is_some_and(|family| COLOR_GLYPH_FAMILIES.contains(&family.as_str()))
}

/// `DrawCommand::GlyphRun`'s `transform` is translate-ONLY downstream —
/// both `uzor-urx-cpu::draw_glyph_run` and `uzor-urx-wgpu::encode_glyph_run`
/// extract ONLY the translation component (design's own deliberate
/// CPU/GPU parity choice, Wave 2). A non-identity LINEAR part (scale /
/// rotate / shear) on the CURRENT canvas transform can't be represented
/// that way — `fill_text` checks this before choosing the `GlyphRun`
/// path (falls back to the vector-outline path, counted, when false).
fn is_translation_only(t: KAffine) -> bool {
    let c = t.as_coeffs();
    const EPS: f64 = 1e-6;
    (c[0] - 1.0).abs() < EPS && c[1].abs() < EPS && c[2].abs() < EPS && (c[3] - 1.0).abs() < EPS
}

// ── TextRenderer ───────────────────────────────────────────────────────────

impl TextRenderer for UrxRenderContext {
    fn set_font(&mut self, font: &str) { self.font_info = parse_css_font(font); }
    fn set_text_align(&mut self, align: TextAlign) { self.text_align = align; }
    fn set_text_baseline(&mut self, baseline: TextBaseline) { self.text_baseline = baseline; }

    fn fill_text(&mut self, text: &str, x: f64, y: f64) {
        if text.is_empty() { return; }

        let font_str = font_string(&self.font_info);
        let total_w  = self.measure_text(text);
        let x_off = match self.text_align {
            TextAlign::Center => -total_w / 2.0,
            TextAlign::Right  => -total_w,
            _ => 0.0,
        };
        // Exhaustive match (no `_` wildcard) — `TextBaseline::Alphabetic`
        // used to silently fall into a wildcard arm that applied
        // `Middle`'s offset (an extra `size * 0.35` shift downward).
        // "Alphabetic" means the caller's own `y` IS ALREADY the
        // baseline (standard Canvas2D/CSS definition) — correct offset
        // is `0.0`, same as `Bottom`. `uzor-render-vello-cpu` had the
        // IDENTICAL bug (fixed the same pass, 2026-07-24, while
        // calibrating the typography fixture's line positions against
        // it) — fixing only one side would have introduced a NEW
        // baseline disagreement between the two legs instead of
        // removing one.
        let y_off = match self.text_baseline {
            TextBaseline::Top        => self.font_info.size as f64 * 0.8,
            TextBaseline::Middle     => self.font_info.size as f64 * 0.35,
            TextBaseline::Bottom     => 0.0,
            TextBaseline::Alphabetic => 0.0,
        };
        let text_xform = KAffine::translate((x + x_off, y + y_off));
        let combined   = self.transform * text_xform;

        if !is_translation_only(self.transform) {
            metrics::counter!(
                uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                "kind" => "urx_filltext_glyphrun_transform_fallback",
            ).increment(1);
            self.fill_text_as_path(text, &font_str, combined);
            return;
        }

        let segments = uzor::shaper::shape_glyph_runs(text, &font_str);
        if segments.is_empty() {
            // Nothing shaped (e.g. whitespace-only text) — nothing to
            // fall back to either; same silent no-op Canvas2D itself
            // has for invisible content.
            return;
        }

        if let Some(sh) = self.shadow.clone() {
            let shadow_xform = combined.then_translate(Vec2::new(sh.dx, sh.dy));
            let shadow_brush = PenikoBrush::Solid(apply_alpha(sh.color, self.global_alpha));
            self.emit_text_segments(&segments, &font_str, shadow_brush, shadow_xform);
        }
        let brush = self.effective_fill_brush();
        self.emit_text_segments(&segments, &font_str, brush, combined);
    }
}

impl UrxRenderContext {
    /// Emit one `DrawCommand::GlyphRun` per segment (registering/caching
    /// a `FontId` for each distinct font on first use). Color-glyph
    /// segments (`is_color_glyph_font`) and segments whose font bytes
    /// are unexpectedly unavailable both fall back to the vector-outline
    /// path for JUST that segment — shifted to the segment's own
    /// starting pen position — never silently dropped (both counted).
    fn emit_text_segments(
        &mut self,
        segments: &[uzor::shaper::GlyphSegment],
        font_str: &str,
        brush: PenikoBrush,
        transform: KAffine,
    ) {
        for seg in segments {
            let seg_origin_x = seg.glyphs.first().map(|g| g.x as f64).unwrap_or(0.0);

            if is_color_glyph_font(seg.font) {
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "urx_filltext_glyphrun_color_font_fallback",
                ).increment(1);
                self.fill_path_segment(&seg.text, font_str, brush.clone(), transform.then_translate(Vec2::new(seg_origin_x, 0.0)));
                continue;
            }

            let Some(font_id) = resolve_font_id(seg.font) else {
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "urx_filltext_glyphrun_font_bytes_unavailable",
                ).increment(1);
                self.fill_path_segment(&seg.text, font_str, brush.clone(), transform.then_translate(Vec2::new(seg_origin_x, 0.0)));
                continue;
            };

            let glyphs: Vec<Glyph> =
                seg.glyphs.iter().map(|g| Glyph { glyph_id: g.glyph_id, x: g.x, y: g.y }).collect();
            self.scene.push(DrawCommand::GlyphRun {
                glyphs,
                font: font_id,
                font_size: seg.font_size,
                brush: brush.clone(),
                transform,
                text: Some(seg.text.clone()),
            });
        }
    }

    /// Whole-call vector-outline fallback (non-translation transform
    /// case) — same shadow-then-main sequencing `fill_text` always used
    /// before this change.
    fn fill_text_as_path(&mut self, text: &str, font_str: &str, transform: KAffine) {
        if let Some(sh) = self.shadow.clone() {
            let shadow_xform = transform.then_translate(Vec2::new(sh.dx, sh.dy));
            self.fill_path_segment(text, font_str, PenikoBrush::Solid(apply_alpha(sh.color, self.global_alpha)), shadow_xform);
        }
        let brush = self.effective_fill_brush();
        self.fill_path_segment(text, font_str, brush, transform);
    }

    /// Render `text` as a vector-outline path via `shaper::text_to_path`
    /// (SVG path string) → `kurbo::BezPath` → `DrawCommand::FillPath`.
    /// This was `fill_text`'s ONLY path before this change — `uzor::shaper`
    /// exposed cluster-level metrics only (`GlyphMetric` has
    /// `x_offset/advance`, not a raw `glyph_id`), so there was no glyph
    /// identity to build a `DrawCommand::GlyphRun` from. That gap closed
    /// when `uzor::shaper::shape_glyph_runs` was added (cosmic-text's own
    /// `LayoutGlyph.glyph_id`/`.font_id` were ALWAYS available internally
    /// — `text_to_path_uncached` already threads the same `glyph_id`
    /// into `SwashCache::get_outline_commands` to build ITS OWN outlines
    /// — just never surfaced to a caller before). This function now
    /// serves only the two genuinely-can't-map cases above.
    fn fill_path_segment(&mut self, text: &str, font_str: &str, brush: PenikoBrush, transform: KAffine) {
        let svg = uzor::shaper::text_to_path(text, font_str);
        if svg.is_empty() { return; }
        let Ok(path) = BezPath::from_svg(&svg) else { return; };
        self.scene.push(DrawCommand::FillPath { path, rule: FillRule::NonZero, brush, transform });
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

    /// Real word-wrap via cosmic-text `Wrap::Word` (same delegation as the
    /// other shaper-backed backends; the trait default is a greedy
    /// heuristic).
    fn measure_text_wrapped(&self, text: &str, font: &str, max_width: f64) -> Vec<uzor::render::WrappedLine> {
        uzor::shaper::measure_glyphs_wrapped(text, font, max_width)
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

    #[test]
    fn solid_line_batch_emits_one_compound_stroke_path() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        BatchPainter::draw_line_batch(
            &mut ctx,
            &[
                LineSegment { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0 },
                LineSegment { x1: 5.0, y1: 6.0, x2: 7.0, y2: 8.0 },
            ],
            "#ffffff",
            1.5,
        );

        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(
            &scene.commands[0],
            DrawCommand::StrokePath { path, stroke, .. }
                if path.elements().len() == 4 && stroke.dash.is_none()
        ));
    }

    #[test]
    fn dashed_line_batch_retains_compound_stroke_path_semantics() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        Painter::set_line_dash(&mut ctx, &[8.0, 5.0]);
        BatchPainter::draw_line_batch(
            &mut ctx,
            &[
                LineSegment { x1: 1.0, y1: 2.0, x2: 3.0, y2: 4.0 },
                LineSegment { x1: 5.0, y1: 6.0, x2: 7.0, y2: 8.0 },
            ],
            "#ffffff",
            1.5,
        );

        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(
            &scene.commands[0],
            DrawCommand::StrokePath { stroke, .. } if stroke.dash.is_some()
        ));
    }
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
    fn set_line_dash_carries_the_pattern_onto_the_emitted_stroke_path() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.set_stroke_color("#ffffff");
        Painter::set_line_dash(&mut ctx, &[5.0, 3.0]);
        ctx.begin_path();
        ctx.move_to(0.0, 0.0);
        ctx.line_to(10.0, 0.0);
        Painter::stroke(&mut ctx);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        match &scene.commands[0] {
            DrawCommand::StrokePath { stroke, .. } => {
                let dash = stroke.dash.as_ref().expect("dashed stroke() must carry a Some(Dash)");
                assert_eq!(dash.pattern, vec![5.0, 3.0]);
            }
            other => panic!("expected StrokePath, got {:?}", other),
        }
    }

    #[test]
    fn set_line_dash_empty_clears_a_previously_set_pattern() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.set_stroke_color("#ffffff");
        Painter::set_line_dash(&mut ctx, &[5.0, 3.0]);
        Painter::set_line_dash(&mut ctx, &[]);
        ctx.begin_path();
        ctx.move_to(0.0, 0.0);
        ctx.line_to(10.0, 0.0);
        Painter::stroke(&mut ctx);
        let scene = ctx.take_scene();
        match &scene.commands[0] {
            DrawCommand::StrokePath { stroke, .. } => {
                assert!(stroke.dash.is_none(), "an empty pattern must clear dashing back to solid");
            }
            other => panic!("expected StrokePath, got {:?}", other),
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

    // ── GradientPainter (bug fix 2026-07-25: gradient fills were never
    // emitted + leaked into the next fill-brush-consuming call) ─────────

    /// `fill_linear_gradient` must emit exactly one `FillPath` carrying
    /// a `Brush::Gradient` (not a bare no-op that leaves the shape
    /// unpainted, and not a stashed no-command state waiting for a
    /// caller to separately call `fill()`).
    #[test]
    fn fill_linear_gradient_emits_a_gradient_fillpath() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.begin_path();
        ctx.rect(0.0, 0.0, 20.0, 10.0);
        GradientPainter::fill_linear_gradient(
            &mut ctx,
            &[(0.0, "#ff0000"), (1.0, "#0000ff")],
            0.0, 0.0, 20.0, 0.0,
        );
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1, "the gradient fill must emit its own draw command, not silently no-op");
        match &scene.commands[0] {
            DrawCommand::FillPath { brush, .. } => {
                assert!(matches!(brush, PenikoBrush::Gradient(_)), "expected a Gradient brush, got {:?}", brush);
            }
            other => panic!("expected FillPath, got {:?}", other),
        }
    }

    /// Same immediate-emit contract for `fill_radial_gradient`.
    #[test]
    fn fill_radial_gradient_emits_a_gradient_fillpath() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);
        ctx.begin_path();
        ctx.rect(0.0, 0.0, 20.0, 20.0);
        GradientPainter::fill_radial_gradient(
            &mut ctx,
            10.0, 10.0, 10.0,
            &[(0.0, "#ff0000"), (1.0, "#0000ff")],
            0.0, 0.0, 20.0, 20.0,
        );
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        match &scene.commands[0] {
            DrawCommand::FillPath { brush, .. } => {
                assert!(matches!(brush, PenikoBrush::Gradient(_)), "expected a Gradient brush, got {:?}", brush);
            }
            other => panic!("expected FillPath, got {:?}", other),
        }
    }

    /// The defect's own reported shape: a gradient fill followed by an
    /// UNRELATED solid fill must leave the solid fill with its OWN
    /// color — never the gradient (the exact leak the coordinator saw
    /// on `figures_heatmap_backends.png`'s `"36.00"` tick label,
    /// painted in the colorbar gradient's first-stop orange instead of
    /// the theme's white). With the `fill_gradient` stash field removed
    /// entirely, there is no state left to leak — this test pins that.
    #[test]
    fn gradient_fill_does_not_leak_into_the_next_solid_fill() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);

        // First: a gradient-filled rect (mirrors uzor-figures's colorbar:
        // begin_path -> rect -> fill_linear_gradient, no separate fill()).
        ctx.begin_path();
        ctx.rect(0.0, 0.0, 20.0, 10.0);
        GradientPainter::fill_linear_gradient(
            &mut ctx,
            &[(0.0, "#ff8000"), (1.0, "#000000")],
            0.0, 0.0, 20.0, 0.0,
        );

        // Second: an UNRELATED solid-color fill (mirrors the colorbar's
        // own next call — `ctx.set_fill_color(&theme.label_color);
        // ctx.fill_text(...)`, which routes through `effective_fill_brush`
        // exactly like a plain `fill()` would).
        ctx.set_fill_color("#ffffff");
        ctx.begin_path();
        ctx.rect(30.0, 0.0, 10.0, 10.0);
        Painter::fill(&mut ctx);

        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 2);
        assert!(matches!(scene.commands[0], DrawCommand::FillPath { brush: PenikoBrush::Gradient(_), .. }));
        match &scene.commands[1] {
            DrawCommand::FillPath { brush: PenikoBrush::Solid(c), .. } => {
                assert_eq!(*c, Color::from_rgba8(255, 255, 255, 255), "the second fill must use ITS OWN white, not the gradient's orange leaking through");
            }
            other => panic!("expected a solid-white FillPath, got {:?}", other),
        }
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

    // ── GlyphRun bridging (2026-07-24 follow-up) ─────────────────────

    /// The whole point of this change: plain text under an identity/
    /// translation-only transform must emit `GlyphRun`, never
    /// `FillPath`, now.
    #[test]
    fn fill_text_emits_glyph_run_not_fillpath() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(200, 100);
        ctx.set_fill_color("#ffffff");
        TextRenderer::set_font(&mut ctx, "16px Roboto");
        TextRenderer::fill_text(&mut ctx, "Hello", 10.0, 20.0);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1, "one segment (one font) -> exactly one GlyphRun");
        match &scene.commands[0] {
            DrawCommand::GlyphRun { glyphs, font_size, text, .. } => {
                assert_eq!(glyphs.len(), 5, "one glyph per character, no ligatures in \"Hello\"");
                assert_eq!(*font_size, 16.0);
                assert_eq!(text.as_deref(), Some("Hello"));
            }
            other => panic!("expected GlyphRun, got {:?}", other),
        }
    }

    /// The `GlyphRun`'s own `transform` must carry the full `(x, y)`
    /// origin translation `fill_text` was called with (plus the
    /// baseline offset) — same contract `FillPath`'s transform used to
    /// carry, since CPU/GPU both read `coeffs[4]/[5]` off this value.
    #[test]
    fn fill_text_glyph_run_transform_carries_the_origin() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(200, 100);
        ctx.set_fill_color("#ffffff");
        TextRenderer::set_font(&mut ctx, "16px Roboto");
        TextRenderer::set_text_baseline(&mut ctx, TextBaseline::Top);
        TextRenderer::fill_text(&mut ctx, "Hi", 30.0, 40.0);
        let scene = ctx.take_scene();
        match &scene.commands[0] {
            DrawCommand::GlyphRun { transform, .. } => {
                let c = transform.as_coeffs();
                assert_eq!(c[4], 30.0);
                assert!((c[5] - (40.0 + 16.0 * 0.8)).abs() < 1e-6, "Top baseline offset must be added: got {}", c[5]);
            }
            other => panic!("expected GlyphRun, got {:?}", other),
        }
    }

    /// Repeated `fill_text` calls with the SAME font must reuse the
    /// SAME `FontId` — proves the `font_id_cache` actually caches
    /// (never mints a second registration for a font already seen).
    #[test]
    fn fill_text_reuses_the_same_font_id_across_calls() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(200, 100);
        ctx.set_fill_color("#ffffff");
        TextRenderer::set_font(&mut ctx, "16px Roboto");
        TextRenderer::fill_text(&mut ctx, "AB", 0.0, 20.0);
        TextRenderer::fill_text(&mut ctx, "CD", 0.0, 40.0);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 2);
        let font_id = |cmd: &DrawCommand| match cmd {
            DrawCommand::GlyphRun { font, .. } => *font,
            other => panic!("expected GlyphRun, got {:?}", other),
        };
        assert_eq!(font_id(&scene.commands[0]), font_id(&scene.commands[1]), "same font across two calls must reuse the SAME FontId");
    }

    /// A non-identity linear transform (scale) can't be represented by
    /// `GlyphRun`'s translate-only downstream contract — must fall back
    /// to `FillPath`, not silently drop the scale.
    #[test]
    fn fill_text_falls_back_to_fillpath_under_a_scale_transform() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(200, 100);
        ctx.set_fill_color("#ffffff");
        Painter::scale(&mut ctx, 2.0, 2.0);
        TextRenderer::set_font(&mut ctx, "16px Roboto");
        TextRenderer::fill_text(&mut ctx, "Hi", 10.0, 20.0);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], DrawCommand::FillPath { .. }), "scaled text must fall back to FillPath, not silently drop the scale");
    }

    /// Shadow pre-pass still emits (as `GlyphRun` now, matching the
    /// main draw) BEFORE the main glyph run — same ordering the old
    /// FillPath shadow pre-pass used.
    #[test]
    fn fill_text_with_shadow_emits_shadow_then_main_glyph_run() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(200, 100);
        ctx.set_fill_color("#ffffff");
        Effects::set_shadow(&mut ctx, 2.0, 2.0, 0.0, "#000000");
        TextRenderer::set_font(&mut ctx, "16px Roboto");
        TextRenderer::fill_text(&mut ctx, "Hi", 10.0, 20.0);
        let scene = ctx.take_scene();
        assert_eq!(scene.commands.len(), 2, "shadow pre-pass + main draw");
        assert!(matches!(scene.commands[0], DrawCommand::GlyphRun { .. }));
        assert!(matches!(scene.commands[1], DrawCommand::GlyphRun { .. }));
    }

    #[test]
    fn empty_text_emits_nothing() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(200, 100);
        TextRenderer::set_font(&mut ctx, "16px Roboto");
        TextRenderer::fill_text(&mut ctx, "", 10.0, 20.0);
        let scene = ctx.take_scene();
        assert!(scene.commands.is_empty());
    }

    // -----------------------------------------------------------------
    // Transform-composition regression (canvas-style incremental CTM)
    // -----------------------------------------------------------------
    //
    // Root-caused defect: `translate`/`rotate`/`scale` used to compose via
    // kurbo's `then_*` family (`Op * self` — the operation applied in the
    // OUTER/world frame, after everything already accumulated). A
    // `translate` followed by a `rotate` on a non-identity transform must
    // instead compose LOCAL-frame (`self * Op`, kurbo's `pre_*` family) —
    // exactly the semantics `uzor-render-tiny-skia`'s reference-correct
    // `Painter::translate`/`rotate`/`scale` already use. This test pins
    // the exact expected device-space mapping of a known local point so a
    // regression to `then_*` fails immediately, with no rendering
    // required.
    #[test]
    fn translate_then_rotate_matches_local_frame_composition() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);

        // translate(10, 0) then rotate(90deg) — a point drawn locally at
        // (5, 0) after these two calls must land at device (10, 5):
        // rotate(90deg) first turns local (5,0) into (0,5) in the frame
        // `translate` already established, THEN that frame's own (10, 0)
        // offset is added.
        Painter::translate(&mut ctx, 10.0, 0.0);
        Painter::rotate(&mut ctx, std::f64::consts::FRAC_PI_2);

        let p = ctx.transform * KPoint::new(5.0, 0.0);
        assert!((p.x - 10.0).abs() < 1e-9, "x mismatch: got {p:?}");
        assert!((p.y - 5.0).abs() < 1e-9, "y mismatch: got {p:?}");

        // A `then_*`-composed (world-frame) regression would instead
        // rotate the ALREADY-translated point about the origin, landing
        // at device (0, 10) — pinning the wrong-answer shape too so a
        // silent revert is unambiguous, not just "some other number."
        assert!(
            (p.x - 0.0).abs() > 1.0 || (p.y - 10.0).abs() > 1.0,
            "result matches the WRONG (then_*, world-frame) composition"
        );
    }

    #[test]
    fn translate_then_scale_matches_local_frame_composition() {
        let mut ctx = UrxRenderContext::new(1.0);
        ctx.begin_frame(100, 100);

        // Matches Canvas2D semantics: local (0,0) -> device (10,20);
        // local (5,5) -> device (10 + 2*5, 20 + 3*5) = (20, 35).
        Painter::translate(&mut ctx, 10.0, 20.0);
        Painter::scale(&mut ctx, 2.0, 3.0);

        let origin = ctx.transform * KPoint::new(0.0, 0.0);
        assert!((origin.x - 10.0).abs() < 1e-9 && (origin.y - 20.0).abs() < 1e-9);

        let p = ctx.transform * KPoint::new(5.0, 5.0);
        assert!((p.x - 20.0).abs() < 1e-9 && (p.y - 35.0).abs() < 1e-9, "got {p:?}");
    }
}
