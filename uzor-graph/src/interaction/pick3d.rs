//! 3D screen-space projection + CPU ray picking (W3D arc plan
//! §1.3/§1.5/§2). [`project_world_to_screen`] (Wave 2) feeds the label
//! overlay (project a node's world position, hand the screen point to
//! the existing `label_grid.rs` LOD pass — see
//! [`crate::engine3d::GraphEngine3D::visible_labels`]).
//! [`screen_to_ray`]/[`nearest_node_3d`] (Wave 3) are the CPU
//! ray-vs-sphere picking primitive
//! [`crate::engine3d::GraphEngine3D::on_event`] hover/click-select wires
//! into — v1 per plan §1.5, escalated to GPU color-ID picking above
//! `GPU_PICK_NODE_THRESHOLD` nodes in Wave 4.

use glam::{Vec3, Vec4};

use uzor::types::Rect;
use uzor_urx_3d::PerspectiveCamera;

use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;

/// Project a world-space point through `camera`'s view-projection into
/// screen-space pixel coordinates within `viewport`. `None` when the
/// point is behind (or exactly at) the eye.
///
/// For a right-handed perspective projection the clip-space `w`
/// component equals `-view_space.z`; a point in front of the camera has
/// negative view-space `z` (the camera looks down `-Z` in its own view
/// space), so `w > 0` for anything actually visible. `w <= 0` means "at
/// or behind the eye" — there's no meaningful screen position for it.
///
/// NDC → screen applies a Y-flip: `uzor-urx-3d`'s NDC top (`+1`) is the
/// TOP of the render target (WebGPU/wgpu convention — framebuffer row 0
/// is the top row, confirmed against `uzor-urx-3d/tests/cube_render.rs`'s
/// own readback, which reads texture rows in order with no flip). This is
/// the opposite of [`crate::camera::Camera2D::world_to_screen`]'s plain
/// linear map, which has no such axis-convention mismatch to correct for
/// — 2D world space has no inherent up/down beyond what the caller
/// assigns it.
pub fn project_world_to_screen(camera: &PerspectiveCamera, world: Vec3, viewport: Rect) -> Option<(f64, f64)> {
    let clip = camera.view_proj() * world.extend(1.0);
    if clip.w <= 1e-5 {
        return None;
    }
    let ndc_x = (clip.x / clip.w) as f64;
    let ndc_y = (clip.y / clip.w) as f64;
    let sx = viewport.x + (ndc_x * 0.5 + 0.5) * viewport.width;
    let sy = viewport.y + (0.5 - ndc_y * 0.5) * viewport.height;
    Some((sx, sy))
}

/// Screen-space cursor position -> world-space ray (`origin`, unit
/// `dir`) — the inverse of [`project_world_to_screen`]'s NDC mapping,
/// and the CPU picking primitive (plan §1.5/§4 Wave 3). Recovers NDC
/// `(x, y)` from `cursor` within `viewport` (undoing the same Y-flip
/// documented on [`project_world_to_screen`]), then unprojects two
/// points along that fixed screen ray — NDC `z = 0` (near) and `z = 1`
/// (far), matching `uzor-urx-3d`'s `glam::Mat4::perspective_rh` `[0, 1]`
/// wgpu-convention depth range, NOT OpenGL's `[-1, 1]` — through
/// `camera.view_proj().inverse()`.
///
/// `origin` is `camera.eye`, not the unprojected near-plane point: for a
/// pinhole perspective camera every point unprojected from the same
/// screen pixel (at any depth) is exactly collinear with the eye, so
/// `camera.eye` is both correct and numerically simpler than carrying
/// the near-plane intersection through as the ray's start.
pub fn screen_to_ray(camera: &PerspectiveCamera, viewport: Rect, cursor: (f64, f64)) -> (Vec3, Vec3) {
    let ndc_x = (((cursor.0 - viewport.x) / viewport.width) * 2.0 - 1.0) as f32;
    let ndc_y = (1.0 - ((cursor.1 - viewport.y) / viewport.height) * 2.0) as f32;
    let inv_view_proj = camera.view_proj().inverse();
    let near = inv_view_proj * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far = inv_view_proj * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near_world = near.truncate() / near.w;
    let far_world = far.truncate() / far.w;
    let dir = (far_world - near_world).normalize_or_zero();
    (camera.eye, dir)
}

/// World-space hit slack added to a node's paint radius (plan §1.5's
/// "HOVER-tolerance analog in world radius terms"). Deliberately NOT a
/// screen-space pixel tolerance like 2D's `pick::HOVER_TOLERANCE_PX`:
/// converting a fixed pixel slack into world units would need the exact
/// per-node distance-to-camera at pick time, one more moving part for a
/// v1 CPU picker that plan §1.5 already scopes as "sub-millisecond up to
/// ~10k nodes, escalate to GPU color-ID above that" (Wave 4). A fixed
/// world-space margin is simpler and self-consistent: it's more
/// forgiving (in screen pixels) for a near node than a far one — the
/// opposite of 2D's viewport-independent tolerance — an accepted v1
/// tradeoff, not a bug.
pub const PICK_RADIUS_SLACK_WORLD: f32 = 1.0;

/// Nearest positive-`t` intersection of the ray (`origin`, unit `dir`)
/// with the sphere (`center`, `radius`), or `None` on a miss. Standard
/// analytic ray-sphere solve (`dir` assumed pre-normalized by the
/// caller — [`nearest_node_3d`] normalizes once up front rather than
/// re-normalizing per candidate).
fn ray_sphere_hit(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_d = discriminant.sqrt();
    let t_near = -b - sqrt_d;
    let t_far = -b + sqrt_d;
    if t_near >= 0.0 {
        Some(t_near)
    } else if t_far >= 0.0 {
        // Ray origin sits inside the sphere — still a valid hit at the
        // exit point, `t_far`.
        Some(t_far)
    } else {
        None
    }
}

/// Nearest node among `candidates` whose sphere (`center = particle
/// position`, `radius = node.radius + PICK_RADIUS_SLACK_WORLD`) the ray
/// hits at the smallest positive `t`, or `None` if the ray misses every
/// candidate (plan §1.5) — O(candidates.len()) per call, the CPU
/// picking v1 cost the caller's own pointer-move-with-movement-guard
/// (`GraphEngine3D::on_event`) keeps off the hot per-event path.
pub fn nearest_node_3d<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    ray_origin: Vec3,
    ray_dir: Vec3,
    candidates: &[NodeIndex],
) -> Option<NodeIndex> {
    let dir = ray_dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let mut best: Option<(NodeIndex, f32)> = None;
    for &id in candidates {
        let Some(p) = particles.get(id.index()) else { continue };
        let Some(node) = graph.get_node(id) else { continue };
        let center = Vec3::new(p.x, p.y, p.z);
        let radius = node.radius + PICK_RADIUS_SLACK_WORLD;
        if let Some(t) = ray_sphere_hit(ray_origin, dir, center, radius) {
            match best {
                Some((_, best_t)) if best_t <= t => {}
                _ => best = Some((id, t)),
            }
        }
    }
    best.map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head_on_camera() -> PerspectiveCamera {
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, 1.0)
    }

    #[test]
    fn a_point_at_the_target_projects_to_the_viewport_center() {
        let camera = head_on_camera();
        let viewport = Rect::new(0.0, 0.0, 200.0, 200.0);

        let (sx, sy) = project_world_to_screen(&camera, Vec3::ZERO, viewport).expect("the target sits in front of the eye");

        assert!((sx - 100.0).abs() < 1e-3);
        assert!((sy - 100.0).abs() < 1e-3);
    }

    #[test]
    fn a_point_above_world_up_projects_above_screen_center() {
        let camera = head_on_camera();
        let viewport = Rect::new(0.0, 0.0, 200.0, 200.0);

        let (_sx, sy) =
            project_world_to_screen(&camera, Vec3::new(0.0, 2.0, 0.0), viewport).expect("visible in front of the eye");

        assert!(sy < 100.0, "a point above world-up must land above screen-center in screen (y-down) space, got y={sy}");
    }

    #[test]
    fn an_off_center_viewport_offsets_the_projection_by_its_own_origin() {
        let camera = head_on_camera();
        let viewport = Rect::new(50.0, 25.0, 200.0, 200.0);

        let (sx, sy) = project_world_to_screen(&camera, Vec3::ZERO, viewport).expect("visible");

        assert!((sx - 150.0).abs() < 1e-3);
        assert!((sy - 125.0).abs() < 1e-3);
    }

    #[test]
    fn a_point_behind_the_eye_returns_none() {
        let camera = head_on_camera();
        let viewport = Rect::new(0.0, 0.0, 200.0, 200.0);

        assert!(project_world_to_screen(&camera, Vec3::new(0.0, 0.0, 20.0), viewport).is_none());
    }

    // ── screen_to_ray / ray-sphere / nearest_node_3d (Wave 3) ──────────────

    fn viewport_200() -> Rect {
        Rect::new(0.0, 0.0, 200.0, 200.0)
    }

    type DemoGraph = Graph<(), ()>;

    fn one_node_graph(radius: f32) -> DemoGraph {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "x", radius);
        graph
    }

    #[test]
    fn screen_to_ray_at_viewport_center_points_from_the_eye_toward_the_target() {
        let camera = head_on_camera();
        let (origin, dir) = screen_to_ray(&camera, viewport_200(), (100.0, 100.0));

        assert!((origin - camera.eye).length() < 1e-4, "ray origin must be the camera eye");
        let expected_dir = (camera.target - camera.eye).normalize();
        assert!((dir - expected_dir).length() < 1e-3, "a screen-center ray must point straight at the look-at target");
    }

    #[test]
    fn screen_to_ray_hits_a_known_sphere_at_a_known_distance() {
        // Eye at (0,0,10) looking at the origin; a radius-2 sphere sits
        // exactly at the target. The screen-center ray must hit it, and
        // the nearest intersection distance must be `eye_distance -
        // radius` (the ray enters the near face of the sphere first).
        let camera = head_on_camera();
        let (origin, dir) = screen_to_ray(&camera, viewport_200(), (100.0, 100.0));

        let hit = ray_sphere_hit(origin, dir, Vec3::ZERO, 2.0).expect("a screen-center ray must hit a sphere centered on the target");
        assert!((hit - 8.0).abs() < 1e-3, "expected t ~= 10.0 (eye distance) - 2.0 (radius) = 8.0, got {hit}");
    }

    #[test]
    fn screen_to_ray_at_a_corner_misses_a_small_target_centered_sphere() {
        let camera = head_on_camera();
        // Far corner of a narrow-fov viewport — well off the sphere's
        // silhouette for a small radius.
        let (origin, dir) = screen_to_ray(&camera, viewport_200(), (0.0, 0.0));

        assert!(ray_sphere_hit(origin, dir, Vec3::ZERO, 0.05).is_none(), "a corner ray must miss a tiny target-centered sphere");
    }

    #[test]
    fn nearest_node_3d_returns_none_when_the_ray_hits_no_candidate() {
        let graph = one_node_graph(1.0);
        let particles = vec![Particle::at3(50.0, 50.0, 0.0)]; // well off the ray below
        let candidates = [NodeIndex(0)];

        let hit = nearest_node_3d(&graph, &particles, Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -1.0), &candidates);
        assert_eq!(hit, None);
    }

    #[test]
    fn nearest_node_3d_hits_a_sphere_directly_on_the_ray() {
        let graph = one_node_graph(1.0);
        let particles = vec![Particle::at3(0.0, 0.0, 0.0)];
        let candidates = [NodeIndex(0)];

        let hit = nearest_node_3d(&graph, &particles, Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -1.0), &candidates);
        assert_eq!(hit, Some(NodeIndex(0)));
    }

    #[test]
    fn nearest_node_3d_picks_the_closer_of_two_candidates_on_the_same_ray() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "near", "x", 1.0);
        graph.push_node((), "far", "x", 1.0);
        let particles = vec![Particle::at3(0.0, 0.0, 5.0), Particle::at3(0.0, 0.0, -5.0)];
        let candidates = [NodeIndex(0), NodeIndex(1)];

        let hit = nearest_node_3d(&graph, &particles, Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -1.0), &candidates);
        assert_eq!(hit, Some(NodeIndex(0)), "the nearer sphere along the ray must win, not just any hit");
    }

    #[test]
    fn nearest_node_3d_ignores_a_candidate_outside_the_provided_slice() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "on_ray", "x", 1.0);
        graph.push_node((), "excluded", "x", 1.0);
        let particles = vec![Particle::at3(100.0, 100.0, 0.0), Particle::at3(0.0, 0.0, 0.0)];
        // Only node 0 (off-ray) is a candidate — node 1 (on-ray) is
        // deliberately excluded from `candidates`, mirroring how the
        // engine restricts picking to its own visible/candidate set.
        let candidates = [NodeIndex(0)];

        let hit = nearest_node_3d(&graph, &particles, Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -1.0), &candidates);
        assert_eq!(hit, None, "a real hit outside the candidate slice must not be returned");
    }

    /// Wave 3 gate: project a node's world position to screen, then
    /// `screen_to_ray` + `nearest_node_3d` at that exact screen point
    /// must recover the SAME node — the round-trip [`project_world_to_screen`]
    /// promises for the label-overlay/picking pair.
    #[test]
    fn project_then_ray_pick_round_trip_recovers_the_same_node() {
        let graph = one_node_graph(3.0);
        let camera = PerspectiveCamera::new(Vec3::new(15.0, 8.0, 20.0), Vec3::ZERO, 800.0 / 600.0);
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let world = Vec3::new(4.0, -2.0, 1.0);
        let particles = vec![Particle::at3(world.x, world.y, world.z)];
        let candidates = [NodeIndex(0)];

        let screen = project_world_to_screen(&camera, world, viewport).expect("the fixture node sits in front of the eye");
        let (origin, dir) = screen_to_ray(&camera, viewport, screen);
        let hit = nearest_node_3d(&graph, &particles, origin, dir, &candidates);

        assert_eq!(hit, Some(NodeIndex(0)), "projecting a node then ray-picking at its own screen position must recover it");
    }
}
