//! Path rasterisation — fill (scanline AET + winding) and stroke
//! (per-segment capsule + round joins/caps).
//!
//! Fill uses a classic Active-Edge-Table, sampled at [`VERTICAL_SUBSAMPLES`]
//! sub-scanlines per pixel row (see that constant's own doc comment for
//! why one sample per row isn't enough for thin, steeply-sloped content
//! like a small rotated glyph stroke):
//!   * edges bucketed by their first active sub-scanline → activated when
//!     the AET reaches them
//!   * each edge appears in the AET exactly once per sub-scanline it's
//!     active on, x_at advanced in place
//!   * retain while `y_max > sample_y`; drop otherwise
//!   * spans emitted by winding rule (NonZero / EvenOdd) per sub-scanline
//!   * per-span horizontal AA via shared `axis_coverage`, averaged across
//!     all sub-scanlines before blending a row once
//!
//! Flatten tolerance is applied in screen-space: the tolerance handed to
//! `kurbo::flatten` is `0.25 / max_scale` so a 2× zoomed path still stays
//! sub-pixel after the affine maps it to the pixmap.
//!
//! Stroke flattens the path then renders each segment as a capsule.
//! At every joint we drop a `half_width` disc — gives a round-join
//! visual and round-cap on subpath endpoints with one primitive. The
//! `Stroke.join` / `Stroke.cap` / `Stroke.miter_limit` fields are not
//! yet honoured per-style; round is the only mode that renders today
//! and a `tracing::warn!` is emitted on first non-round style use to
//! avoid silent visual drift.

use std::sync::atomic::{AtomicBool, Ordering};

use kurbo::{Affine as KAffine, BezPath, PathEl, Point as KPoint};

use crate::clip::ClipStack;
use crate::color::{color_to_premul, premul_scale};
use crate::pixmap::Pixmap;
use uzor_urx_core::math::{Affine, Color, Rect};
use uzor_urx_core::scene::{FillRule, LineCap, LineJoin, Stroke};

const FLATTEN_TOLERANCE_PX: f64 = 0.25;
const CLOSE_EPS: f64 = 1e-6;

/// Vertical supersample count for [`fill_path_aa`]'s scanline coverage.
///
/// The AET fill takes ONE sample height per pixel row (`sample_y = py +
/// 0.5`) and anti-aliases only the HORIZONTAL span edges via
/// `axis_coverage` (module doc's own "per-span horizontal AA" — by
/// design, and correct for the common case this rasterizer was tuned
/// against: axis-aligned/gently-sloped UI shapes, where a near-
/// horizontal edge essentially never occurs). A thin (sub-2px-wide),
/// STEEPLY or NEAR-HORIZONTALLY sloped edge — exactly what a small
/// rotated glyph stroke presents, since a round letterform's tangent
/// sweeps through every angle and a 45° rotation reliably lines some of
/// it up near-horizontal — breaks that assumption: single-sample-per-row
/// vertical resolution can miss/mis-hit a stroke narrower than 1px in
/// the row's OWN vertical direction, which reads as torn/piled/doubled
/// strokes (root-caused 2026-07-25 against a `uzor-figures`
/// `LabelOverflow::Rotate(45)` proof: `urx-cpu`'s rendering of a small
/// rotated axis label was geometrically correct — right anchor/tilt —
/// but glyph CONTENT was mangled, while tiny-skia/vello-cpu, both true
/// 2D-analytic/multi-sample rasterizers, stayed legible; a controlled
/// size sweep at 11/22/44px showed the defect shrinking as glyph (hence
/// stroke width in device pixels) grew — the signature of a vertical-
/// resolution artifact, not a flattening-tolerance or path-emission bug:
/// tightening `FLATTEN_TOLERANCE_PX` 6x in isolation measurably changed
/// nothing).
///
/// 4x vertical supersampling (each row split into 4 sub-scanlines,
/// coverage averaged) bounds the same row-center-sampling error to
/// ±1/8px instead of ±1/2px — small enough that even an 11px glyph's
/// ~1px-wide stroke renders legibly, confirmed by the regenerated size-
/// sweep proof. Scoped to THIS function only (`fill_rect_aa`/
/// `stroke_path_aa` use their own, already-2D-correct or capsule-based
/// coverage and don't share this failure mode) — the perf cost (up to
/// 4x the edge-activation/winding work per fill) is confined to generic
/// path fills (glyph-outline fallback, pie/sankey/dag curves, stroke-
/// joint triangles), never the axis-aligned rect fill every bar chart's
/// bars go through (`compare_baselines`'s own bench doc: "path
/// rendering... not measured here — separate concern").
const VERTICAL_SUBSAMPLES: usize = 4;

#[derive(Debug, Clone, Copy)]
struct Edge {
    y_min:  f64,   // upper edge in screen y
    y_max:  f64,   // lower edge (exclusive at scanline level)
    x_at:   f64,   // current x at the current sample_y; mutated per scanline
    dx_dy:  f64,
    dir:    i32,   // +1 down, -1 up
}

#[inline]
fn screen_flatten_tolerance(transform: &KAffine) -> f64 {
    // Take 2-norm of the 2x2 linear part rows; use max as a conservative
    // "screen-pixels per path-unit" scale.
    let c = transform.as_coeffs();
    let s_row0 = (c[0] * c[0] + c[2] * c[2]).sqrt();
    let s_row1 = (c[1] * c[1] + c[3] * c[3]).sqrt();
    let max_scale = s_row0.max(s_row1).max(1e-9);
    FLATTEN_TOLERANCE_PX / max_scale
}

#[inline]
fn flatten_into<F: FnMut(KPoint, KPoint)>(
    path: &BezPath,
    transform: &KAffine,
    mut emit: F,
) {
    let tol = screen_flatten_tolerance(transform);
    let mut cur = KPoint::ZERO;
    let mut sub_start = KPoint::ZERO;
    kurbo::flatten(path.elements().iter().copied(), tol, |el| match el {
        PathEl::MoveTo(p) => {
            cur = *transform * p;
            sub_start = cur;
        }
        PathEl::LineTo(p) => {
            let to = *transform * p;
            emit(cur, to);
            cur = to;
        }
        PathEl::ClosePath => {
            if (cur.x - sub_start.x).abs() > CLOSE_EPS
                || (cur.y - sub_start.y).abs() > CLOSE_EPS
            {
                emit(cur, sub_start);
            }
            cur = sub_start;
        }
        // flatten never emits QuadTo/CurveTo.
        _ => {}
    });
}

pub(crate) fn fill_path_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    path:   &BezPath,
    rule:   FillRule,
    color:  Color,
    transform: &Affine,
) {
    let premul = color_to_premul(color);
    fill_path_generic(pixmap, clip, path, rule, transform, |_, _| premul);
}

/// Same scanline/coverage/AA machinery as [`fill_path_aa`] (see this
/// module's own doc comment, incl. [`VERTICAL_SUBSAMPLES`]'s 4x
/// vertical supersampling), but sources the fill colour PER PIXEL from
/// a [`crate::gradient::GradientSampler`] instead of one flat
/// premultiplied colour — coordinator's 2026-07-25 fix: a `FillPath`
/// with a `Brush::Gradient` used to flatten unconditionally to the
/// brush's first stop (`color::brush_to_color`), the same fallback
/// EVERY non-`FillRect` primitive on this backend still uses for a
/// gradient brush; `FillRect` was the only command that ever consulted
/// a real gradient. The gradient colour is evaluated per PIXEL (not
/// per sub-scanline) — coverage and colour are independent concerns,
/// the same split `fill_rect_gradient_aa` already uses.
pub(crate) fn fill_path_gradient_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    path:   &BezPath,
    rule:   FillRule,
    gradient: &uzor_urx_core::math::Gradient,
    transform: &Affine,
) {
    let sampler = crate::gradient::GradientSampler::new(gradient, transform);
    fill_path_generic(pixmap, clip, path, rule, transform, |cx, cy| sampler.sample_premul(cx, cy));
}

/// Shared scanline fill core for [`fill_path_aa`]/[`fill_path_gradient_aa`]
/// — everything through per-pixel COVERAGE (the AET scan, the 4x
/// vertical supersampling, horizontal `axis_coverage` AA, the
/// rounded-clip mask consult) is IDENTICAL for a solid or gradient
/// fill; only the colour differs, resolved by `color_at` once per
/// touched pixel at its DEVICE-space centre (`px + 0.5, py + 0.5`) —
/// never per sub-scanline, matching `fill_rect_gradient_aa`'s own
/// "coverage and colour are orthogonal" structure.
fn fill_path_generic<F: FnMut(f32, f32) -> [u8; 4]>(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    path:   &BezPath,
    rule:   FillRule,
    transform: &Affine,
    mut color_at: F,
) {
    if path.elements().is_empty() { return; }

    let k_xform = KAffine::new(transform.as_coeffs());

    // Collect screen-space segments + bbox in a single pass.
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut segments: Vec<(KPoint, KPoint)> = Vec::with_capacity(64);
    flatten_into(path, &k_xform, |a, b| {
        min_x = min_x.min(a.x).min(b.x);
        min_y = min_y.min(a.y).min(b.y);
        max_x = max_x.max(a.x).max(b.x);
        max_y = max_y.max(a.y).max(b.y);
        segments.push((a, b));
    });

    if segments.is_empty() { return; }

    let path_bbox = Rect::new(min_x, min_y, max_x, max_y);
    let cur_clip = clip.current();
    let visible = path_bbox.intersect(cur_clip);
    if visible.width() <= 0.0 || visible.height() <= 0.0 { return; }

    let w = pixmap.width()  as i64;
    let h = pixmap.height() as i64;
    let ix0 = (visible.x0.floor() as i64).max(0);
    let iy0 = (visible.y0.floor() as i64).max(0);
    let ix1 = (visible.x1.ceil()  as i64).min(w);
    let iy1 = (visible.y1.ceil()  as i64).min(h);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    // Build edges + per-SUB-SCANLINE activation buckets. Each pixel row is
    // sampled `VERTICAL_SUBSAMPLES` times (see that constant's own doc
    // comment for why) — the activation grid is built at that finer
    // granularity so an edge whose `y_min` falls partway through a row
    // still activates on its own correct sub-scanline, not the whole row.
    let row_count = (iy1 - iy0) as usize;
    let ss = VERTICAL_SUBSAMPLES;
    let sub_h = 1.0 / ss as f64;
    let total_sub_rows = row_count * ss;
    let mut all_edges: Vec<Edge> = Vec::with_capacity(segments.len());
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); total_sub_rows];
    for (p0, p1) in &segments {
        if (p1.y - p0.y).abs() < 1e-9 { continue; } // skip horizontal
        let (y0, y1, x0, _x1, dir) = if p0.y < p1.y {
            (p0.y, p1.y, p0.x, p1.x, 1)
        } else {
            (p1.y, p0.y, p1.x, p0.x, -1)
        };
        let dx_dy = if p0.y < p1.y {
            (p1.x - p0.x) / (p1.y - p0.y)
        } else {
            (p0.x - p1.x) / (p0.y - p1.y)
        };
        // Activate on the first sub-scanline with sample_y > y_min, i.e.
        // the smallest `vr` (0-based sub-row index from `iy0`) such that
        // `iy0 + vr*sub_h + sub_h/2 > y0` — the same
        // "ceil(value - half_step)" activation technique the pre-existing
        // one-sample-per-row code used, generalized to `sub_h` steps.
        let activate_vr = ((y0 - iy0 as f64) / sub_h - 0.5).ceil().max(0.0) as i64;
        if activate_vr >= total_sub_rows as i64 || y1 <= y0 { continue; }
        let edge_idx = all_edges.len() as u32;
        all_edges.push(Edge {
            y_min: y0,
            y_max: y1,
            x_at:  x0,  // recomputed precisely on activation
            dx_dy,
            dir,
        });
        let bi = activate_vr as usize;
        if bi < total_sub_rows {
            buckets[bi].push(edge_idx);
        }
    }
    if all_edges.is_empty() { return; }

    let cur_clip = clip.current();
    let clip_x0 = cur_clip.x0;
    let clip_x1 = cur_clip.x1;
    let use_mask = !clip.all_rect();

    let mut active: Vec<Edge> = Vec::with_capacity(16);
    // Per-row coverage accumulator (sum of 0..=255 `h_cov` samples across
    // all `ss` sub-scanlines) — reused across rows, cleared at the start
    // of each. Indexed by `px - ix0`.
    let row_w = (ix1 - ix0) as usize;
    let mut row_accum: Vec<u32> = vec![0; row_w];

    for py in iy0 .. iy1 {
        for c in row_accum.iter_mut() { *c = 0; }

        for sub in 0 .. ss {
            let vr = (py - iy0) as usize * ss + sub;
            let sample_y = iy0 as f64 + vr as f64 * sub_h + sub_h * 0.5;

            // Drop edges whose y_max has been crossed.
            active.retain(|e| e.y_max > sample_y);

            // Activate edges scheduled for this sub-scanline.
            for &eidx in &buckets[vr] {
                let mut e = all_edges[eidx as usize];
                e.x_at = e.x_at + (sample_y - e.y_min) * e.dx_dy;
                active.push(e);
            }

            if active.is_empty() { continue; }

            // Insertion sort by x_at (active is small & near-sorted).
            for i in 1..active.len() {
                let mut j = i;
                while j > 0 && active[j - 1].x_at > active[j].x_at {
                    active.swap(j - 1, j);
                    j -= 1;
                }
            }

            // Walk by winding rule.
            let mut winding: i32 = 0;
            let mut span_start: Option<f64> = None;
            for edge in active.iter() {
                let prev = winding;
                winding += edge.dir;
                let was_inside = match rule {
                    FillRule::NonZero => prev != 0,
                    FillRule::EvenOdd => (prev & 1) != 0,
                };
                let now_inside = match rule {
                    FillRule::NonZero => winding != 0,
                    FillRule::EvenOdd => (winding & 1) != 0,
                };
                if !was_inside && now_inside {
                    span_start = Some(edge.x_at);
                } else if was_inside && !now_inside {
                    if let Some(s) = span_start.take() {
                        let e_x = edge.x_at;
                        let span_x0 = s.max(clip_x0).max(ix0 as f64);
                        let span_x1 = e_x.min(clip_x1).min(ix1 as f64);
                        if span_x1 > span_x0 {
                            let ipx0 = span_x0.floor() as i64;
                            let ipx1 = (span_x1.ceil() as i64).min(w);
                            for px in ipx0 .. ipx1 {
                                let h_cov = crate::fill::axis_coverage(
                                    px as f64, px as f64 + 1.0,
                                    span_x0, span_x1,
                                );
                                if h_cov == 0 { continue; }
                                row_accum[(px - ix0) as usize] += h_cov as u32;
                            }
                        }
                    }
                }
            }

            // Advance x_at for the next sub-scanline.
            for e in active.iter_mut() {
                e.x_at += e.dx_dy * sub_h;
            }
        }

        // Blend this row once — average the `ss` sub-scanline coverage
        // samples, then apply the clip mask exactly as the pre-existing
        // single-sample code did.
        for px in ix0 .. ix1 {
            let raw = row_accum[(px - ix0) as usize];
            if raw == 0 { continue; }
            let mut cov = ((raw + ss as u32 / 2) / ss as u32).min(255) as u8;
            if use_mask {
                let m = clip.pixel_coverage(px, py);
                cov = ((cov as u32 * m as u32 + 127) / 255) as u8;
                if cov == 0 { continue; }
            }
            let premul = color_at(px as f32 + 0.5, py as f32 + 0.5);
            let src = premul_scale(premul, cov);
            pixmap.blend_pixel(px as u32, py as u32, src);
        }
    }
}

static STROKE_STYLE_WARNED: AtomicBool = AtomicBool::new(false);

pub(crate) fn stroke_path_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    path:   &BezPath,
    stroke: &Stroke,
    color:  Color,
    transform: &Affine,
) {
    if path.elements().is_empty() || stroke.width <= 0.0 { return; }

    // Collect screen-space segments first (separately per subpath) so
    // we can spot interior joints vs endpoints, then run the joiner.
    let k_xform = KAffine::new(transform.as_coeffs());
    let mut subpaths: Vec<Vec<KPoint>> = Vec::with_capacity(4);
    let mut cur_chain: Vec<KPoint> = Vec::with_capacity(32);
    let mut cur_pt = KPoint::ZERO;
    let mut sub_start = KPoint::ZERO;
    let tol = screen_flatten_tolerance(&k_xform);
    kurbo::flatten(path.elements().iter().copied(), tol, |el| match el {
        PathEl::MoveTo(p) => {
            if !cur_chain.is_empty() {
                subpaths.push(std::mem::take(&mut cur_chain));
            }
            cur_pt = k_xform * p;
            sub_start = cur_pt;
            cur_chain.push(cur_pt);
        }
        PathEl::LineTo(p) => {
            let to = k_xform * p;
            cur_chain.push(to);
            cur_pt = to;
        }
        PathEl::ClosePath => {
            if (cur_pt.x - sub_start.x).abs() > CLOSE_EPS
                || (cur_pt.y - sub_start.y).abs() > CLOSE_EPS
            {
                cur_chain.push(sub_start);
            }
            if !cur_chain.is_empty() {
                cur_chain.push(KPoint::ZERO); // sentinel: closed
                subpaths.push(std::mem::take(&mut cur_chain));
            }
            cur_pt = sub_start;
        }
        _ => {}
    });
    if !cur_chain.is_empty() {
        subpaths.push(cur_chain);
    }

    let half: f64 = stroke.width as f64 * 0.5;
    let miter_limit = stroke.miter_limit.max(1.0) as f64;
    let cap = stroke.cap;
    let join = stroke.join;

    // Track unsupported-style warning ONLY once for the whole program.
    if !matches!(cap, LineCap::Butt | LineCap::Round | LineCap::Square)
        && !STROKE_STYLE_WARNED.swap(true, Ordering::Relaxed)
    {
        #[cfg(debug_assertions)]
        eprintln!("[urx] unknown stroke cap {:?} → butt", cap);
    }

    for chain in &subpaths {
        let closed = chain.last().map(|p| p.x == 0.0 && p.y == 0.0).unwrap_or(false);
        let pts: &[KPoint] = if closed { &chain[..chain.len() - 1] } else { chain };
        if pts.len() < 2 { continue; }

        // Segments — flat-butt ends so the cap/join logic below has
        // exclusive control over endpoint shape.
        for w in pts.windows(2) {
            crate::stroke::stroke_line_aa_butt(
                pixmap, clip,
                uzor_urx_core::math::Vec2 { x: w[0].x, y: w[0].y },
                uzor_urx_core::math::Vec2 { x: w[1].x, y: w[1].y },
                stroke.width, color, &Affine::IDENTITY,
            );
        }

        // Joints between consecutive segments.
        let joint_indices: Vec<usize> = if closed {
            (0..pts.len()).collect()
        } else {
            (1..pts.len() - 1).collect()
        };
        for &i in &joint_indices {
            let p_prev = if closed && i == 0 { pts[pts.len() - 1] } else { pts[i - 1] };
            let p_cur  = pts[i];
            let p_next = if closed && i == pts.len() - 1 { pts[0] } else if i + 1 < pts.len() { pts[i + 1] } else { continue; };
            draw_joint(pixmap, clip, p_prev, p_cur, p_next, half, miter_limit, join, color);
        }

        // Caps for open subpaths.
        if !closed {
            let first = pts[0];
            let second = pts[1];
            let last = pts[pts.len() - 1];
            let prev_last = pts[pts.len() - 2];
            draw_cap(pixmap, clip, first, second, half, cap, color, /* end */ false);
            draw_cap(pixmap, clip, last, prev_last, half, cap, color, /* end */ true);
        }
    }
}

#[inline]
fn unit(dx: f64, dy: f64) -> (f64, f64) {
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 { (0.0, 0.0) } else { (dx / len, dy / len) }
}

fn draw_joint(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    prev:   KPoint,
    pivot:  KPoint,
    next:   KPoint,
    half:   f64,
    miter_limit: f64,
    join:   LineJoin,
    color:  Color,
) {
    let (vx_in, vy_in) = unit(pivot.x - prev.x, pivot.y - prev.y);
    let (vx_out, vy_out) = unit(next.x - pivot.x, next.y - pivot.y);
    if (vx_in == 0.0 && vy_in == 0.0) || (vx_out == 0.0 && vy_out == 0.0) { return; }

    // Perpendiculars (outward normals on the same side).
    let (nx_in, ny_in)   = (-vy_in,  vx_in);
    let (nx_out, ny_out) = (-vy_out, vx_out);
    // Outer side of the joint = where in/out normals point AWAY from each
    // other (the "open" side after a corner turn).
    let dot = nx_in * nx_out + ny_in * ny_out;
    // For very straight joints (dot ≈ 1) no joint geometry needed —
    // the two capsules already overlap cleanly.
    if dot > 0.9999 { return; }

    match join {
        LineJoin::Round => {
            crate::stroke::stroke_disc_aa(
                pixmap, clip,
                uzor_urx_core::math::Vec2 { x: pivot.x, y: pivot.y },
                half, color,
            );
        }
        LineJoin::Bevel => {
            // Triangle [pivot, pivot+n_in*half, pivot+n_out*half] on
            // outer side. We do both sides — only one is "outer"
            // depending on turn direction; the other is fully inside
            // the capsule overlap and benign.
            for sign in [1.0_f64, -1.0] {
                let mut tri = BezPath::new();
                tri.move_to((pivot.x, pivot.y));
                tri.line_to((pivot.x + sign * nx_in  * half, pivot.y + sign * ny_in  * half));
                tri.line_to((pivot.x + sign * nx_out * half, pivot.y + sign * ny_out * half));
                tri.close_path();
                fill_path_aa(pixmap, clip, &tri, FillRule::NonZero, color, &Affine::IDENTITY);
            }
        }
        LineJoin::Miter => {
            // Miter point = intersection of the two offset lines.
            // Length cap by miter_limit: if extension > limit*half_w,
            // fall back to bevel.
            // Cosine of half-angle between incoming and outgoing.
            let cos_half = ((1.0 + dot) * 0.5).max(0.0).sqrt();
            if cos_half < 1e-6 {
                // 180° turn — silly; just bevel.
                draw_joint(pixmap, clip, prev, pivot, next, half, miter_limit,
                           LineJoin::Bevel, color);
                return;
            }
            let miter_len = half / cos_half;
            if miter_len > miter_limit * half {
                draw_joint(pixmap, clip, prev, pivot, next, half, miter_limit,
                           LineJoin::Bevel, color);
                return;
            }
            // Miter direction = sum of two outward normals, normalised.
            for sign in [1.0_f64, -1.0] {
                let mx = sign * (nx_in + nx_out);
                let my = sign * (ny_in + ny_out);
                let (umx, umy) = unit(mx, my);
                if umx == 0.0 && umy == 0.0 { continue; }
                let apex_x = pivot.x + umx * miter_len;
                let apex_y = pivot.y + umy * miter_len;
                let mut quad = BezPath::new();
                quad.move_to((pivot.x, pivot.y));
                quad.line_to((pivot.x + sign * nx_in  * half, pivot.y + sign * ny_in  * half));
                quad.line_to((apex_x, apex_y));
                quad.line_to((pivot.x + sign * nx_out * half, pivot.y + sign * ny_out * half));
                quad.close_path();
                fill_path_aa(pixmap, clip, &quad, FillRule::NonZero, color, &Affine::IDENTITY);
            }
        }
    }
}

fn draw_cap(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    endpoint: KPoint,
    other:    KPoint,
    half:     f64,
    cap:      LineCap,
    color:    Color,
    is_end:   bool,
) {
    let _ = is_end;
    match cap {
        LineCap::Butt => {} // capsule already gives flat butt end
        LineCap::Round => {
            crate::stroke::stroke_disc_aa(
                pixmap, clip,
                uzor_urx_core::math::Vec2 { x: endpoint.x, y: endpoint.y },
                half, color,
            );
        }
        LineCap::Square => {
            // Extend by half_w along the segment direction; draw a
            // small rect at the end.
            let (ux, uy) = unit(other.x - endpoint.x, other.y - endpoint.y);
            if ux == 0.0 && uy == 0.0 { return; }
            let (rx, ry) = (-uy * half, ux * half); // perpendicular
            let extra_x = -ux * half;
            let extra_y = -uy * half;
            let mut sq = BezPath::new();
            sq.move_to((endpoint.x + rx,            endpoint.y + ry));
            sq.line_to((endpoint.x - rx,            endpoint.y - ry));
            sq.line_to((endpoint.x + extra_x - rx,  endpoint.y + extra_y - ry));
            sq.line_to((endpoint.x + extra_x + rx,  endpoint.y + extra_y + ry));
            sq.close_path();
            fill_path_aa(pixmap, clip, &sq, FillRule::NonZero, color, &Affine::IDENTITY);
        }
    }
}
