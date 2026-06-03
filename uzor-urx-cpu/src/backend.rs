//! Top-level CPU backend — walks `Scene::commands`, dispatches to
//! per-primitive rasterisers, emits metrics.

use std::time::Instant;

use uzor_urx_core::math::{Brush, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};

/// Predicate: is this scene eligible for the tile pipeline?
/// Tile path supports: FillRect (Solid brush only, no radii, axis-aligned
/// transform), PushClipRect, PopClip. Anything else forces the
/// scanline fallback.
fn tile_eligible(scene: &Scene) -> bool {
    for cmd in &scene.commands {
        match cmd {
            DrawCommand::FillRect { radii, brush, transform, .. } => {
                if let Some(r) = radii {
                    if r.iter().any(|v| *v > 0.0) { return false; }
                }
                if !matches!(brush, Brush::Solid(_)) { return false; }
                let c = transform.as_coeffs();
                if c[1].abs() > 1e-6 || c[2].abs() > 1e-6 { return false; }
            }
            DrawCommand::PushClipRect { transform, .. } => {
                let c = transform.as_coeffs();
                if c[1].abs() > 1e-6 || c[2].abs() > 1e-6 { return false; }
            }
            DrawCommand::PopClip => {}
            _ => return false,
        }
    }
    true
}

use crate::clip::ClipStack;
use crate::color::brush_to_color;
use crate::fill::fill_rect_aa;
use crate::pixmap::Pixmap;
use crate::stroke::{stroke_line_aa, stroke_rect_aa};

#[derive(Debug, Clone, Copy)]
pub enum RenderError {
    /// `Scene::commands` had unbalanced PushClip/PopClip — too many pops.
    ClipUnderflow,
    /// `render_parallel` cannot handle the scene because it contains
    /// primitives that need shared per-scene state (paths, glyphs, images).
    /// Caller should drop back to the sequential `render()` entry point.
    /// Carries the (zero-indexed) command position so callers can locate
    /// the offending primitive.
    ParallelUnsupported(usize),
}

/// The CPU backend. Stateless — every call is `render(scene, pixmap)`.
/// (Per-region caches live one layer up, in the URX engine — keeping
/// the backend itself stateless makes it trivially `Send + Sync` for
/// the future rayon-per-region path.)
#[derive(Debug, Default, Clone, Copy)]
pub struct CpuBackend;

impl CpuBackend {
    pub fn new() -> Self { Self }

    /// Render a whole scene into a pixmap. Does NOT clear the pixmap
    /// first — caller decides background fill (gives us a free
    /// `LoadOp::Load` equivalent for dirty-rect re-paint).
    ///
    /// **Auto-routing**: if the scene contains ONLY FillRect commands
    /// with simple brushes (Solid) and axis-aligned transforms, AND
    /// command count ≥ 50, the tile pipeline is used (bumpalo arena
    /// + rayon parallel band flush + bg-replacement). Otherwise falls
    /// through to the per-primitive scanline backend.
    pub fn render(&self, scene: &Scene, pixmap: &mut Pixmap) -> Result<(), RenderError> {
        use uzor_urx_core::metrics_keys::{
            render_submit_us_key, render_submit_count_key,
            KEY_TICK_SUBMIT_US, KEY_TICK_FRAMES,
            KEY_RENDER_PRIMITIVES,
        };

        let t0 = Instant::now();

        if scene.commands.len() >= 50 && tile_eligible(scene) {
            crate::tile::render_tiled(scene, pixmap);
            let elapsed_us = t0.elapsed().as_micros() as u64;
            metrics::histogram!(KEY_TICK_SUBMIT_US).record(elapsed_us as f64);
            metrics::counter!(KEY_TICK_FRAMES).increment(1);
            metrics::histogram!(render_submit_us_key("urx_cpu_tile")).record(elapsed_us as f64);
            metrics::counter!(render_submit_count_key("urx_cpu_tile")).increment(1);
            metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
            return Ok(());
        }

        let bounds = Rect::new(0.0, 0.0, pixmap.width() as f64, pixmap.height() as f64);
        let mut clip = ClipStack::new(bounds);

        for cmd in &scene.commands {
            match cmd {
                DrawCommand::FillRect { rect, radii, brush, transform } => {
                    // Corner radii → push a transient rounded clip,
                    // draw the rect, pop. Same path the consumer would
                    // have to write by hand otherwise.
                    let _radii_guard = if let Some(r) = radii {
                        if r.iter().any(|v| *v > 0.0) {
                            let rr = uzor_urx_core::math::RoundedRect::from_rect(
                                *rect,
                                uzor_urx_core::math::RoundedRectRadii::new(
                                    r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64,
                                ),
                            );
                            clip.push_rounded_rect(rr, transform);
                            true
                        } else { false }
                    } else { false };
                    if matches!(brush, uzor_urx_core::math::Brush::Gradient(_)) {
                        if crate::gradient::try_fill_rect_gradient(pixmap, &clip, *rect, brush, transform).is_some() {
                            if _radii_guard { clip.pop(); }
                            continue;
                        }
                    }
                    let color = brush_to_color(brush);
                    fill_rect_aa(pixmap, &clip, *rect, color, transform);
                    if _radii_guard { clip.pop(); }
                }
                DrawCommand::StrokeRect { rect, radii, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    if let Some(r) = radii {
                        if r.iter().any(|v| *v > 0.0) {
                            // Round-corner stroke = stroke a flattened
                            // rounded path (uses scanline + capsules).
                            let rr = uzor_urx_core::math::RoundedRect::from_rect(
                                *rect,
                                uzor_urx_core::math::RoundedRectRadii::new(
                                    r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64,
                                ),
                            );
                            use kurbo::Shape as _;
                            let path: uzor_urx_core::math::BezPath = rr.into_path(0.25);
                            crate::path::stroke_path_aa(pixmap, &clip, &path, stroke, color, transform);
                            continue;
                        }
                    }
                    stroke_rect_aa(pixmap, &clip, *rect, stroke.width, color, transform);
                }
                DrawCommand::Line { from, to, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    stroke_line_aa(pixmap, &clip, *from, *to, stroke.width, color, transform);
                }
                DrawCommand::FillPath { path, rule, brush, transform } => {
                    let color = brush_to_color(brush);
                    crate::path::fill_path_aa(pixmap, &clip, path, *rule, color, transform);
                }
                DrawCommand::StrokePath { path, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    crate::path::stroke_path_aa(pixmap, &clip, path, stroke, color, transform);
                }
                DrawCommand::GlyphRun { glyphs, font, font_size, brush, transform } => {
                    #[cfg(feature = "glyph")]
                    {
                        let color = brush_to_color(brush);
                        let coeffs = transform.as_coeffs();
                        let (tx, ty) = (coeffs[4] as f32, coeffs[5] as f32);
                        let pw = pixmap.width();
                        let ph = pixmap.height();
                        let _ = uzor_urx_glyph::draw_glyph_run(
                            pixmap.pixels_mut(),
                            pw, ph,
                            tx, ty,
                            glyphs,
                            *font,
                            *font_size,
                            [color.r, color.g, color.b, color.a],
                        );
                    }
                    #[cfg(not(feature = "glyph"))]
                    {
                        let _ = (glyphs, font, font_size, brush, transform);
                        metrics::counter!(
                            uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                            "kind" => "glyph_run_skipped_no_feature",
                        ).increment(1);
                    }
                }
                DrawCommand::Image { src, src_rect, dest, transform } => {
                    let _ = crate::image_draw::draw_image_aa(
                        pixmap, &clip, *src, *src_rect, *dest, transform,
                    );
                }
                DrawCommand::PushClipRect { rect, transform } => {
                    clip.push_rect(*rect, transform);
                }
                DrawCommand::PushClipRoundedRect { rect, transform } => {
                    clip.push_rounded_rect(*rect, transform);
                }
                DrawCommand::PopClip => {
                    clip.pop();
                }
            }
        }

        let elapsed_us = t0.elapsed().as_micros() as u64;
        metrics::histogram!(KEY_TICK_SUBMIT_US).record(elapsed_us as f64);
        metrics::counter!(KEY_TICK_FRAMES).increment(1);
        metrics::histogram!(render_submit_us_key("urx_cpu")).record(elapsed_us as f64);
        metrics::counter!(render_submit_count_key("urx_cpu")).increment(1);
        metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
        Ok(())
    }
}
