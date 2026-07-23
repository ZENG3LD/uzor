//! `Scene` → native-pipeline instance encoder.
//!
//! Commit 1 covered `FillRect`/`StrokeRect` (`Brush::Solid` — gradient/
//! image degrade to a first-stop/transparent solid, counted). Commit 2
//! adds `Line` and the painter's-order batching machinery (`Batch`/
//! `BatchKind`) that keeps Quad/Line draws interleaved correctly.
//! Every other `DrawCommand` variant is a counted, never-silent degrade
//! via `uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES` with a
//! `native_*` `kind` tag — distinct from the legacy adapter's `wgpu_*`
//! labels so dashboards can tell the two backends apart.

use uzor_urx_core::math::{Affine, Brush, Color, Rect, Vec2};
use uzor_urx_core::metrics_keys::{KEY_RENDER_PRIMITIVES, KEY_RENDER_SKIPPED_NONFINITE};
use uzor_urx_core::scene::{DrawCommand, LineCap, Scene, Stroke};
use uzor_urx_core::validate::{validate_command, ValidationIssue};

use crate::pipelines::line::LineInstance;
use crate::pipelines::quad::{pack_rgba8, QuadInstance};
use crate::renderer::Viewport;

/// Which pipeline a [`Batch`] replays through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchKind {
    Quad,
    Line,
}

/// A contiguous run of same-kind instances in painter's-order scan
/// order — `start`/`count` index into `EncodedFrame::quads` or
/// `EncodedFrame::lines` depending on `kind`. Copied algorithm from
/// legacy `InstancedRenderer::render`'s Phase-1 scan
/// (`uzor-render-wgpu-instanced/src/renderer.rs:766-848`, design §5):
/// a new batch opens only when the kind differs from the previous
/// one, so consecutive same-kind commands coalesce into one draw call
/// while painter's order (replay in scan order, never reordered) is
/// preserved across a Quad/Line interleave.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Batch {
    pub(crate) kind: BatchKind,
    pub(crate) start: u32,
    pub(crate) count: u32,
}

/// Painter's-order encoded frame — the Scene walker's output, ready
/// for `NativeUrxRenderer` to upload + replay via `batches`.
/// `triangles` (Path pipeline) lands in Commit 3.
#[derive(Debug, Default)]
pub(crate) struct EncodedFrame {
    pub(crate) quads: Vec<QuadInstance>,
    pub(crate) lines: Vec<LineInstance>,
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

/// Single radius for the SDF (Wave 1 approximation — per-corner radii
/// land in Wave 4). Emits `native_per_corner_radii_uniform_approx`
/// when the four radii actually differ, so the approximation is never
/// silent.
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

pub(crate) fn encode_scene(scene: &Scene, viewport: Viewport) -> EncodedFrame {
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
                encode_fill_rect(&mut frame, *rect, radii, brush, transform, full_clip);
            }
            DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                encode_stroke_rect(&mut frame, *rect, radii, stroke, brush, transform, full_clip);
            }
            DrawCommand::Line { from, to, stroke, brush, transform } => {
                encode_line(&mut frame, *from, *to, stroke, brush, transform, full_clip);
            }
            DrawCommand::FillPath { .. } => degrade("native_fill_path_not_yet_implemented"),
            DrawCommand::StrokePath { .. } => degrade("native_stroke_path_not_yet_implemented"),
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
    rect: Rect,
    radii: &Option<[f32; 4]>,
    brush: &Brush,
    transform: &Affine,
    clip_rect: [f32; 4],
) {
    let color = match brush {
        Brush::Solid(c) => *c,
        Brush::Gradient(_) => {
            // Wave 1 scope: FillRect gradients route through the Path
            // pipeline once it lands (Commit 3); until then, first-stop
            // solid — counted, never silent.
            degrade("native_fill_rect_gradient_pending_path_pipeline");
            resolve_brush_color(brush).0
        }
        Brush::Image(_) => {
            // No image atlas wired yet — dropped entirely, matching the
            // legacy adapter's own `wgpu_fill_rect_image_dropped` policy
            // (`adapter.rs:109-115`).
            degrade("native_fill_rect_image_dropped");
            return;
        }
    };

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

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_urx_core::scene::Stroke as SceneStroke;

    fn viewport() -> Viewport {
        Viewport { width: 100, height: 100 }
    }

    #[test]
    fn fill_rect_solid_emits_one_quad() {
        let mut scene = Scene::new();
        scene.fill_rect_solid(Rect::new(10.0, 10.0, 50.0, 50.0), Color::from_rgba8(255, 0, 0, 255));
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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
        let frame = encode_scene(&scene, viewport());
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

        let frame = encode_scene(&scene, viewport());
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

        let frame = encode_scene(&scene, viewport());
        assert_eq!(frame.batches.len(), 4);
        let kinds: Vec<BatchKind> = frame.batches.iter().map(|b| b.kind).collect();
        assert_eq!(kinds, vec![BatchKind::Quad, BatchKind::Line, BatchKind::Quad, BatchKind::Line]);
    }
}
