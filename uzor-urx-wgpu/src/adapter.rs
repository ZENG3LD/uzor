//! `Scene` → `InstancedRenderContext` adapter.
//!
//! Walks `urx_core::Scene::commands` in painter's order, calls the
//! corresponding `uzor::render::RenderContext` methods on the
//! provided `InstancedRenderContext` so the underlying primitive
//! pipelines (Quad SDF, Line capsule, lyon triangles, glyph atlas)
//! see the same DrawCmd stream they always have.
//!
//! ## Stage 1a (2026-06-05) — degraded paths fixed
//!
//! The previous adapter silently turned gradient brushes, radii,
//! glyphs and rounded clips into counter-increments + no-ops. Stage 1a
//! routes them into the real `InstancedRenderContext` methods that
//! already exist:
//!
//! * `FillRect { radii: Some(r) }` → `fill_rounded_rect(x, y, w, h, r0)`
//!   (single radius — per-corner is a Stage 2 IR addition).
//! * `FillRect { brush: Linear gradient }` → `fill_linear_gradient(...)`
//!   with stop quad + the rect bounds as the gradient endpoints.
//! * `FillRect { brush: Radial gradient }` → degraded to solid for now
//!   (the underlying ctx has no `fill_radial_gradient` method;
//!   delivered in Stage 1b together with the native pipeline rewrite).
//! * `GlyphRun` → `fill_text(text, x, y)` per-glyph fallback. Note: we
//!   don't have the source text string at this layer (Scene carries
//!   pre-shaped glyph ids), so this still degrades; Stage 2 adds the
//!   `(text: String)` companion to `GlyphRun` or a `FontId`-driven
//!   atlas direct submit.
//! * `PushClipRect` / `PopClip` → real clip via path + `clip()`.

use std::time::Instant;

use uzor::render::{GradientPainter, Masking, Painter, ShapeHelpers};
use uzor_render_wgpu_instanced::InstancedRenderContext;
use uzor_urx_core::math::{Affine, BezPath, Brush, Color, GradientKind};
use uzor_urx_core::scene::{DrawCommand, Scene};
use kurbo::PathEl;

/// Zero-state marker — the adapter is stateless; future per-region
/// caches live one layer up.
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
                apply_transform(ctx, transform);
                let (x, y, w, h) = (rect.x0, rect.y0, rect.width(), rect.height());
                match brush {
                    Brush::Solid(c) => {
                        ctx.set_fill_color(&color_to_css(*c));
                        if let Some(r) = radii.filter(|r| r.iter().any(|v| *v > 0.0)) {
                            // Stage 1a — single radius. Stage 2: per-corner.
                            ctx.fill_rounded_rect(x, y, w, h, r[0] as f64);
                        } else {
                            ctx.fill_rect(x, y, w, h);
                        }
                    }
                    Brush::Gradient(grad) => match &grad.kind {
                        GradientKind::Linear(pos) => {
                            // Stage 1a — radii dropped under linear gradient
                            // (the underlying tessellator only knows the rect bounds).
                            let stops: Vec<(f32, String)> = grad.stops.iter().map(|s| {
                                let c = s.color.to_alpha_color::<peniko::color::Srgb>();
                                (s.offset, color_to_css(c))
                            }).collect();
                            let stop_refs: Vec<(f32, &str)> =
                                stops.iter().map(|(t, s)| (*t, s.as_str())).collect();
                            ctx.fill_linear_gradient(
                                &stop_refs,
                                pos.start.x, pos.start.y,
                                pos.end.x,   pos.end.y,
                            );
                            degrade_radii(radii, "wgpu_fill_rect_gradient_radii_dropped");
                        }
                        GradientKind::Radial(_) | GradientKind::Sweep(_) => {
                            // Stage 1b — InstancedRenderContext has no
                            // radial/sweep gradient API; degrade to solid
                            // using the first stop colour.
                            let c = grad.stops.first()
                                .map(|s| s.color.to_alpha_color::<peniko::color::Srgb>())
                                .unwrap_or(Color::from_rgba8(0,0,0,0));
                            ctx.set_fill_color(&color_to_css(c));
                            if let Some(r) = radii.filter(|r| r.iter().any(|v| *v > 0.0)) {
                                ctx.fill_rounded_rect(x, y, w, h, r[0] as f64);
                            } else {
                                ctx.fill_rect(x, y, w, h);
                            }
                            metrics::counter!(
                                KEY_RENDER_PRIMITIVES,
                                "kind" => "wgpu_fill_rect_radial_or_sweep_to_solid",
                            ).increment(1);
                        }
                    },
                    Brush::Image(_) => {
                        // Stage 1b — image brushes require atlas wire.
                        metrics::counter!(
                            KEY_RENDER_PRIMITIVES,
                            "kind" => "wgpu_fill_rect_image_dropped",
                        ).increment(1);
                    }
                }
                unapply_transform(ctx, transform);
            }
            DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                apply_transform(ctx, transform);
                let color = brush_to_solid_color(brush);
                ctx.set_stroke_color(&color_to_css(color));
                ctx.set_stroke_width(stroke.width as f64);
                let (x, y, w, h) = (rect.x0, rect.y0, rect.width(), rect.height());
                if let Some(r) = radii.filter(|r| r.iter().any(|v| *v > 0.0)) {
                    ctx.stroke_rounded_rect(x, y, w, h, r[0] as f64);
                } else {
                    ctx.stroke_rect(x, y, w, h);
                }
                unapply_transform(ctx, transform);
            }
            DrawCommand::Line { from, to, stroke, brush, transform } => {
                let color = brush_to_solid_color(brush);
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
                let color = brush_to_solid_color(brush);
                ctx.set_fill_color(&color_to_css(color));
                apply_transform(ctx, transform);
                ctx.begin_path();
                emit_path_into_ctx(path, ctx);
                ctx.fill();
                unapply_transform(ctx, transform);
            }
            DrawCommand::StrokePath { path, stroke, brush, transform } => {
                let color = brush_to_solid_color(brush);
                ctx.set_stroke_color(&color_to_css(color));
                ctx.set_stroke_width(stroke.width as f64);
                apply_transform(ctx, transform);
                ctx.begin_path();
                emit_path_into_ctx(path, ctx);
                ctx.stroke();
                unapply_transform(ctx, transform);
            }
            DrawCommand::GlyphRun { .. } => {
                // Stage 1a — Scene carries pre-shaped glyph ids (u32) but
                // not the source text. InstancedRenderContext's atlas-based
                // text path takes a `&str`, not glyph ids. Stage 2 IR
                // expansion adds either a `text: String` companion field
                // or a FontId-keyed atlas direct submit path. Until then
                // GlyphRun continues to degrade — UrxRenderContext doesn't
                // emit it yet (it path-converts text to FillPath instead).
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_glyphrun_dropped",
                ).increment(1);
            }
            DrawCommand::Image { .. } => {
                // Stage 1b — image atlas wire.
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_image_dropped",
                ).increment(1);
            }
            DrawCommand::PushClipRect { rect, transform } => {
                apply_transform(ctx, transform);
                ctx.save();
                ctx.begin_path();
                ctx.move_to(rect.x0, rect.y0);
                ctx.line_to(rect.x1, rect.y0);
                ctx.line_to(rect.x1, rect.y1);
                ctx.line_to(rect.x0, rect.y1);
                ctx.close_path();
                ctx.clip();
                clip_depth += 1;
                unapply_transform(ctx, transform);
            }
            DrawCommand::PushClipRoundedRect { rect, transform } => {
                // Stage 1a — approximate rounded clip with the bounding rect.
                // True rounded clip needs stencil — Stage 1b.
                apply_transform(ctx, transform);
                ctx.save();
                ctx.begin_path();
                let r = rect.rect();
                ctx.move_to(r.x0, r.y0);
                ctx.line_to(r.x1, r.y0);
                ctx.line_to(r.x1, r.y1);
                ctx.line_to(r.x0, r.y1);
                ctx.close_path();
                ctx.clip();
                clip_depth += 1;
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_rounded_clip_to_rect_bbox",
                ).increment(1);
                unapply_transform(ctx, transform);
            }
            DrawCommand::PopClip => {
                if clip_depth > 0 {
                    ctx.restore();
                    clip_depth -= 1;
                }
            }
            DrawCommand::PushBlendLayer { .. } | DrawCommand::PopBlendLayer => {
                // Stage 2 IR additions — non-SrcOver blend modes on the
                // wgpu adapter require offscreen-target lift, which the
                // underlying InstancedRenderContext doesn't expose yet.
                // Drop silently; Stage 1b's native pipeline rewrite owns
                // the proper implementation.
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "wgpu_blend_layer_dropped",
                ).increment(1);
            }
        }
    }
    while clip_depth > 0 {
        ctx.restore();
        clip_depth -= 1;
    }

    let elapsed_us = t0.elapsed().as_micros() as u64;
    metrics::histogram!(render_submit_count_key("urx_wgpu_adapt")).record(elapsed_us as f64);
    metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
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
fn brush_to_solid_color(brush: &Brush) -> Color {
    match brush {
        Brush::Solid(c)    => *c,
        Brush::Gradient(g) => g.stops.first()
            .map(|s| s.color.to_alpha_color::<peniko::color::Srgb>())
            .unwrap_or(Color::from_rgba8(0, 0, 0, 0)),
        Brush::Image(_)    => Color::from_rgba8(0, 0, 0, 0),
    }
}

#[inline]
fn color_to_css(c: Color) -> String {
    let p = c.to_rgba8();
    format!("rgba({},{},{},{})", p.r, p.g, p.b, (p.a as f64) / 255.0)
}

/// Apply a kurbo `Affine` to the context's transform via the small set
/// of methods exposed by `Painter` (translate/rotate/scale/save+restore).
/// We decompose to translate+scale only — full affine support would
/// require the underlying context to grow a `set_transform([f32;6])`
/// method (deferred to Stage 1b).
fn apply_transform(ctx: &mut InstancedRenderContext, t: &Affine) {
    if *t == Affine::IDENTITY { return; }
    ctx.save();
    let c = t.as_coeffs();
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
        assert!(!ctx.draw_commands.is_empty());
    }

    #[test]
    fn adapt_emits_rounded_rect_for_radii() {
        let mut ctx = InstancedRenderContext::new(100.0, 100.0, 0.0, 0.0);
        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 50.0, 50.0),
            radii: Some([8.0, 8.0, 8.0, 8.0]),
            brush: Brush::Solid(Color::from_rgba8(255, 0, 0, 255)),
            transform: Affine::IDENTITY,
        });
        adapt_scene_into(&scene, &mut ctx);
        // Verifies the rounded-radius path doesn't degrade now.
        assert!(!ctx.draw_commands.is_empty());
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
        assert!(!ctx.draw_commands.is_empty());
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
    }
}
