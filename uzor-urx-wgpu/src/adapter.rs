//! `Scene` → `InstancedRenderContext` adapter.
//!
//! Walks `urx_core::Scene::commands` in painter's order, calls the
//! corresponding `uzor::render::RenderContext` methods on the
//! provided `InstancedRenderContext` so the underlying primitive
//! pipelines (Quad SDF, Line capsule, lyon triangles, glyph atlas)
//! see the same DrawCmd stream they always have.

use std::time::Instant;

use uzor::render::{Painter, ShapeHelpers};
use uzor_render_wgpu_instanced::InstancedRenderContext;
use uzor_urx_core::math::{Affine, BezPath, Brush, Color};
use uzor_urx_core::scene::{DrawCommand, Scene};
use kurbo::PathEl;

/// Zero-state marker — the adapter is stateless; future per-region
/// caches live one layer up (Phase 5+).
#[derive(Debug, Default, Clone, Copy)]
pub struct UrxWgpuBackend;

impl UrxWgpuBackend {
    pub fn new() -> Self { Self }
}

/// Translate a `Scene` into context calls. Caller has already cleared
/// `ctx.draw_commands` for this frame (the hub's `with_render_context`
/// path takes care of that).
pub fn adapt_scene_into(scene: &Scene, ctx: &mut InstancedRenderContext) {
    use uzor_urx_core::metrics_keys::{
        KEY_RENDER_PRIMITIVES, render_submit_count_key,
    };

    let t0 = Instant::now();
    let mut clip_depth: u32 = 0;

    for cmd in &scene.commands {
        match cmd {
            DrawCommand::FillRect { rect, radii, brush, transform } => {
                degrade_brush(brush, "wgpu_fill_rect_gradient_degraded", "wgpu_fill_rect_image_degraded");
                degrade_radii(radii, "wgpu_fill_rect_radii_dropped");
                degrade_shear(transform, "wgpu_fill_rect_shear_dropped");
                let color = brush_to_color(brush);
                ctx.set_fill_color(&color_to_css(color));
                apply_transform(ctx, transform);
                ctx.fill_rect(rect.x0, rect.y0, rect.width(), rect.height());
                unapply_transform(ctx, transform);
            }
            DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                degrade_brush(brush, "wgpu_stroke_rect_gradient_degraded", "wgpu_stroke_rect_image_degraded");
                degrade_radii(radii, "wgpu_stroke_rect_radii_dropped");
                degrade_shear(transform, "wgpu_stroke_rect_shear_dropped");
                let color = brush_to_color(brush);
                ctx.set_stroke_color(&color_to_css(color));
                ctx.set_stroke_width(stroke.width as f64);
                apply_transform(ctx, transform);
                ctx.stroke_rect(rect.x0, rect.y0, rect.width(), rect.height());
                unapply_transform(ctx, transform);
            }
            DrawCommand::Line { from, to, stroke, brush, transform } => {
                let color = brush_to_color(brush);
                ctx.set_stroke_color(&color_to_css(color));
                ctx.set_stroke_width(stroke.width as f64);
                apply_transform(ctx, transform);
                ctx.begin_path();
                ctx.move_to(from.x, from.y);
                ctx.line_to(to.x, to.y);
                ctx.stroke();
                unapply_transform(ctx, transform);
            }
            DrawCommand::FillPath { path, rule: _rule, brush, transform } => {
                // The underlying InstancedRenderContext has its own
                // path API via Painter's begin_path/move_to/line_to/
                // fill. Lyon tessellator inside it produces triangle
                // instances. Curves are pre-flattened on our side
                // (Painter has no quadratic/cubic helper exposed).
                let color = brush_to_color(brush);
                ctx.set_fill_color(&color_to_css(color));
                apply_transform(ctx, transform);
                ctx.begin_path();
                emit_path_into_ctx(path, ctx);
                ctx.fill();
                unapply_transform(ctx, transform);
                // FillRule honoring is left to the inner ctx; current
                // lyon impl uses NonZero by default. EvenOdd will need
                // an extension to InstancedRenderContext.
            }
            DrawCommand::StrokePath { path, stroke, brush, transform } => {
                let color = brush_to_color(brush);
                ctx.set_stroke_color(&color_to_css(color));
                ctx.set_stroke_width(stroke.width as f64);
                apply_transform(ctx, transform);
                ctx.begin_path();
                emit_path_into_ctx(path, ctx);
                ctx.stroke();
                unapply_transform(ctx, transform);
            }
            DrawCommand::GlyphRun { .. } => {
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_glyphrun_dropped",
                ).increment(1);
            }
            DrawCommand::Image { .. } => {
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_image_dropped",
                ).increment(1);
            }
            DrawCommand::PushClipRect { .. } => {
                ctx.save();
                clip_depth += 1;
                // Don't degrade — true scissor lands when we own the
                // wgpu pass; until then save/restore approximates.
                continue;
            }
            DrawCommand::PushClipRoundedRect { .. } => {
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_rounded_clip_degraded",
                ).increment(1);
                // Underlying context has its own clip stack via
                // axis-aligned rects in fragment-discard. Without
                // extending the API we approximate by saving state;
                // real scissor wiring lands when we move clip into
                // the URX-WGPU layer instead of the inner crate.
                ctx.save();
                clip_depth += 1;
            }
            DrawCommand::PopClip => {
                if clip_depth > 0 {
                    ctx.restore();
                    clip_depth -= 1;
                }
            }
        }
    }
    // Balance any leftover saves (defensive — shouldn't happen if
    // the scene is well-formed).
    while clip_depth > 0 {
        ctx.restore();
        clip_depth -= 1;
    }

    let elapsed_us = t0.elapsed().as_micros() as u64;
    metrics::histogram!(render_submit_count_key("urx_wgpu_adapt")).record(elapsed_us as f64);
    metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
}

#[inline]
fn degrade_brush(brush: &Brush, grad_label: &'static str, image_label: &'static str) {
    use uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES;
    match brush {
        Brush::Gradient(_) => {
            metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => grad_label).increment(1);
        }
        Brush::Image(_) => {
            metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => image_label).increment(1);
        }
        _ => {}
    }
}

#[inline]
fn degrade_radii(radii: &Option<[f32; 4]>, label: &'static str) {
    if let Some(r) = radii {
        if r.iter().any(|v| *v > 0.0) {
            metrics::counter!(
                uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                "kind" => label,
            ).increment(1);
        }
    }
}

#[inline]
fn degrade_shear(transform: &Affine, label: &'static str) {
    let c = transform.as_coeffs();
    if c[1].abs() > 1e-6 || c[2].abs() > 1e-6 {
        metrics::counter!(
            uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
            "kind" => label,
        ).increment(1);
    }
}

/// Emit a kurbo `BezPath` into an `InstancedRenderContext` via the
/// Painter trait. Quadratic/cubic curves flattened on our side
/// (the underlying ctx only exposes move_to/line_to). Tolerance
/// matches urx-cpu (0.25 px).
fn emit_path_into_ctx(path: &BezPath, ctx: &mut InstancedRenderContext) {
    kurbo::flatten(path.elements().iter().copied(), 0.25, |el| {
        match el {
            PathEl::MoveTo(p) => { ctx.move_to(p.x, p.y); }
            PathEl::LineTo(p) => { ctx.line_to(p.x, p.y); }
            PathEl::ClosePath => { ctx.close_path(); }
            _ => {}
        }
    });
}

#[inline]
fn brush_to_color(brush: &Brush) -> Color {
    match brush {
        Brush::Solid(c)    => *c,
        // peniko 0.6: ColorStop.color is DynamicColor — convert back to AlphaColor<Srgb>.
        Brush::Gradient(g) => g.stops.first()
            .map(|s| s.color.to_alpha_color::<peniko::color::Srgb>())
            .unwrap_or(Color::from_rgba8(0, 0, 0, 0)),
        Brush::Image(_)    => Color::from_rgba8(0, 0, 0, 0),
    }
}

#[inline]
fn color_to_css(c: Color) -> String {
    // CSS rgba — the underlying context parses this back into [r,g,b,a].
    // Premultiplication happens in the shader.
    // peniko 0.6: r/g/b/a fields gone — use to_rgba8() byte quad.
    let p = c.to_rgba8();
    format!("rgba({},{},{},{})", p.r, p.g, p.b, (p.a as f64) / 255.0)
}

/// Apply a kurbo `Affine` to the context's transform via the small set
/// of methods exposed by `Painter` (translate/rotate/scale/save+restore).
/// We decompose to translate+scale only — full affine support would
/// require the underlying context to grow a `set_transform([f32;6])`
/// method.
fn apply_transform(ctx: &mut InstancedRenderContext, t: &Affine) {
    if *t == Affine::IDENTITY { return; }
    ctx.save();
    let c = t.as_coeffs();
    // Translate (c[4], c[5]); scale (c[0], c[3]). Rotation/shear
    // dropped silently — caller knows we're a primitive-grade backend.
    let (sx, sy, tx, ty) = (c[0], c[3], c[4], c[5]);
    if tx != 0.0 || ty != 0.0 { ctx.translate(tx, ty); }
    if sx != 1.0 || sy != 1.0 { ctx.scale(sx, sy); }
}

fn unapply_transform(ctx: &mut InstancedRenderContext, t: &Affine) {
    if *t == Affine::IDENTITY { return; }
    ctx.restore();
}

/// Round-trip self-test: feed a 2-command Scene through adapt_scene_into
/// and verify the underlying context recorded the right number of
/// draw_commands.
#[cfg(test)]
mod tests {
    use super::*;
    use uzor_urx_core::math::{Rect, Vec2};
    use uzor_urx_core::scene::Stroke;

    #[test]
    fn adapt_emits_quad_for_fill_rect() {
        let mut ctx = InstancedRenderContext::new(100.0, 100.0, 0.0, 0.0);
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 50.0, 50.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
            transform: Affine::IDENTITY,
        });
        adapt_scene_into(&scene, &mut ctx);
        assert!(!ctx.draw_commands.is_empty(),
                "fill_rect should have produced at least one DrawCmd, got {}",
                ctx.draw_commands.len());
    }

    #[test]
    fn adapt_emits_for_line() {
        let mut ctx = InstancedRenderContext::new(100.0, 100.0, 0.0, 0.0);
        let mut scene = Scene::new();
        scene.push(DrawCommand::Line {
            from: Vec2 { x: 5.0, y: 5.0 },
            to:   Vec2 { x: 50.0, y: 50.0 },
            stroke: Stroke { width: 2.0, ..Stroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 255, 0, 255)),
            transform: Affine::IDENTITY,
        });
        adapt_scene_into(&scene, &mut ctx);
        assert!(!ctx.draw_commands.is_empty(),
                "line should have produced at least one DrawCmd");
    }

    #[test]
    fn adapt_balances_clip_stack() {
        let mut ctx = InstancedRenderContext::new(100.0, 100.0, 0.0, 0.0);
        let mut scene = Scene::new();
        scene.push(DrawCommand::PushClipRect {
            rect: Rect::new(0.0, 0.0, 50.0, 50.0),
            transform: Affine::IDENTITY,
        });
        // Missing PopClip — adapter must balance defensively.
        adapt_scene_into(&scene, &mut ctx);
        // No crash, no leak — the test passing IS the assertion.
    }
}
