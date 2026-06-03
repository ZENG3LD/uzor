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
    let mut out = Vec::with_capacity(scene.commands.len());
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
            _ => {
                // StrokeRect, StrokePath, FillPath, GlyphRun, Image, Clip —
                // not yet encoded.
            }
        }
    }
    out
}
