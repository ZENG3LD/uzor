//! `Scene` → native-pipeline instance encoder.
//!
//! Commit 1 populates `EncodedFrame::quads` only, from `FillRect`/
//! `StrokeRect` (`Brush::Solid` — gradient/image degrade to a
//! first-stop/transparent solid, counted). Every other `DrawCommand`
//! variant is a counted, never-silent degrade via
//! `uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES` with a
//! `native_*` `kind` tag — distinct from the legacy adapter's `wgpu_*`
//! labels so dashboards can tell the two backends apart.

use uzor_urx_core::math::{Affine, Brush, Color, Rect};
use uzor_urx_core::metrics_keys::{KEY_RENDER_PRIMITIVES, KEY_RENDER_SKIPPED_NONFINITE};
use uzor_urx_core::scene::{DrawCommand, Scene, Stroke};
use uzor_urx_core::validate::{validate_command, ValidationIssue};

use crate::pipelines::quad::{pack_rgba8, QuadInstance};
use crate::renderer::Viewport;

/// Painter's-order encoded frame. Commit 1 only ever populates `quads`
/// — `lines`/`triangles` land in Commit 2/3 (design §5 `EncodedFrame`
/// grows then; adding empty placeholder fields for pipelines that
/// don't exist yet would just be dead weight this commit).
#[derive(Debug, Default)]
pub(crate) struct EncodedFrame {
    pub(crate) quads: Vec<QuadInstance>,
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
            DrawCommand::Line { .. } => degrade("native_line_not_yet_implemented"),
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

    frame.quads.push(QuadInstance {
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

    frame.quads.push(QuadInstance {
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
    fn unimplemented_line_command_is_counted_not_silent() {
        // We can't easily assert on the global metrics recorder here
        // (no test recorder installed in this crate's unit tests), but
        // we CAN assert the encoder doesn't panic and produces zero
        // quads for a still-unsupported primitive.
        let mut scene = Scene::new();
        scene.line_solid(
            uzor_urx_core::math::Vec2 { x: 0.0, y: 0.0 },
            uzor_urx_core::math::Vec2 { x: 10.0, y: 10.0 },
            2.0,
            Color::from_rgba8(255, 255, 255, 255),
        );
        let frame = encode_scene(&scene, viewport());
        assert!(frame.quads.is_empty());
    }
}
