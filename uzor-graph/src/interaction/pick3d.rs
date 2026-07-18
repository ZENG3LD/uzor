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
use uzor_urx_3d::{PerspectiveCamera, Renderer3D, Scene3D, Texture3D};

use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;
use crate::render3d;

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

/// Ray/plane intersection — the 3D node-drag primitive
/// ([`crate::engine3d::GraphEngine3D::on_event`]'s `Dragging` mode,
/// vasturiano `3d-force-graph`'s own convention): a dragged node moves
/// along a plane through its OWN current world position, with the plane
/// normal set to the camera's view direction ("camera-parallel drag") so
/// the node tracks the cursor at its own fixed depth instead of snapping
/// to whatever world surface the ray happens to cross first. `None` when
/// the ray is (near-)parallel to the plane, or when the intersection
/// falls behind the ray origin (`t < 0`).
pub fn ray_plane_intersection(ray_origin: Vec3, ray_dir: Vec3, plane_point: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let denom = ray_dir.dot(plane_normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (plane_point - ray_origin).dot(plane_normal) / denom;
    if t < 0.0 {
        return None;
    }
    Some(ray_origin + ray_dir * t)
}

// ── GPU color-ID picking escalation (Wave 4, plan §1.5/§4) ─────────────
//
// [`crate::engine3d::GraphEngine3D`] escalates from this module's own CPU
// ray-vs-sphere picking (above) to GPU color-ID picking once
// `graph.node_count()` exceeds [`GPU_PICK_NODE_THRESHOLD`] — the id-pass
// scene itself lives in [`crate::render3d::build_id_pass_scene`] (its own
// module doc has the full tonemap/encode writeup); this module owns the
// DEFERRED readback plumbing: a pure, `wgpu`-free state machine
// ([`GpuPickPipeline`], fully unit-testable) plus the real `wgpu`-driving
// functions ([`request_gpu_pick`]/[`poll_gpu_pick`]) that feed it,
// following the plan's own "non-blocking, result may land 1-2 frames
// late" design — a live pick must never stall the render loop.

/// Node-count threshold above which [`crate::engine3d::GraphEngine3D`]
/// escalates hover picking to the GPU color-ID pass (plan §1.5) — a
/// `const` DEFAULT, not a hard rule:
/// [`crate::engine3d::GraphEngine3D::set_gpu_pick_threshold`] overrides
/// it per-engine so a test can exercise the switch without a literal
/// >10k-node fixture.
pub const GPU_PICK_NODE_THRESHOLD: usize = 10_000;

/// In-progress/-resolved GPU pick request state (private — see
/// [`GpuPickPipeline`]'s own public surface).
#[derive(Debug, Clone, Copy, PartialEq)]
enum GpuPickPhase {
    Idle,
    InFlight { cursor: (f64, f64) },
    Ready { cursor: (f64, f64), result: Option<NodeIndex> },
}

impl Default for GpuPickPhase {
    fn default() -> Self {
        GpuPickPhase::Idle
    }
}

/// Deferred GPU color-ID pick request/result state machine (plan §1.5's
/// "non-blocking... result may land 1-2 frames late" design) —
/// deliberately holds NO `wgpu` resources of its own (no `Buffer`, no
/// `Receiver`) so it's fully unit-testable without a GPU: a test drives
/// it end-to-end via [`GpuPickPipeline::request`]/[`GpuPickPipeline::complete`]/
/// [`GpuPickPipeline::poll_consume`] directly (mocked/synchronous
/// completion). The real `wgpu` I/O ([`request_gpu_pick`]/[`poll_gpu_pick`])
/// owns its own [`GpuPickReadback`] handle SEPARATELY and calls
/// [`GpuPickPipeline::complete`] once its `map_async` callback has
/// actually fired.
#[derive(Debug, Default)]
pub struct GpuPickPipeline {
    phase: GpuPickPhase,
    requests_started: usize,
}

impl GpuPickPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_in_flight(&self) -> bool {
        matches!(self.phase, GpuPickPhase::InFlight { .. })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.phase, GpuPickPhase::Ready { .. })
    }

    /// Total number of requests [`GpuPickPipeline::request`] actually
    /// STARTED (not counting no-ops while already in flight) — the
    /// threshold-switch test's own call-counter probe.
    pub fn requests_started(&self) -> usize {
        self.requests_started
    }

    /// Begin a new request at `cursor`. `false` (no-op) while a previous
    /// request is still [`GpuPickPipeline::is_in_flight`] — one
    /// outstanding GPU readback at a time.
    pub fn request(&mut self, cursor: (f64, f64)) -> bool {
        if self.is_in_flight() {
            return false;
        }
        self.phase = GpuPickPhase::InFlight { cursor };
        self.requests_started += 1;
        true
    }

    /// Resolve the in-flight request (`InFlight -> Ready`) — the seam
    /// [`poll_gpu_pick`] calls once a real readback has actually
    /// completed, and a state-machine test drives directly with a
    /// synthetic result (no GPU needed). A no-op while not
    /// [`GpuPickPipeline::is_in_flight`].
    pub fn complete(&mut self, result: Option<NodeIndex>) {
        if let GpuPickPhase::InFlight { cursor } = self.phase {
            self.phase = GpuPickPhase::Ready { cursor, result };
        }
    }

    /// Consume a [`GpuPickPipeline::is_ready`] result (`Ready -> Idle`).
    /// The OUTER `Option` is "nothing to consume yet" (`Idle`/`InFlight`);
    /// the INNER `Option<NodeIndex>` is the GPU pick's own answer (`None`
    /// = background/no node under the cursor).
    pub fn poll_consume(&mut self) -> Option<Option<NodeIndex>> {
        if let GpuPickPhase::Ready { result, .. } = self.phase {
            self.phase = GpuPickPhase::Idle;
            Some(result)
        } else {
            None
        }
    }
}

/// Live `wgpu` resources for one in-flight GPU pick readback — owned by
/// the CALLER (not [`GpuPickPipeline`], which stays `wgpu`-free for
/// testability, see its own doc comment), kept alive across frames until
/// [`poll_gpu_pick`] resolves it.
pub struct GpuPickReadback {
    staging: wgpu::Buffer,
    rx: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
}

/// Minimum `wgpu` row-copy alignment (`COPY_BYTES_PER_ROW_ALIGNMENT`) —
/// the id-pass 1x1 readback still needs a full aligned row's worth of
/// staging space even though only the first 4 bytes (one RGBA pixel) are
/// ever read.
const READBACK_ROW_BYTES: u64 = 256;

/// Kick off a new GPU color-ID pick (plan §1.5/§4 Wave 4): renders
/// `id_scene` (see [`crate::render3d::build_id_pass_scene`]) into a
/// `viewport_px`-sized offscreen target, then copies ONLY the pixel under
/// `cursor` into a tiny staging buffer and issues a NON-BLOCKING
/// `map_async` — this call does not stall the render loop; poll
/// [`poll_gpu_pick`] on subsequent frames until it resolves. Disarms
/// bloom/SSAO on `renderer` first — see `render3d.rs`'s own id-pass
/// module doc for why (bloom bleed/AO darkening would corrupt the flat
/// per-node id color).
pub fn request_gpu_pick(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer3D,
    id_scene: &Scene3D,
    camera: &PerspectiveCamera,
    viewport_px: (u32, u32),
    cursor: (f64, f64),
) -> GpuPickReadback {
    let (w, h) = (viewport_px.0.max(1), viewport_px.1.max(1));
    let px = (cursor.0.round() as i64).clamp(0, w as i64 - 1) as u32;
    let py = (cursor.1.round() as i64).clamp(0, h as i64 - 1) as u32;

    renderer.set_bloom_strength(0.0);
    renderer.set_ssao_strength(0.0);

    let target = Texture3D::render_target(device, w, h);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("uzor-graph.gpu_pick.encoder") });
    renderer.render_to_texture(device, queue, &mut encoder, &target, camera, id_scene);

    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor-graph.gpu_pick.staging"),
        size: READBACK_ROW_BYTES,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target.texture, mip_level: 0, origin: wgpu::Origin3d { x: px, y: py, z: 0 }, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(READBACK_ROW_BYTES as u32), rows_per_image: Some(1) },
        },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    GpuPickReadback { staging, rx }
}

/// Non-blocking poll — call once per frame with the handle
/// [`request_gpu_pick`] returned. `device.poll(PollType::Poll)` never
/// blocks (the plan's own "must never stall the render loop"
/// requirement); returns `Some(decoded)` once the readback has actually
/// completed (decoded via [`crate::render3d::decode_gpu_pick_pixel`],
/// `None` inner meaning "background/no node"), or the outer `None` while
/// still in flight. A `map_async`/device-poll error also resolves as
/// `Some(None)` — a failed readback degrades to "no pick this cycle"
/// rather than panicking or hanging the caller forever.
pub fn poll_gpu_pick(device: &wgpu::Device, readback: &GpuPickReadback, node_count: u32) -> Option<Option<NodeIndex>> {
    let _ = device.poll(wgpu::PollType::Poll);
    match readback.rx.try_recv() {
        Ok(Ok(())) => {
            let slice = readback.staging.slice(..);
            let data = slice.get_mapped_range();
            let rgba = [data[0], data[1], data[2], data[3]];
            drop(data);
            readback.staging.unmap();
            Some(render3d::decode_gpu_pick_pixel(rgba, node_count))
        }
        Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(None),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
    }
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

    // ── Node drag: ray_plane_intersection ───────────────────────────────

    #[test]
    fn ray_plane_intersection_hits_a_plane_head_on() {
        // Ray straight down -Z from (0,0,10); plane at z=3, normal +Z.
        let hit = ray_plane_intersection(Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 3.0), Vec3::Z);
        assert_eq!(hit, Some(Vec3::new(0.0, 0.0, 3.0)));
    }

    #[test]
    fn ray_plane_intersection_returns_none_for_a_ray_parallel_to_the_plane() {
        // Ray travels along +X; plane normal is +Z (perpendicular to the
        // ray direction) — the ray never crosses the plane.
        let hit = ray_plane_intersection(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 3.0), Vec3::Z);
        assert_eq!(hit, None);
    }

    #[test]
    fn ray_plane_intersection_returns_none_when_the_plane_is_behind_the_ray_origin() {
        let hit = ray_plane_intersection(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 5.0), Vec3::Z);
        assert_eq!(hit, None, "the plane sits behind the ray's own direction of travel");
    }

    #[test]
    fn ray_plane_intersection_at_an_oblique_angle_lands_on_the_plane() {
        let origin = Vec3::new(0.0, 0.0, 10.0);
        let dir = Vec3::new(1.0, 1.0, -2.0).normalize();
        let plane_point = Vec3::new(0.0, 0.0, 0.0);
        let plane_normal = Vec3::Z;
        let hit = ray_plane_intersection(origin, dir, plane_point, plane_normal).expect("an oblique ray toward the plane must hit it");
        assert!((hit.z - 0.0).abs() < 1e-4, "the hit point must lie exactly on the plane (z=0)");
        // t = 5 along this specific direction (dz = -2/len over t=5*len -> dz=-10 matches origin.z=10 -> 0).
        assert!((hit - Vec3::new(5.0, 5.0, 0.0)).length() < 1e-3);
    }

    // ── Wave 4: GpuPickPipeline deferred-readback state machine ────────
    // Pure — no `wgpu`/GPU involved, per the task's own "mocked/
    // synchronous completion where GPU isn't needed" gate.

    #[test]
    fn gpu_pick_pipeline_starts_idle() {
        let p = GpuPickPipeline::new();
        assert!(!p.is_in_flight());
        assert!(!p.is_ready());
        assert_eq!(p.requests_started(), 0);
    }

    #[test]
    fn request_transitions_idle_to_in_flight_and_counts_the_request() {
        let mut p = GpuPickPipeline::new();
        assert!(p.request((10.0, 20.0)));
        assert!(p.is_in_flight());
        assert_eq!(p.requests_started(), 1);
    }

    #[test]
    fn a_second_request_while_already_in_flight_is_a_no_op_and_does_not_double_count() {
        let mut p = GpuPickPipeline::new();
        assert!(p.request((0.0, 0.0)));
        assert!(!p.request((5.0, 5.0)), "a request while already in flight must be rejected");
        assert_eq!(p.requests_started(), 1);
    }

    #[test]
    fn complete_then_poll_consume_resolves_the_result_and_resets_to_idle() {
        let mut p = GpuPickPipeline::new();
        p.request((1.0, 2.0));
        assert!(!p.is_ready());

        p.complete(Some(NodeIndex(7)));
        assert!(p.is_ready());
        assert!(!p.is_in_flight());

        assert_eq!(p.poll_consume(), Some(Some(NodeIndex(7))));
        assert!(!p.is_in_flight() && !p.is_ready(), "consuming a ready result must return to Idle");
        assert_eq!(p.poll_consume(), None, "nothing left to consume after the first poll_consume");
    }

    #[test]
    fn complete_with_none_resolves_to_a_background_pick_not_a_missing_result() {
        let mut p = GpuPickPipeline::new();
        p.request((0.0, 0.0));
        p.complete(None);
        assert_eq!(p.poll_consume(), Some(None), "a resolved-but-empty GPU pick must be Some(None), distinct from still-pending None");
    }

    #[test]
    fn complete_while_idle_is_a_no_op() {
        let mut p = GpuPickPipeline::new();
        p.complete(Some(NodeIndex(1)));
        assert!(!p.is_ready(), "completing with nothing in flight must not fabricate a ready result");
        assert_eq!(p.poll_consume(), None);
    }

    #[test]
    fn a_new_request_can_start_again_after_a_ready_result_is_consumed() {
        let mut p = GpuPickPipeline::new();
        p.request((0.0, 0.0));
        p.complete(None);
        p.poll_consume();

        assert!(p.request((1.0, 1.0)), "Idle after consuming Ready must accept a fresh request");
        assert_eq!(p.requests_started(), 2);
    }
}
