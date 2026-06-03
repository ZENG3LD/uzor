//! Encode `uzor_urx_core::scene::Scene` into a flat `Vec<SceneCmd>`.
//! Walks DrawCommand variants and emits GPU-uploadable commands for
//! `FillRect` with Solid, LinearGradient, and RadialGradient brushes.
//! Sweep gradients and other command types are silently skipped.
//!
//! The output is ready to upload directly to a GPU storage buffer
//! (`bytemuck::cast_slice(&cmds[..])`).

use uzor_urx_core::math::{Brush, GradientKind};
use uzor_urx_core::scene::{DrawCommand, LineCap, Scene};

use crate::cmd::{cap_kind, lin_dir, SceneCmd};

/// Snap a (dx, dy) direction vector to the nearest of the 4 supported
/// linear gradient direction constants.
///
/// Horizontal axis (|dx| >= |dy|) → `HORIZONTAL` (L→R).
/// Vertical axis   (|dy| >  |dx|) → `VERTICAL`   (T→B).
/// When axes are equal (diagonal intent): sign of dy determines TL→BR vs BL→TR.
fn snap_direction(dx: f64, dy: f64) -> u32 {
    let ax = dx.abs();
    let ay = dy.abs();
    if ax > ay {
        lin_dir::HORIZONTAL
    } else if ay > ax {
        lin_dir::VERTICAL
    } else if dy >= 0.0 {
        lin_dir::DIAGONAL_TLBR
    } else {
        lin_dir::DIAGONAL_BLTR
    }
}

/// Map a [`LineCap`] enum to the GPU-side `cap_kind` constant.
fn cap_to_gpu(c: LineCap) -> u32 {
    match c {
        LineCap::Butt   => cap_kind::BUTT,
        LineCap::Round  => cap_kind::ROUND,
        LineCap::Square => cap_kind::SQUARE,
    }
}

/// Encode a `Scene` into a flat list of GPU-uploadable `SceneCmd`s.
///
/// Convenience wrapper around [`encode_scene_with_paths`] for scenes
/// that contain no `DrawCommand::StrokePath` cmds (no flattened paths).
///
/// Supported sources:
/// - `DrawCommand::FillRect { Brush::Solid }`             → `SceneCmd::rect`
/// - `DrawCommand::FillRect { Brush::Gradient(Linear) }`  → `SceneCmd::lin_gradient`
/// - `DrawCommand::FillRect { Brush::Gradient(Radial) }`  → `SceneCmd::rad_gradient`
/// - `DrawCommand::Line  { stroke: Stroke, brush: Solid }`→ `SceneCmd::stroke`
///
/// Unsupported variants (Sweep gradients, StrokeRect, StrokePath, FillPath,
/// GlyphRun, Image, Clip) are silently skipped — they require either Path
/// tessellation (Tier C) or glyph atlas integration that the engine consumer
/// is expected to drive via `SceneCmd::glyph` directly.
pub fn encode_scene(scene: &Scene) -> Vec<SceneCmd> {
    let (cmds, _points) = encode_scene_with_paths(scene, 0);
    cmds
}

/// Encode a `Scene`, also producing the `path_points` vertex array
/// referenced by any emitted `SceneCmd::Path` cmds.
///
/// `point_offset_base` is the index in the consumer's global
/// `path_points` buffer where this scene's points will be uploaded;
/// emitted Path cmds reference indices starting from there.
///
/// Returns `(cmds, points)` — both lists are in painter order; points
/// for a Path cmd at `cmds[i]` live at `points[offset_local..offset_local+count]`
/// where `offset_local = path_params(cmds[i]).offset - point_offset_base`.
pub fn encode_scene_with_paths(
    scene: &Scene,
    point_offset_base: u32,
) -> (Vec<SceneCmd>, Vec<[f32; 2]>) {
    let mut out    = Vec::with_capacity(scene.commands.len());
    let mut points = Vec::<[f32; 2]>::new();
    for cmd in scene.commands.iter() {
        match cmd {
            DrawCommand::FillRect { rect, radii: _, brush, transform: _ } => {
                let x0 = rect.x0 as f32;
                let y0 = rect.y0 as f32;
                let x1 = rect.x1 as f32;
                let y1 = rect.y1 as f32;

                match brush {
                    Brush::Solid(c) => {
                        out.push(SceneCmd::rect(x0, y0, x1, y1, [c.r, c.g, c.b, c.a]));
                    }
                    Brush::Gradient(g) => match g.kind {
                        GradientKind::Linear { start, end } => {
                            if g.stops.is_empty() {
                                continue;
                            }
                            let first = g.stops.first().map(|s| s.color).unwrap_or_default();
                            let last  = g.stops.last().map(|s| s.color).unwrap_or_default();

                            let dx = end.x - start.x;
                            let dy = end.y - start.y;
                            let direction = snap_direction(dx, dy);

                            out.push(SceneCmd::lin_gradient(
                                x0, y0, x1, y1,
                                [first.r, first.g, first.b, first.a],
                                [last.r,  last.g,  last.b,  last.a],
                                direction,
                            ));
                        }
                        GradientKind::Radial { .. } => {
                            if g.stops.is_empty() {
                                continue;
                            }
                            let inner = g.stops.first().map(|s| s.color).unwrap_or_default();
                            let outer = g.stops.last().map(|s| s.color).unwrap_or_default();

                            out.push(SceneCmd::rad_gradient(
                                x0, y0, x1, y1,
                                [inner.r, inner.g, inner.b, inner.a],
                                [outer.r, outer.g, outer.b, outer.a],
                            ));
                        }
                        GradientKind::Sweep { .. } => {
                            // Sweep gradients not yet supported in GPU pipeline v1.6.x.
                        }
                    },
                    Brush::Image(_) => {
                        // Image brushes not yet encoded.
                    }
                }
            }
            DrawCommand::Line { from, to, stroke, brush, transform: _ } => {
                // Only solid brush supported; gradient/image along a stroke
                // are out of scope until Path / GlyphRun encoding lands.
                let color = match brush {
                    Brush::Solid(c) => [c.r, c.g, c.b, c.a],
                    _ => continue,
                };
                if !(stroke.width > 0.0) {
                    continue;
                }
                out.push(SceneCmd::stroke(
                    from.x as f32, from.y as f32,
                    to.x   as f32, to.y   as f32,
                    stroke.width,
                    color,
                    cap_to_gpu(stroke.cap),
                ));
            }
            DrawCommand::StrokePath { path, stroke, brush, transform: _ } => {
                // Flatten the Bézier path into a polyline at `flatten`
                // tolerance (0.25 px). Each `MoveTo` starts a new
                // sub-path → emit one Path cmd per sub-path. Tolerance
                // 0.25 keeps curves visually smooth without exploding
                // point counts (a typical quad-Bézier flattens to 6-12
                // segments at this tolerance).
                let color = match brush {
                    Brush::Solid(c) => [c.r, c.g, c.b, c.a],
                    _ => continue,
                };
                if !(stroke.width > 0.0) { continue; }

                // We use peniko's BezPath re-export from kurbo — same type.
                let mut sub_start_local = points.len();
                let mut sub_count = 0usize;
                let mut x_min = f32::INFINITY;
                let mut y_min = f32::INFINITY;
                let mut x_max = f32::NEG_INFINITY;
                let mut y_max = f32::NEG_INFINITY;
                let half_w = (stroke.width * 0.5).max(0.5);

                let flush_sub = |sub_start: usize, sub_count: usize,
                                     x_min: f32, y_min: f32, x_max: f32, y_max: f32,
                                     out: &mut Vec<SceneCmd>| {
                    if sub_count < 2 { return; }
                    let bbox = [x_min - half_w, y_min - half_w, x_max + half_w, y_max + half_w];
                    let global_offset = point_offset_base + sub_start as u32;
                    out.push(SceneCmd::path(
                        bbox, color, stroke.width,
                        global_offset, sub_count as u32,
                    ));
                };

                let push_pt = |px: f32, py: f32,
                                   points: &mut Vec<[f32; 2]>,
                                   sub_count: &mut usize,
                                   x_min: &mut f32, y_min: &mut f32,
                                   x_max: &mut f32, y_max: &mut f32| {
                    points.push([px, py]);
                    *sub_count += 1;
                    if px < *x_min { *x_min = px; }
                    if py < *y_min { *y_min = py; }
                    if px > *x_max { *x_max = px; }
                    if py > *y_max { *y_max = py; }
                };

                kurbo::flatten(path.iter(), 0.25, |el: kurbo::PathEl| {
                    match el {
                        kurbo::PathEl::MoveTo(pt) => {
                            flush_sub(sub_start_local, sub_count,
                                      x_min, y_min, x_max, y_max, &mut out);
                            sub_start_local = points.len();
                            sub_count = 0;
                            x_min = f32::INFINITY; y_min = f32::INFINITY;
                            x_max = f32::NEG_INFINITY; y_max = f32::NEG_INFINITY;
                            push_pt(pt.x as f32, pt.y as f32,
                                    &mut points, &mut sub_count,
                                    &mut x_min, &mut y_min,
                                    &mut x_max, &mut y_max);
                        }
                        kurbo::PathEl::LineTo(pt) => {
                            push_pt(pt.x as f32, pt.y as f32,
                                    &mut points, &mut sub_count,
                                    &mut x_min, &mut y_min,
                                    &mut x_max, &mut y_max);
                        }
                        kurbo::PathEl::ClosePath => {
                            // Close = line back to sub-path start.
                            if sub_count >= 1 {
                                let p0 = points[sub_start_local];
                                push_pt(p0[0], p0[1],
                                        &mut points, &mut sub_count,
                                        &mut x_min, &mut y_min,
                                        &mut x_max, &mut y_max);
                            }
                        }
                        // kurbo::flatten guarantees only MoveTo/LineTo/ClosePath
                        // reach this callback — quadratic/cubic Béziers get
                        // turned into LineTo runs by the flattener itself.
                        _ => {}
                    }
                });
                flush_sub(sub_start_local, sub_count,
                          x_min, y_min, x_max, y_max, &mut out);
            }
            _ => {
                // StrokeRect, FillPath, GlyphRun, Image, Clip — not yet
                // encoded. FillPath needs interior fill (different SDF);
                // it lands in a future Tier C wave.
            }
        }
    }
    (out, points)
}
