//! Node hit-testing — screen-space, sharing the exact same
//! `Camera2D::world_to_screen` + `Camera2D::node_screen_radius` pair the
//! renderer uses. This is the fix for the rejected MVP's root cause
//! (design doc §0.3/§4.2): render and hit-test independently clamped
//! zoom at different floors there, so the visibly-implied clickable
//! area and the actually-hit-tested area silently diverged. Doing the
//! comparison in screen space with the shared helper makes that class
//! of bug structurally impossible — there is only one number.

use uzor::types::Rect;

use crate::camera::Camera2D;
use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;

/// Extra screen-space slack added to a node's paint radius so a click
/// near — not just exactly on — a small node still lands.
pub const HOVER_TOLERANCE_PX: f64 = 6.0;

/// Nearest node under `cursor_screen` among `candidates`, or `None` if
/// the cursor isn't within `node_screen_radius + HOVER_TOLERANCE_PX` of
/// any of them. Ties broken by whichever candidate the cursor is deepest
/// inside (most negative slack).
pub fn nearest_node<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    camera: &Camera2D,
    viewport: Rect,
    cursor_screen: (f64, f64),
    candidates: &[NodeIndex],
) -> Option<NodeIndex> {
    let mut best: Option<(NodeIndex, f64)> = None;
    for &id in candidates {
        let Some(p) = particles.get(id.index()) else { continue };
        let Some(node) = graph.get_node(id) else { continue };
        let screen = camera.world_to_screen((p.x as f64, p.y as f64), viewport);
        let r = camera.node_screen_radius(node.radius);
        let dx = cursor_screen.0 - screen.0;
        let dy = cursor_screen.1 - screen.1;
        let dist = (dx * dx + dy * dy).sqrt();
        let slack = dist - r - HOVER_TOLERANCE_PX;
        if slack <= 0.0 {
            match best {
                Some((_, best_slack)) if best_slack <= slack => {}
                _ => best = Some((id, slack)),
            }
        }
    }
    best.map(|(id, _)| id)
}
