//! Encode `uzor_urx_core::scene::Scene` into a flat `Vec<SceneCmd>`.
//! Walks DrawCommand variants and emits Rect commands for `FillRect`
//! with a Solid Color brush. Skips anything else (v1.6.0 first-stage
//! is rect-only — gradient/glyph/path land in subsequent stages).
//!
//! The output is ready to upload directly to a GPU storage buffer
//! (`bytemuck::cast_slice(&cmds[..])`).

use uzor_urx_core::math::Brush;
use uzor_urx_core::scene::{DrawCommand, Scene};

use crate::cmd::SceneCmd;

/// Encode a `Scene` into a flat list of GPU-uploadable `SceneCmd`s.
///
/// Only `FillRect` commands with a `Brush::Solid` are emitted in v1.6.0.
/// All other draw commands are silently skipped.
pub fn encode_scene(scene: &Scene) -> Vec<SceneCmd> {
    let mut out = Vec::with_capacity(scene.commands.len());
    for cmd in scene.commands.iter() {
        match cmd {
            DrawCommand::FillRect { rect, radii: _, brush, transform: _ } => {
                if let Brush::Solid(c) = brush {
                    out.push(SceneCmd::rect(
                        rect.x0 as f32, rect.y0 as f32,
                        rect.x1 as f32, rect.y1 as f32,
                        [c.r, c.g, c.b, c.a],
                    ));
                }
            }
            _ => {
                // Other commands — gradient / glyph / path / clip / stroke —
                // are not yet encoded. v1.6.0 first-stage is rect-only.
            }
        }
    }
    out
}
