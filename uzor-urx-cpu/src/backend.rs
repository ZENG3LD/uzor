//! Top-level CPU backend — walks `Scene::commands`, dispatches to
//! per-primitive rasterisers, emits metrics.

use std::time::Instant;

use uzor_urx_core::math::Rect;
use uzor_urx_core::scene::{DrawCommand, Scene};

use crate::clip::ClipStack;
use crate::color::brush_to_color;
use crate::fill::fill_rect_aa;
use crate::pixmap::Pixmap;
use crate::stroke::{stroke_line_aa, stroke_rect_aa};

#[derive(Debug, Clone, Copy)]
pub enum RenderError {
    /// `Scene::commands` had unbalanced PushClip/PopClip — too many pops.
    ClipUnderflow,
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
    pub fn render(&self, scene: &Scene, pixmap: &mut Pixmap) -> Result<(), RenderError> {
        use uzor_urx_core::metrics_keys::{
            render_submit_us_key, render_submit_count_key,
            KEY_TICK_SUBMIT_US, KEY_TICK_FRAMES,
            KEY_RENDER_PRIMITIVES,
        };

        let t0 = Instant::now();
        let bounds = Rect::new(0.0, 0.0, pixmap.width() as f64, pixmap.height() as f64);
        let mut clip = ClipStack::new(bounds);

        for cmd in &scene.commands {
            match cmd {
                DrawCommand::FillRect { rect, radii: _radii, brush, transform } => {
                    // Gradient brush → scanline gradient path. Otherwise
                    // fall through to solid fill_rect_aa.
                    if matches!(brush, uzor_urx_core::math::Brush::Gradient(_)) {
                        if crate::gradient::try_fill_rect_gradient(pixmap, &clip, *rect, brush, transform).is_some() {
                            continue;
                        }
                    }
                    let color = brush_to_color(brush);
                    fill_rect_aa(pixmap, &clip, *rect, color, transform);
                }
                DrawCommand::StrokeRect { rect, radii: _radii, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
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
                DrawCommand::GlyphRun { .. } => {
                    // Text deferred to Phase 3.5 (skrifa atlas). For now
                    // we skip so a scene containing text renders the
                    // non-text portion correctly instead of failing.
                }
                DrawCommand::Image { .. } => {
                    // Image deferred — needs the image-atlas plumbing.
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
