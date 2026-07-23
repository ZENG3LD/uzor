//! `Scene` → native-pipeline instance encoder.
//!
//! Commit 1 covered `FillRect`/`StrokeRect` (`Brush::Solid` — gradient/
//! image degrade to a first-stop/transparent solid, counted). Commit 2
//! added `Line` and the painter's-order batching machinery (`Batch`/
//! `BatchKind`) that keeps Quad/Line draws interleaved correctly.
//! Commit 3 adds `FillPath`/`StrokePath` (lyon tessellation via
//! `TessCache`) and routes `FillRect{Brush::Gradient(Linear), radii}`
//! through the same tessellated-triangle path with real per-vertex
//! gradient colour (design §5) — the ONLY `FillRect` gradient kind
//! that gets full support in Wave 1; `Radial`/`Sweep` still degrade to
//! solid, and gradients on `Line`/`FillPath`/`StrokePath` still
//! degrade to first-stop solid (design: "only FillRect gets gradients
//! in Wave 1"). Every unimplemented `DrawCommand` variant is a
//! counted, never-silent degrade via
//! `uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES` with a
//! `native_*` `kind` tag.
//!
//! ## Tessellation-cache transform semantics — honest write-up (design
//! item 3 investigation)
//!
//! `uzor-urx-cpu::path::stroke_path_aa` transforms the PATH POINTS
//! through the full affine (translation/scale/rotation/shear all
//! honoured, `path.rs:261,272,277`) but then strokes with
//! `stroke.width` **used exactly as given, never scaled by the
//! transform** (`path.rs:299` `let half: f64 = stroke.width as f64 *
//! 0.5;` — no `sx`/`sy` factor anywhere in that function). CPU's
//! stroke width is therefore a DEVICE-space constant, independent of
//! the transform's scale.
//!
//! This crate's `TessCache` (design §6) tessellates in LOCAL
//! (pre-transform) space — `stroke.width` is fed to lyon as a LOCAL
//! geometric quantity (`tessellate.rs::tessellate_stroke`), and the
//! resulting mesh (including that baked-in width) is re-projected
//! through the frame's translate+scale at replay (`emit_solid_mesh`
//! below). Under a non-1.0 scale this means the NATIVE pipeline's
//! apparent on-screen stroke width SCALES with the transform, while
//! CPU's stays constant — a genuine, structural semantic difference
//! between the two backends for scaled `StrokePath` content. Every
//! Wave 1 fixture uses `Affine::IDENTITY` (scale 1.0), so this
//! difference is inert today; flagged here for the record rather than
//! papered over, per the coordinator's explicit ask. The same applies
//! to curve-flattening tolerance: CPU adapts its flatten tolerance to
//! the transform's scale (`path.rs::screen_flatten_tolerance`) so a
//! zoomed-in curve stays smooth; this crate tessellates once at a
//! fixed LOCAL tolerance (`tessellate::TESS_TOLERANCE_PX`), so a
//! heavily-scaled-up cached mesh could look faceted on screen — again
//! a real, currently-untested (identity-transform-only fixtures)
//! architecture limitation of the local-space caching design, not
//! something this commit invents.
//!
//! ## Gradient-routing finding (design item 5 investigation)
//!
//! Direct read of `uzor-urx-cpu::backend.rs`'s `FillRect` arm shows
//! that CPU's `Brush::Gradient` branch calls
//! `gradient::try_fill_rect_gradient` → `fill_rect_gradient_aa`, which
//! iterates pixels bounded ONLY by `clip.current()` (the clip stack's
//! bounding-box intersection) and never calls
//! `clip.pixel_coverage()` — i.e. it never consults the per-pixel
//! rounded-clip MASK that `radii: Some(...)` pushes for the SOLID fill
//! path (`fill_rect_aa`'s `use_mask` branch). **CPU silently ignores
//! `radii` for gradient fills** — a genuine, pre-existing gap (out of
//! scope to fix; `uzor-urx-cpu` is off-limits) that only affects the
//! gradient code path (solid `FillRect`+radii still rounds correctly
//! on CPU). This native pipeline renders the gradient through a REAL
//! `RoundedRect` tessellation (correctly rounded corners), so
//! `linear_gradient_rect`'s corner regions are an EXPECTED, honestly-
//! documented mismatch between the two backends — sized (fixture rect/
//! radius choice) to stay inside the design §7 tolerance budget, same
//! engineering call as `solid_rects_with_radii` in Commit 1.
//!
//! Separately: CPU's `fill_rect_gradient_aa` uses the gradient axis
//! (`LinearGradientPosition.start/end`) DIRECTLY against screen-space
//! pixel coordinates — it never applies `transform` to the gradient
//! axis at all (only to `rect`). This native pipeline applies the SAME
//! translate+scale decomposition to both the rect's tessellated mesh
//! AND the gradient axis, which is more correct under a future non-
//! identity transform but is, again, a difference from CPU's literal
//! behaviour — invisible at `Affine::IDENTITY` (every Wave 1 fixture).
//!
//! Stop interpolation: CPU builds a dense 256-entry LUT by lerping
//! PREMULTIPLIED sRGB stop bytes per-pixel
//! (`uzor-urx-cpu/src/gradient.rs`'s module doc: "interpolate in
//! linear-premul space"). This pipeline evaluates STRAIGHT
//! (non-premultiplied) colour at each tessellation VERTEX only and
//! lets the rasteriser's barycentric interpolation carry it across
//! each triangle, premultiplying once in the fragment shader — the
//! same "straight-in, premultiply-in-shader" convention Quad/Line
//! already use. Straight-lerp and premultiplied-lerp are
//! mathematically IDENTICAL when alpha is constant across the
//! gradient (premultiplied = straight × a constant, and lerp commutes
//! with constant scaling) — `linear_gradient_rect`'s fixture therefore
//! uses two fully-opaque stops, sidestepping this real algorithmic
//! difference cleanly rather than hoping the tolerance budget absorbs
//! it. A future gradient with varying alpha would need this
//! documented and probed more carefully (design risk 5 already
//! anticipates exactly this).

use kurbo::Shape as _;
use peniko::LinearGradientPosition;
use uzor_urx_core::math::{
    Affine, BezPath, Brush, Color, ColorStops, Extend, Gradient, GradientKind, Rect, RoundedRect,
    RoundedRectRadii, Vec2,
};
use uzor_urx_core::metrics_keys::{KEY_RENDER_PRIMITIVES, KEY_RENDER_SKIPPED_NONFINITE};
use uzor_urx_core::scene::{DrawCommand, FillRule, LineCap, Scene, Stroke};
use uzor_urx_core::validate::{validate_command, ValidationIssue};

use crate::pipelines::line::LineInstance;
use crate::pipelines::path::TriInstance;
use crate::pipelines::quad::{pack_rgba8, QuadInstance};
use crate::renderer::Viewport;
use crate::tessellate::TessCache;

/// Which pipeline a [`Batch`] replays through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchKind {
    Quad,
    Line,
    Triangle,
}

/// A contiguous run of same-kind instances in painter's-order scan
/// order — `start`/`count` index into `EncodedFrame::{quads,lines,
/// triangles}` depending on `kind`. Copied algorithm from legacy
/// `InstancedRenderer::render`'s Phase-1 scan
/// (`uzor-render-wgpu-instanced/src/renderer.rs:766-848`, design §5):
/// a new batch opens only when the kind differs from the previous
/// one, so consecutive same-kind commands coalesce into one draw call
/// while painter's order (replay in scan order, never reordered) is
/// preserved across a Quad/Line/Triangle interleave.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Batch {
    pub(crate) kind: BatchKind,
    pub(crate) start: u32,
    pub(crate) count: u32,
}

/// Painter's-order encoded frame — the Scene walker's output, ready
/// for `NativeUrxRenderer` to upload + replay via `batches`.
#[derive(Debug, Default)]
pub(crate) struct EncodedFrame {
    pub(crate) quads: Vec<QuadInstance>,
    pub(crate) lines: Vec<LineInstance>,
    pub(crate) triangles: Vec<TriInstance>,
    pub(crate) batches: Vec<Batch>,
}

impl EncodedFrame {
    fn push_quad(&mut self, instance: QuadInstance) {
        let start = self.quads.len() as u32;
        self.quads.push(instance);
        self.bump_batch(BatchKind::Quad, start);
    }

    fn push_line(&mut self, instance: LineInstance) {
        let start = self.lines.len() as u32;
        self.lines.push(instance);
        self.bump_batch(BatchKind::Line, start);
    }

    fn push_triangle(&mut self, instance: TriInstance) {
        let start = self.triangles.len() as u32;
        self.triangles.push(instance);
        self.bump_batch(BatchKind::Triangle, start);
    }

    /// Extend the current batch if its kind matches, else open a new
    /// one — the exact coalescing rule design §5 specifies.
    fn bump_batch(&mut self, kind: BatchKind, start: u32) {
        if let Some(last) = self.batches.last_mut() {
            if last.kind == kind {
                last.count += 1;
                return;
            }
        }
        self.batches.push(Batch { kind, start, count: 1 });
    }
}

/// Translate + uniform-scale decomposition (design §5) — the native
/// path drops shear/rotation (`c[1]`/`c[2]`) for Wave 1, matching the
/// legacy adapter's own known Wave-4 gap (`adapter.rs:311-323`). A
/// sheared `FillRect`/`Line` would need the (not-yet-built) path
/// pipeline to render correctly — Wave 4's documented "detect +
/// reroute" extension, not invented here.
fn decompose_translate_scale(t: &Affine) -> (f64, f64, f64, f64) {
    let c = t.as_coeffs();
    (c[0], c[3], c[4], c[5])
}

/// Transform `rect` by the translate+scale decomposition of `t`,
/// normalizing for a possible negative scale. Returns
/// `(x0, y0, w, h, |sx|, |sy|)` in device space.
fn transform_rect(rect: Rect, t: &Affine) -> (f64, f64, f64, f64, f64, f64) {
    let (sx, sy, tx, ty) = decompose_translate_scale(t);
    let x0 = rect.x0 * sx + tx;
    let y0 = rect.y0 * sy + ty;
    let x1 = rect.x1 * sx + tx;
    let y1 = rect.y1 * sy + ty;
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    (x0, y0, x1 - x0, y1 - y0, sx.abs(), sy.abs())
}

/// Project a LOCAL-space point through the translate+scale
/// decomposition — the same operation `tessellate.rs`'s cached meshes
/// need applied at every replay.
#[inline]
fn project_local(p: [f32; 2], sx: f64, sy: f64, tx: f64, ty: f64) -> [f32; 2] {
    [(p[0] as f64 * sx + tx) as f32, (p[1] as f64 * sy + ty) as f32]
}

#[inline]
fn degrade(kind: &'static str) {
    metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => kind).increment(1);
}

/// Which case `resolve_brush_color` hit — drives which `native_*`
/// counter (if any) the caller emits. Kept distinct from the color
/// itself so callers can label gradient-vs-image degrades honestly
/// instead of collapsing both into one "brush wasn't solid" counter.
enum BrushKind {
    Solid,
    Gradient,
    Image,
}

/// Resolve any `Brush` to a single representative colour: the brush's
/// own colour for `Solid`, the first gradient stop for `Gradient`
/// (matches the legacy adapter's `brush_to_solid_color`,
/// `adapter.rs:290-298`), fully transparent for `Image` (no image
/// atlas wired yet).
fn resolve_brush_color(brush: &Brush) -> (Color, BrushKind) {
    match brush {
        Brush::Solid(c) => (*c, BrushKind::Solid),
        Brush::Gradient(g) => {
            let c = g
                .stops
                .first()
                .map(|s| s.color.to_alpha_color::<peniko::color::Srgb>())
                .unwrap_or(Color::from_rgba8(0, 0, 0, 0));
            (c, BrushKind::Gradient)
        }
        Brush::Image(_) => (Color::from_rgba8(0, 0, 0, 0), BrushKind::Image),
    }
}

#[inline]
fn packed_color(c: Color) -> u32 {
    let p = c.to_rgba8();
    pack_rgba8([p.r, p.g, p.b, p.a])
}

/// Single radius for the SDF (Wave 1 approximation for the Quad
/// pipeline — per-corner radii land in Wave 4 there). Emits
/// `native_per_corner_radii_uniform_approx` when the four radii
/// actually differ, so the approximation is never silent. NOT used by
/// the gradient-routed path below — that path tessellates a REAL
/// `RoundedRect` and supports true per-corner radii for free.
fn corner_radius_uniform(radii: &Option<[f32; 4]>) -> f32 {
    match radii {
        None => 0.0,
        Some(r) => {
            if r.iter().any(|v| (*v - r[0]).abs() > 0.01) {
                degrade("native_per_corner_radii_uniform_approx");
            }
            r[0].max(0.0)
        }
    }
}

pub(crate) fn encode_scene(scene: &Scene, viewport: Viewport, tess_cache: &mut TessCache) -> EncodedFrame {
    let mut frame = EncodedFrame::default();
    // Wave 1 has no native clip stack yet (commit 4) — every instance
    // gets the full viewport as its clip rect.
    let full_clip = [0.0f32, 0.0, viewport.width as f32, viewport.height as f32];

    for cmd in &scene.commands {
        // Same "silent skip + counter" policy as `uzor-urx-cpu`
        // (`uzor-urx-cpu/src/backend.rs:137-140`) — degenerate geometry
        // passes through to the per-primitive code below, which already
        // rejects zero-area rects via its own `w <= 0.0 || h <= 0.0` guard.
        if let Err(ValidationIssue::NonFinite) = validate_command(cmd) {
            metrics::counter!(KEY_RENDER_SKIPPED_NONFINITE).increment(1);
            continue;
        }

        match cmd {
            DrawCommand::FillRect { rect, radii, brush, transform } => {
                encode_fill_rect(&mut frame, tess_cache, *rect, radii, brush, transform, full_clip);
            }
            DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                encode_stroke_rect(&mut frame, *rect, radii, stroke, brush, transform, full_clip);
            }
            DrawCommand::Line { from, to, stroke, brush, transform } => {
                encode_line(&mut frame, *from, *to, stroke, brush, transform, full_clip);
            }
            DrawCommand::FillPath { path, rule, brush, transform } => {
                encode_fill_path(&mut frame, tess_cache, path, *rule, brush, transform, full_clip);
            }
            DrawCommand::StrokePath { path, stroke, brush, transform } => {
                encode_stroke_path(&mut frame, tess_cache, path, stroke, brush, transform, full_clip);
            }
            DrawCommand::GlyphRun { .. } => degrade("native_glyph_run_not_yet_implemented"),
            DrawCommand::Image { .. } => degrade("native_image_not_yet_implemented"),
            DrawCommand::PushClipRect { .. } => degrade("native_clip_not_yet_implemented"),
            DrawCommand::PushClipRoundedRect { .. } => degrade("native_clip_not_yet_implemented"),
            // Nothing was ever pushed onto a (non-existent) native clip
            // stack this commit, so there is nothing to balance/pop —
            // the corresponding push already counted its own degrade.
            DrawCommand::PopClip => {}
            DrawCommand::PushBlendLayer { .. } | DrawCommand::PopBlendLayer => {
                degrade("native_blend_layer_not_yet_implemented");
            }
        }
    }

    frame
}

fn encode_fill_rect(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    match brush {
        Brush::Solid(c) => encode_fill_rect_quad(frame, rect, radii, *c, transform, clip_rect),
        Brush::Gradient(g) => match &g.kind {
            GradientKind::Linear(pos) => {
                encode_fill_rect_linear_gradient(frame, tess_cache, rect, radii, g, pos, transform, clip_rect);
            }
            GradientKind::Radial(_) | GradientKind::Sweep(_) => {
                // Wave 1 scope: only Linear gets full gradient support
                // on FillRect (design §5). Radial/Sweep degrade to
                // first-stop solid, same policy as the legacy adapter.
                degrade("native_fill_rect_radial_or_sweep_to_solid");
                let color = resolve_brush_color(brush).0;
                encode_fill_rect_quad(frame, rect, radii, color, transform, clip_rect);
            }
        },
        Brush::Image(_) => {
            // No image atlas wired yet — dropped entirely, matching the
            // legacy adapter's own `wgpu_fill_rect_image_dropped` policy
            // (`adapter.rs:109-115`).
            degrade("native_fill_rect_image_dropped");
        }
    }
}

fn encode_fill_rect_quad(
    frame: &mut EncodedFrame,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    color: Color,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let (x0, y0, w, h, sx, sy) = transform_rect(rect, transform);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let scale = ((sx + sy) * 0.5) as f32;
    let corner_radius = corner_radius_uniform(radii) * scale;

    frame.push_quad(QuadInstance {
        pos: [x0 as f32, y0 as f32],
        size: [w as f32, h as f32],
        color: packed_color(color),
        border_color: 0,
        corner_radius,
        border_width: 0.0,
        _pad0: [0.0; 2],
        clip_rect,
    });
}

/// Build a LOCAL-space rect `BezPath` — a real `RoundedRect` when
/// `radii` carries any positive value (exact per-corner radii, no
/// uniform-radius approximation needed since this path tessellates
/// the actual shape rather than evaluating an SDF), a plain 4-corner
/// rect otherwise. Mirrors the RECIPE `uzor-urx-cpu::backend.rs`'s
/// `StrokeRect`+radii arm uses (`RoundedRect::from_rect` +
/// `.into_path(0.25)`), NOT its `FillRect`+`Gradient`+radii behaviour
/// (which, per this module's doc comment, doesn't actually consult the
/// rounded mask it pushes).
fn rect_bez_path(rect: Rect, radii: &Option<[f32; 4]>) -> BezPath {
    if let Some(r) = radii {
        if r.iter().any(|v| *v > 0.0) {
            let rr = RoundedRect::from_rect(
                rect,
                RoundedRectRadii::new(
                    r[0].max(0.0) as f64,
                    r[1].max(0.0) as f64,
                    r[2].max(0.0) as f64,
                    r[3].max(0.0) as f64,
                ),
            );
            return rr.into_path(crate::tessellate::TESS_TOLERANCE_PX);
        }
    }
    let mut path = BezPath::new();
    path.move_to((rect.x0, rect.y0));
    path.line_to((rect.x1, rect.y0));
    path.line_to((rect.x1, rect.y1));
    path.line_to((rect.x0, rect.y1));
    path.close_path();
    path
}

/// Fold a raw gradient parameter `t` into `[0, 1]` per the spread mode
/// — copied formula-for-formula from `uzor-urx-cpu::gradient::apply_spread`
/// so a future non-`Pad` fixture still matches CPU's folding exactly.
fn apply_spread(t: f32, mode: Extend) -> f32 {
    match mode {
        Extend::Pad => t.clamp(0.0, 1.0),
        Extend::Repeat => {
            let f = t - t.floor();
            if f < 0.0 {
                f + 1.0
            } else {
                f
            }
        }
        Extend::Reflect => {
            let m = (t.rem_euclid(2.0) - 1.0).abs();
            1.0 - m
        }
    }
}

/// Straight (non-premultiplied) sRGB byte quad for a stop — same
/// conversion `uzor-urx-cpu::gradient::stop_rgba8` uses, minus the
/// premultiply step (this crate premultiplies once in the fragment
/// shader; see this module's doc comment).
fn stop_rgba8(stop: &peniko::ColorStop) -> [u8; 4] {
    let p = stop.color.to_alpha_color::<peniko::color::Srgb>().to_rgba8();
    [p.r, p.g, p.b, p.a]
}

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let r = (a as f32) * (1.0 - t) + (b as f32) * t;
    r.round().clamp(0.0, 255.0) as u8
}

/// Sample a stop sequence at parameter `t` — same bracket-search +
/// lerp shape as `uzor-urx-cpu::gradient::sample_stops`, but returns a
/// STRAIGHT (non-premultiplied) colour (see this module's doc comment
/// for why that's the right choice for this pipeline's convention).
fn sample_gradient_straight(stops: &ColorStops, t: f32) -> Color {
    if stops.is_empty() {
        return Color::from_rgba8(0, 0, 0, 0);
    }
    let last = stops.len() - 1;
    if stops.len() == 1 || t <= stops[0].offset {
        let [r, g, b, a] = stop_rgba8(&stops[0]);
        return Color::from_rgba8(r, g, b, a);
    }
    if t >= stops[last].offset {
        let [r, g, b, a] = stop_rgba8(&stops[last]);
        return Color::from_rgba8(r, g, b, a);
    }
    for w in stops.windows(2) {
        let s0 = &w[0];
        let s1 = &w[1];
        if t >= s0.offset && t <= s1.offset {
            let span = s1.offset - s0.offset;
            let local = if span.abs() < 1e-9 { 0.0 } else { (t - s0.offset) / span };
            let c0 = stop_rgba8(s0);
            let c1 = stop_rgba8(s1);
            return Color::from_rgba8(
                lerp_u8(c0[0], c1[0], local),
                lerp_u8(c0[1], c1[1], local),
                lerp_u8(c0[2], c1[2], local),
                lerp_u8(c0[3], c1[3], local),
            );
        }
    }
    let [r, g, b, a] = stop_rgba8(&stops[last]);
    Color::from_rgba8(r, g, b, a)
}

/// Evaluate the gradient colour at a LOCAL-space mesh vertex by
/// projecting it onto the gradient axis (design §5).
fn gradient_vertex_color(
    p: [f32; 2],
    pos: &LinearGradientPosition,
    axis: (f64, f64),
    axis_len_sq: f64,
    stops: &ColorStops,
    extend: Extend,
) -> Color {
    let t_raw = if axis_len_sq < 1e-9 {
        0.0
    } else {
        ((p[0] as f64 - pos.start.x) * axis.0 + (p[1] as f64 - pos.start.y) * axis.1) / axis_len_sq
    };
    let t = apply_spread(t_raw as f32, extend);
    sample_gradient_straight(stops, t)
}

/// `FillRect{Brush::Gradient(Linear), radii}` — routed through the
/// Path pipeline (design §5). Tessellates a REAL `RoundedRect` (or
/// plain rect) path, colours each output triangle's 3 vertices by
/// projecting them onto the gradient axis and sampling the stop list.
fn encode_fill_rect_linear_gradient(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    gradient: &Gradient,
    pos: &LinearGradientPosition,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let path = rect_bez_path(rect, radii);
    let mesh = tess_cache.get_or_insert_fill(&path, FillRule::NonZero);

    let (sx, sy, tx, ty) = decompose_translate_scale(transform);
    let axis = (pos.end.x - pos.start.x, pos.end.y - pos.start.y);
    let axis_len_sq = axis.0 * axis.0 + axis.1 * axis.1;

    for tri in &mesh.triangles {
        let c0 = gradient_vertex_color(tri[0], pos, axis, axis_len_sq, &gradient.stops, gradient.extend);
        let c1 = gradient_vertex_color(tri[1], pos, axis, axis_len_sq, &gradient.stops, gradient.extend);
        let c2 = gradient_vertex_color(tri[2], pos, axis, axis_len_sq, &gradient.stops, gradient.extend);
        frame.push_triangle(TriInstance {
            v0: project_local(tri[0], sx, sy, tx, ty),
            v1: project_local(tri[1], sx, sy, tx, ty),
            v2: project_local(tri[2], sx, sy, tx, ty),
            color0: packed_color(c0),
            color1: packed_color(c1),
            color2: packed_color(c2),
            _pad0: 0.0,
            clip_rect,
        });
    }
}

fn encode_stroke_rect(
    frame: &mut EncodedFrame,
    rect: Rect,
    radii: &Option<[f32; 4]>,
    stroke: &Stroke,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let (color, kind) = resolve_brush_color(brush);
    match kind {
        BrushKind::Gradient => degrade("native_strokerect_gradient_to_solid"),
        BrushKind::Image => degrade("native_strokerect_image_to_solid"),
        BrushKind::Solid => {}
    }

    let (x0, y0, w, h, sx, sy) = transform_rect(rect, transform);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let scale = ((sx + sy) * 0.5) as f32;
    let corner_radius = corner_radius_uniform(radii) * scale;
    let border_width = (stroke.width * scale).max(0.0);
    if border_width <= 0.0 {
        return;
    }

    frame.push_quad(QuadInstance {
        pos: [x0 as f32, y0 as f32],
        size: [w as f32, h as f32],
        // Fully transparent fill — a StrokeRect draws only the border
        // band; see the centered-border fragment-shader formula.
        color: 0,
        border_color: packed_color(color),
        corner_radius,
        border_width,
        _pad0: [0.0; 2],
        clip_rect,
    });
}

/// `Line` → `LineInstance`. Cap mapping (design §3):
/// `LineCap::Round` → `cap_flags 0` (round-round — the Scene IR's
/// `Stroke.cap` is one style for the whole segment, not per-endpoint,
/// so there's no finer round/butt split to preserve).
/// `LineCap::Butt` → `cap_flags 3` (butt-both).
/// `LineCap::Square` is a pre-existing gap shared with `uzor-urx-cpu`
/// (neither reference implementation renders square caps today) —
/// degrades to Round, counted `native_line_square_cap_to_round`.
fn encode_line(
    frame: &mut EncodedFrame,
    from: Vec2,
    to: Vec2,
    stroke: &Stroke,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let (color, kind) = resolve_brush_color(brush);
    match kind {
        BrushKind::Gradient => degrade("native_line_gradient_to_solid"),
        BrushKind::Image => degrade("native_line_image_to_solid"),
        BrushKind::Solid => {}
    }

    let (sx, sy, tx, ty) = decompose_translate_scale(transform);
    let start = [(from.x * sx + tx) as f32, (from.y * sy + ty) as f32];
    let end = [(to.x * sx + tx) as f32, (to.y * sy + ty) as f32];
    let scale = ((sx.abs() + sy.abs()) * 0.5) as f32;
    let width = (stroke.width * scale).max(0.0);
    if width <= 0.0 {
        return;
    }

    let cap_flags = match stroke.cap {
        LineCap::Round => 0.0,
        LineCap::Butt => 3.0,
        LineCap::Square => {
            degrade("native_line_square_cap_to_round");
            0.0
        }
    };

    frame.push_line(LineInstance {
        start,
        end,
        color: packed_color(color),
        width,
        cap_flags,
        _pad0: 0.0,
        _pad1: [0.0; 2],
        clip_rect,
    });
}

/// Emit every triangle of a (already tessellated, LOCAL-space) mesh as
/// a flat-shaded `TriInstance` (`color0 == color1 == color2`),
/// re-projected through the frame's translate+scale.
fn emit_solid_mesh(
    frame: &mut EncodedFrame,
    mesh: &crate::tessellate::TessMesh,
    transform: &Affine,
    packed: u32,
    clip_rect: [f32; 4],
) {
    let (sx, sy, tx, ty) = decompose_translate_scale(transform);
    for tri in &mesh.triangles {
        frame.push_triangle(TriInstance {
            v0: project_local(tri[0], sx, sy, tx, ty),
            v1: project_local(tri[1], sx, sy, tx, ty),
            v2: project_local(tri[2], sx, sy, tx, ty),
            color0: packed,
            color1: packed,
            color2: packed,
            _pad0: 0.0,
            clip_rect,
        });
    }
}

fn encode_fill_path(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    path: &BezPath,
    rule: FillRule,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let (color, kind) = resolve_brush_color(brush);
    match kind {
        BrushKind::Gradient => degrade("native_fillpath_gradient_to_solid"),
        BrushKind::Image => degrade("native_fillpath_image_to_solid"),
        BrushKind::Solid => {}
    }
    let mesh = tess_cache.get_or_insert_fill(path, rule);
    emit_solid_mesh(frame, &mesh, transform, packed_color(color), clip_rect);
}

fn encode_stroke_path(
    frame: &mut EncodedFrame,
    tess_cache: &mut TessCache,
    path: &BezPath,
    stroke: &Stroke,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    if stroke.width <= 0.0 {
        return;
    }
    let (color, kind) = resolve_brush_color(brush);
    match kind {
        BrushKind::Gradient => degrade("native_strokepath_gradient_to_solid"),
        BrushKind::Image => degrade("native_strokepath_image_to_solid"),
        BrushKind::Solid => {}
    }
    let mesh = tess_cache.get_or_insert_stroke(path, stroke);
    emit_solid_mesh(frame, &mesh, transform, packed_color(color), clip_rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_urx_core::scene::Stroke as SceneStroke;

    fn viewport() -> Viewport {
        Viewport { width: 100, height: 100 }
    }

    fn cache() -> TessCache {
        TessCache::new()
    }

    #[test]
    fn fill_rect_solid_emits_one_quad() {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(10.0, 10.0, 50.0, 50.0), Color::from_rgba8(255, 0, 0, 255));
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.quads.len(), 1);
        assert_eq!(frame.quads[0].pos, [10.0, 10.0]);
        assert_eq!(frame.quads[0].size, [40.0, 40.0]);
        assert_eq!(frame.quads[0].border_width, 0.0);
    }

    #[test]
    fn fill_rect_with_uniform_radii_sets_corner_radius_without_degrade() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: Some([8.0, 8.0, 8.0, 8.0]),
            brush: Brush::Solid(Color::from_rgba8(0, 255, 0, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.quads.len(), 1);
        assert!((frame.quads[0].corner_radius - 8.0).abs() < 0.01);
    }

    #[test]
    fn stroke_rect_emits_transparent_fill_with_border() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokeRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            stroke: SceneStroke { width: 3.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.quads.len(), 1);
        assert_eq!(frame.quads[0].color, 0);
        assert!((frame.quads[0].border_width - 3.0).abs() < 0.01);
        assert_ne!(frame.quads[0].border_color, 0);
    }

    #[test]
    fn zero_width_stroke_emits_nothing() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokeRect {
            rect: Rect::new(0.0, 0.0, 40.0, 40.0),
            radii: None,
            stroke: SceneStroke { width: 0.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert!(frame.quads.is_empty());
    }

    #[test]
    fn degenerate_rect_is_skipped_not_panicked() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 10.0, 10.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert!(frame.quads.is_empty());
    }

    #[test]
    fn line_solid_emits_one_line_not_a_quad() {
        let mut scene = Scene::new();
        scene.line_solid(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 10.0, y: 10.0 },
            2.0,
            Color::from_rgba8(255, 255, 255, 255),
        );
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert!(frame.quads.is_empty());
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(frame.lines[0].start, [0.0, 0.0]);
        assert_eq!(frame.lines[0].end, [10.0, 10.0]);
        assert!((frame.lines[0].width - 2.0).abs() < 0.01);
    }

    #[test]
    fn line_width_scales_with_transform() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 4.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::scale(2.0),
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(frame.lines[0].end, [20.0, 0.0]);
        assert!((frame.lines[0].width - 8.0).abs() < 0.01, "width should scale 2x with the transform");
    }

    #[test]
    fn zero_width_line_emits_nothing() {
        let mut scene = Scene::new();
        scene.line_solid(
            Vec2 { x: 0.0, y: 0.0 },
            Vec2 { x: 10.0, y: 10.0 },
            0.0,
            Color::from_rgba8(255, 255, 255, 255),
        );
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert!(frame.lines.is_empty());
    }

    #[test]
    fn cap_round_maps_to_flag_zero() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 2.0, cap: LineCap::Round, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.lines[0].cap_flags, 0.0);
    }

    #[test]
    fn cap_butt_maps_to_flag_three() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 2.0, cap: LineCap::Butt, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.lines[0].cap_flags, 3.0);
    }

    #[test]
    fn cap_square_degrades_to_round_flag_zero() {
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 0.0, y: 0.0 },
            to: Vec2 { x: 10.0, y: 0.0 },
            stroke: SceneStroke { width: 2.0, cap: LineCap::Square, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(255, 255, 255, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        // Degrades to round (flag 0) — counted via `native_line_square_cap_to_round`
        // (asserting on the global metrics recorder isn't wired in this
        // crate's unit tests; the geometry-level effect is what we can verify).
        assert_eq!(frame.lines[0].cap_flags, 0.0);
    }

    #[test]
    fn batches_coalesce_by_kind_and_preserve_painters_order() {
        // rect, rect, line, rect -> 3 batches: [Quad x2, Line x1, Quad x1].
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.fill_rect_solid(Rect::new(20.0, 0.0, 30.0, 10.0), Color::from_rgba8(0, 255, 0, 255));
        scene.line_solid(
            Vec2 { x: 0.0, y: 20.0 },
            Vec2 { x: 30.0, y: 20.0 },
            2.0,
            Color::from_rgba8(0, 0, 255, 255),
        );
        scene.fill_rect_solid(Rect::new(0.0, 30.0, 10.0, 40.0), Color::from_rgba8(255, 255, 0, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.quads.len(), 3);
        assert_eq!(frame.lines.len(), 1);
        assert_eq!(frame.batches.len(), 3, "batches: {:?}", frame.batches);

        assert_eq!(frame.batches[0].kind, BatchKind::Quad);
        assert_eq!(frame.batches[0].start, 0);
        assert_eq!(frame.batches[0].count, 2);

        assert_eq!(frame.batches[1].kind, BatchKind::Line);
        assert_eq!(frame.batches[1].start, 0);
        assert_eq!(frame.batches[1].count, 1);

        assert_eq!(frame.batches[2].kind, BatchKind::Quad);
        assert_eq!(frame.batches[2].start, 2, "third batch must resume at quad index 2, not restart at 0");
        assert_eq!(frame.batches[2].count, 1);
    }

    #[test]
    fn alternating_kinds_never_coalesce_across_a_gap() {
        // rect, line, rect, line -> 4 distinct batches, never merged.
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 10.0, y: 0.0 }, 2.0, Color::from_rgba8(0, 0, 255, 255));
        scene.fill_rect_solid(Rect::new(0.0, 20.0, 10.0, 30.0), Color::from_rgba8(0, 255, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 40.0 }, Vec2 { x: 10.0, y: 40.0 }, 2.0, Color::from_rgba8(255, 255, 0, 255));

        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.batches.len(), 4);
        let kinds: Vec<BatchKind> = frame.batches.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BatchKind::Quad, BatchKind::Line, BatchKind::Quad, BatchKind::Line]);
    }

    #[test]
    fn fill_path_triangle_emits_triangles_and_batches() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));
        path.line_to((10.0, 20.0));
        path.close_path();

        let mut scene = Scene::new();
        scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert!(!frame.triangles.is_empty());
        assert_eq!(frame.batches.len(), 1);
        assert_eq!(frame.batches[0].kind, BatchKind::Triangle);
        for tri in &frame.triangles {
            assert_eq!(tri.color0, tri.color1);
            assert_eq!(tri.color1, tri.color2);
        }
    }

    #[test]
    fn stroke_path_zero_width_emits_nothing() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));

        let mut scene = Scene::new();
        scene.push(DrawCommand::StrokePath {
            path,
            stroke: SceneStroke { width: 0.0, ..SceneStroke::default() },
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });
        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert!(frame.triangles.is_empty());
    }

    #[test]
    fn quad_line_triangle_batches_interleave_correctly() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.line_to((20.0, 0.0));
        path.line_to((10.0, 20.0));
        path.close_path();

        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(0.0, 0.0, 10.0, 10.0), Color::from_rgba8(255, 0, 0, 255));
        scene.line_solid(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 10.0, y: 0.0 }, 2.0, Color::from_rgba8(0, 0, 255, 255));
        scene.push(DrawCommand::FillPath {
            path,
            rule: FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(10, 20, 30, 255)),
            transform: Affine::IDENTITY,
        });

        let frame = encode_scene(&scene, viewport(), &mut cache());
        assert_eq!(frame.batches.len(), 3);
        let kinds: Vec<BatchKind> = frame.batches.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BatchKind::Quad, BatchKind::Line, BatchKind::Triangle]);
    }

    #[test]
    fn gradient_vertex_projection_matches_analytic_t() {
        // Axis from (0,0) to (10,0); a vertex at x=5 should project to t=0.5.
        let pos = LinearGradientPosition { start: kurbo::Point::new(0.0, 0.0), end: kurbo::Point::new(10.0, 0.0) };
        let axis = (pos.end.x - pos.start.x, pos.end.y - pos.start.y);
        let axis_len_sq = axis.0 * axis.0 + axis.1 * axis.1;
        let stops = ColorStops::from(
            &[
                peniko::ColorStop::from((0.0f32, Color::from_rgba8(0, 0, 0, 255))),
                peniko::ColorStop::from((1.0f32, Color::from_rgba8(255, 255, 255, 255))),
            ][..],
        );
        let mid = gradient_vertex_color([5.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        let bytes = mid.to_rgba8();
        assert!((bytes.r as i32 - 128).abs() <= 2, "midpoint should be ~50% gray, got r={}", bytes.r);

        let start_color = gradient_vertex_color([0.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(start_color.to_rgba8().r, 0);

        let end_color = gradient_vertex_color([10.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(end_color.to_rgba8().r, 255);
    }

    #[test]
    fn gradient_pad_clamps_outside_axis() {
        let pos = LinearGradientPosition { start: kurbo::Point::new(0.0, 0.0), end: kurbo::Point::new(10.0, 0.0) };
        let axis = (pos.end.x - pos.start.x, pos.end.y - pos.start.y);
        let axis_len_sq = axis.0 * axis.0 + axis.1 * axis.1;
        let stops = ColorStops::from(
            &[
                peniko::ColorStop::from((0.0f32, Color::from_rgba8(0, 0, 0, 255))),
                peniko::ColorStop::from((1.0f32, Color::from_rgba8(255, 255, 255, 255))),
            ][..],
        );
        // Past the end of the axis — Pad should clamp to the last stop.
        let beyond = gradient_vertex_color([50.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(beyond.to_rgba8().r, 255);
        // Before the start — Pad should clamp to the first stop.
        let before = gradient_vertex_color([-50.0, 0.0], &pos, axis, axis_len_sq, &stops, Extend::Pad);
        assert_eq!(before.to_rgba8().r, 0);
    }
}
