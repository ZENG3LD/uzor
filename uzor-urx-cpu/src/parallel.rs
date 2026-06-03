//! Per-strip parallel rasterisation (feature = "parallel").
//!
//! Splits the destination pixmap into N horizontal strips, runs the
//! whole scene walker on each strip in parallel via rayon. Each
//! primitive's pixel writes are auto-clipped to the strip's y-range.
//!
//! Wins scale with strip count (≤ core count) AND with vertical pixel
//! coverage per command (a 600-row rect benefits more than a 20-row
//! one — but most UI scenes have lots of independent rects, so the
//! parallelism wins per-frame regardless).

use std::time::Instant;

use uzor_urx_core::math::{Affine, Rect};
use uzor_urx_core::scene::{DrawCommand, Scene};

use crate::backend::RenderError;
use crate::clip::{transform_axis_aligned, ClipStack};
use crate::color::{brush_to_color, color_to_premul, premul_scale};
use crate::fill::axis_coverage;
use crate::pixmap::{Pixmap, PixmapStripMut};

/// Render a scene in parallel across N horizontal strips.
///
/// `strips = 0` means "auto" (one strip per core). Returns elapsed
/// wall-clock per render.
pub fn render_parallel(
    scene:  &Scene,
    pixmap: &mut Pixmap,
    strips: usize,
) -> Result<u64, RenderError> {
    use rayon::prelude::*;
    use uzor_urx_core::metrics_keys::{
        render_submit_count_key, render_submit_us_key, KEY_RENDER_PRIMITIVES,
        KEY_TICK_FRAMES, KEY_TICK_SUBMIT_US,
    };

    let t0 = Instant::now();
    let strip_count = if strips == 0 {
        rayon::current_num_threads().max(1)
    } else {
        strips
    };

    // Pre-compute bounds of the whole pixmap so each strip's clip
    // bottom is its own row range.
    let pixmap_w = pixmap.width();
    let pixmap_h = pixmap.height();
    let mut strip_views = pixmap.split_strips_mut(strip_count);

    strip_views.par_iter_mut().try_for_each(|strip| -> Result<(), RenderError> {
        let strip_bounds = Rect::new(
            0.0,
            strip.y0() as f64,
            pixmap_w as f64,
            (strip.y0() + strip.rows()) as f64,
        );
        let mut clip = ClipStack::new(strip_bounds);
        for cmd in &scene.commands {
            match cmd {
                DrawCommand::FillRect { rect, radii: _, brush, transform } => {
                    let color = brush_to_color(brush);
                    fill_rect_strip(strip, &clip, *rect, color, transform);
                }
                DrawCommand::StrokeRect { rect, radii: _, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    stroke_rect_strip(strip, &clip, *rect, stroke.width, color, transform);
                }
                DrawCommand::Line { from, to, stroke, brush, transform } => {
                    let color = brush_to_color(brush);
                    stroke_line_strip(strip, &clip, *from, *to, stroke.width, color, transform);
                }
                DrawCommand::FillPath { .. } | DrawCommand::StrokePath { .. } => {
                    // Path raster has internal edge-table state that
                    // is per-scene, not per-strip — running it inside
                    // each strip would duplicate work AND each strip
                    // only has partial vertical coverage of the path.
                    // Phase 8 limitation: paths fall back to the
                    // main thread. Phase 9 fix: per-strip edge table
                    // pre-built once, used by each strip with its own
                    // y-range filter.
                    //
                    // For now, the parallel path silently skips paths.
                    // Consumers using paths heavily should stay on the
                    // sequential render() entry point.
                }
                DrawCommand::GlyphRun { .. } | DrawCommand::Image { .. } => {}
                DrawCommand::PushClipRect { rect, transform } => {
                    clip.push_rect(*rect, transform);
                }
                DrawCommand::PushClipRoundedRect { rect, transform } => {
                    clip.push_rounded_rect(*rect, transform);
                }
                DrawCommand::PopClip => { clip.pop(); }
            }
        }
        Ok(())
    })?;

    let elapsed_us = t0.elapsed().as_micros() as u64;
    metrics::histogram!(KEY_TICK_SUBMIT_US).record(elapsed_us as f64);
    metrics::counter!(KEY_TICK_FRAMES).increment(1);
    metrics::histogram!(render_submit_us_key("urx_cpu_parallel")).record(elapsed_us as f64);
    metrics::counter!(render_submit_count_key("urx_cpu_parallel")).increment(1);
    metrics::counter!(KEY_RENDER_PRIMITIVES).increment(scene.commands.len() as u64);
    let _ = pixmap_h;
    Ok(elapsed_us)
}

// ── strip-aware primitives — same math as fill.rs/stroke.rs but
//    they call strip.blend_pixel_parent instead of pixmap.blend_pixel.

fn fill_rect_strip(
    strip: &mut PixmapStripMut<'_>,
    clip:  &ClipStack,
    rect:  Rect,
    color: uzor_urx_core::math::Color,
    transform: &Affine,
) {
    let r_screen = transform_axis_aligned(*transform, rect);
    let c = clip.current();
    let visible = r_screen.intersect(c);
    if visible.width() <= 0.0 || visible.height() <= 0.0 { return; }

    let fx0 = visible.x0;
    let fy0 = visible.y0;
    let fx1 = visible.x1;
    let fy1 = visible.y1;

    let ix0 = fx0.floor() as i64;
    let iy0 = fy0.floor() as i64;
    let ix1 = (fx1.ceil() as i64).max(ix0 + 1);
    let iy1 = (fy1.ceil() as i64).max(iy0 + 1);

    let strip_y0 = strip.y0() as i64;
    let strip_y1 = (strip.y0() + strip.rows()) as i64;
    let pix_w = strip.width() as i64;
    let ix0 = ix0.max(0);
    let iy0 = iy0.max(strip_y0);
    let ix1 = ix1.min(pix_w);
    let iy1 = iy1.min(strip_y1);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let premul = color_to_premul(color);
    for py in iy0 .. iy1 {
        let v_cov = axis_coverage(py as f64, py as f64 + 1.0, fy0, fy1);
        if v_cov == 0 { continue; }
        for px in ix0 .. ix1 {
            let h_cov = axis_coverage(px as f64, px as f64 + 1.0, fx0, fx1);
            if h_cov == 0 { continue; }
            let cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
            let src = premul_scale(premul, cov);
            strip.blend_pixel_parent(px as u32, py as u32, src);
        }
    }
}

fn stroke_rect_strip(
    strip: &mut PixmapStripMut<'_>,
    clip:  &ClipStack,
    rect:  Rect,
    width: f32,
    color: uzor_urx_core::math::Color,
    transform: &Affine,
) {
    let r = transform_axis_aligned(*transform, rect);
    let w = width as f64;
    let half = w * 0.5;
    let outer = Rect::new(r.x0 - half, r.y0 - half, r.x1 + half, r.y1 + half);
    let inner = Rect::new(r.x0 + half, r.y0 + half, r.x1 - half, r.y1 - half);
    if inner.width() <= 0.0 || inner.height() <= 0.0 {
        fill_rect_strip(strip, clip, outer, color, &Affine::IDENTITY);
        return;
    }
    let top    = Rect::new(outer.x0, outer.y0, outer.x1, inner.y0);
    let bottom = Rect::new(outer.x0, inner.y1, outer.x1, outer.y1);
    let left   = Rect::new(outer.x0, inner.y0, inner.x0, inner.y1);
    let right  = Rect::new(inner.x1, inner.y0, outer.x1, inner.y1);
    for b in [top, bottom, left, right] {
        fill_rect_strip(strip, clip, b, color, &Affine::IDENTITY);
    }
}

fn stroke_line_strip(
    strip: &mut PixmapStripMut<'_>,
    clip:  &ClipStack,
    from:  uzor_urx_core::math::Vec2,
    to:    uzor_urx_core::math::Vec2,
    width: f32,
    color: uzor_urx_core::math::Color,
    transform: &Affine,
) {
    let p0 = transform_point(transform, from);
    let p1 = transform_point(transform, to);
    let half_w = width as f64 * 0.5;

    let bb_x0 = (p0.x.min(p1.x) - half_w - 1.0).floor();
    let bb_y0 = (p0.y.min(p1.y) - half_w - 1.0).floor();
    let bb_x1 = (p0.x.max(p1.x) + half_w + 1.0).ceil();
    let bb_y1 = (p0.y.max(p1.y) + half_w + 1.0).ceil();
    let c = clip.current();
    let bb = Rect::new(bb_x0, bb_y0, bb_x1, bb_y1).intersect(c);
    if bb.width() <= 0.0 || bb.height() <= 0.0 { return; }

    let strip_y0 = strip.y0() as i64;
    let strip_y1 = (strip.y0() + strip.rows()) as i64;
    let pix_w = strip.width() as i64;
    let ix0 = (bb.x0 as i64).max(0);
    let iy0 = (bb.y0 as i64).max(strip_y0);
    let ix1 = (bb.x1 as i64).min(pix_w);
    let iy1 = (bb.y1 as i64).min(strip_y1);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let premul = color_to_premul(color);
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let len2 = dx * dx + dy * dy;

    for py in iy0 .. iy1 {
        for px in ix0 .. ix1 {
            let cx = px as f64 + 0.5;
            let cy = py as f64 + 0.5;
            let d = if len2 < 1e-9 {
                let ddx = cx - p0.x;
                let ddy = cy - p0.y;
                (ddx * ddx + ddy * ddy).sqrt()
            } else {
                let t = ((cx - p0.x) * dx + (cy - p0.y) * dy) / len2;
                let t = t.clamp(0.0, 1.0);
                let nx = p0.x + t * dx;
                let ny = p0.y + t * dy;
                let ddx = cx - nx;
                let ddy = cy - ny;
                (ddx * ddx + ddy * ddy).sqrt()
            };
            let cov_f = if d <= half_w - 0.5 {
                1.0
            } else if d >= half_w + 0.5 {
                0.0
            } else {
                (half_w + 0.5 - d).clamp(0.0, 1.0)
            };
            if cov_f <= 0.0 { continue; }
            let cov = (cov_f * 255.0 + 0.5) as u8;
            let src = premul_scale(premul, cov);
            strip.blend_pixel_parent(px as u32, py as u32, src);
        }
    }
}

#[inline]
fn transform_point(t: &Affine, p: uzor_urx_core::math::Vec2) -> uzor_urx_core::math::Vec2 {
    let c = t.as_coeffs();
    uzor_urx_core::math::Vec2 {
        x: c[0] * p.x + c[2] * p.y + c[4],
        y: c[1] * p.x + c[3] * p.y + c[5],
    }
}
