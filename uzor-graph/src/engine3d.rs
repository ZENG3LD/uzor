//! `GraphEngine3D` — the 3D sibling of [`crate::engine::GraphEngine`]
//! (W3D arc plan §1.2: a separate engine, NOT a `Dimension` mode bolted
//! onto the 2D facade). Reuses `Graph`/`SimTopology`/`NodeIndex`/
//! `EdgeIndex` unchanged; owns its own [`Camera3D`] and a 3D-aware
//! [`Layout`] impl ([`crate::layout::force_directed_3d::ForceDirectedLayout3D`]
//! by default).
//!
//! Wave 1 (plan §4) proved the physics core only
//! ([`GraphEngine3D::new`]/[`GraphEngine3D::tick`]). Wave 2 landed the
//! render path ([`GraphEngine3D::build_scene`], wired into
//! [`crate::render3d::build_scene`]) and camera event dispatch
//! ([`GraphEngine3D::on_event`] — drag-orbit, wheel-dolly, shift-drag
//! pan, per plan §1.4). Wave 3 lands CPU ray-vs-sphere hover/click
//! picking (`hovered`/`selected` actually changing,
//! [`crate::interaction::pick3d::screen_to_ray`]/
//! [`crate::interaction::pick3d::nearest_node_3d`]), a
//! [`GraphEngine3D::node_facts`] facts-panel accessor reusing
//! [`crate::engine::NodeFacts`] unchanged, and
//! [`GraphEngine3D::visible_labels`] (the label-overlay's project+LOD-
//! select half — see that method's own doc comment for the Wave 3
//! overlay-draw scope note).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::Vec3;
use uzor::input::{ModifierKeys, MouseButton, PlatformEvent};
use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_urx_3d::{Mesh, MeshLit, PerspectiveCamera, Scene3D};

use crate::camera3d::Camera3D;
use crate::engine::NodeFacts;
use crate::graph::{Graph, NodeIndex};
use crate::interaction::pick3d;
use crate::label_grid;
use crate::layout::force_directed_3d::ForceDirectedLayout3D;
use crate::layout::{Layout, LayoutTickResult};
use crate::particle::Particle;
use crate::render::{draw_hover_card, HoverCardInfo};

/// Wheel-to-dolly screen-delta sensitivity — mirrors
/// [`crate::engine::GraphEngine`]'s own `ZOOM_SENSITIVITY` (`engine.rs`)
/// in magnitude, but flipped in sign: 2D's `zoom` grows for "zoom in"
/// (multiply), 3D's orbit `distance` SHRINKS for "zoom in" — see
/// [`GraphEngine3D::on_scroll`].
const DOLLY_SENSITIVITY: f32 = 0.0015;
const DOLLY_FACTOR_MIN: f32 = 0.8;
const DOLLY_FACTOR_MAX: f32 = 1.25;

/// Screen-space distance the pointer must travel since the last hover
/// pick before [`GraphEngine3D::pick_at`] re-runs (Wave 3 perf guard —
/// mirrors 2D's own `HOVER_PICK_MIN_MOVE_PX`, `engine.rs`; re-declared
/// here since that constant is private to `engine.rs` and the two
/// engines otherwise share no picking code).
const HOVER_PICK_MIN_MOVE_PX: f64 = 2.0;

/// Screen-space distance a `PointerDown`->`PointerUp` pair may travel
/// while orbiting and still count as a click-to-select (Wave 3 — mirrors
/// 2D's own `CLICK_DRAG_THRESHOLD_PX`, `engine.rs`, same rationale and
/// value).
const CLICK_DRAG_THRESHOLD_PX: f64 = 4.0;

/// z-plane-degeneracy guard (live-caught Wave 2 defect, fixed in Wave 3 —
/// see [`GraphEngine3D::seed_positions`]/`uzor-graph/CLAUDE.md`'s
/// divergence log). If the seeded z half-extent is below this fraction
/// of the xy half-extent, the seed is treated as "effectively planar"
/// and gets jittered — every 3D force in `ForceDirectedLayout3D` is
/// z-symmetric (repulsion/link/center all scale the SAME `(dx, dy, dz)`
/// direction vector), so a perfectly planar seed (`dz == 0` for every
/// pair) has an identically-zero z-force forever and the sim can never
/// leave the plane on its own.
const Z_DEGENERACY_RATIO: f32 = 0.05;

/// Deterministic z-jitter half-range, as a fraction of the xy
/// half-extent (owner's own spec: "uniform in ±0.5 * xy_half_extent").
const Z_JITTER_XY_FRACTION: f32 = 0.5;

/// Alpha [`GraphEngine3D::ensure_z_variance`] reheats to after jittering
/// — high enough that the sim actively resolves the newly-introduced z
/// spread into a real 3D layout instead of just sitting on the jittered
/// starting positions (mirrors 2D's own `DRAG_REHEAT_ALPHA`-style "make
/// it visibly move" convention, `engine.rs`).
const Z_JITTER_REHEAT_ALPHA: f32 = 0.6;

/// Deterministic per-index pseudo-random value in `[-1.0, 1.0)` —
/// splitmix64-style LCG, index-seeded, NO `Math::random`/wall-clock time
/// (same convention `force_directed_3d.rs`'s own test fixtures already
/// use). Backs [`GraphEngine3D::ensure_z_variance`]'s z-jitter: distinct
/// indices deterministically land on different offsets, and the exact
/// same graph produces the exact same jitter on every run.
/// Bounding box of every particle's current `(x, y, z)` position — `None`
/// for an empty particle set. Shared by [`GraphEngine3D::fit_view`] and
/// [`GraphEngine3D::build_scene`]'s grid geometry (Wave 5): the same
/// "walk every particle, min/max component-wise" shape
/// [`GraphEngine3D::ensure_z_variance`] already computes inline for its
/// own z-degeneracy check, factored out here now that two more callers
/// need the identical bounding box.
fn particle_aabb(particles: &[Particle]) -> Option<(Vec3, Vec3)> {
    let mut iter = particles.iter();
    let first = iter.next()?;
    let mut min = Vec3::new(first.x, first.y, first.z);
    let mut max = min;
    for p in iter {
        let v = Vec3::new(p.x, p.y, p.z);
        min = min.min(v);
        max = max.max(v);
    }
    Some((min, max))
}

fn z_jitter_unit(index: usize) -> f32 {
    let mut state = (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let unit = ((state >> 40) as u32) as f32 / (1u32 << 24) as f32; // [0, 1)
    unit * 2.0 - 1.0 // [-1, 1)
}

/// In-progress pointer gesture (plan §1.4) — resolved once at
/// `PointerDown` from the held modifiers (mirrors the 2D engine's own
/// `PointerMode`, `engine.rs`), never re-resolved mid-drag even if a
/// modifier is pressed/released while dragging.
#[derive(Debug, Clone, Copy)]
enum Pointer3DMode {
    Idle,
    /// Plain left-drag — orbits the camera. `total` accumulates the
    /// screen-space path length travelled since `PointerDown` (Wave 3):
    /// a release with `total < CLICK_DRAG_THRESHOLD_PX` is a click, not
    /// a drag, and resolves a ray-pick select at the release point
    /// (mirrors 2D's own `PointerMode::PanningCamera { last, total }`
    /// click-vs-pan distinction exactly — node drag has no 3D
    /// equivalent this arc, plan §5, so EVERY plain drag is the
    /// "background" gesture 2D's `total` tracking already models).
    Orbiting { last: (f64, f64), total: f64 },
    /// Middle-drag or Shift+left-drag pan (plan §1.4). The initiating
    /// button is retained so releasing another held button cannot end
    /// the active pan. Deliberately does NOT resolve a click-select on
    /// release — pan is a dedicated camera gesture, not the plain
    /// left-drag/click gesture.
    Panning { last: (f64, f64), button: MouseButton },
    /// 3D node drag (owner-ordered live fix — previously EVERY plain
    /// drag orbited the camera, even one starting directly on a node;
    /// vasturiano `3d-force-graph`'s own convention). `node` is the hit
    /// resolved once at `PointerDown` (CPU `nearest_node_3d`, never
    /// re-resolved mid-drag — same "resolved once" idiom every other
    /// `Pointer3DMode` variant already follows); `plane_point`/
    /// `plane_normal` are the CAMERA-PARALLEL drag plane
    /// (`plane_point` = the node's world position AT DRAG-START,
    /// `plane_normal` = the camera's view direction at drag-start) that
    /// [`GraphEngine3D::on_pointer_moved`] intersects every move — see
    /// [`pick3d::ray_plane_intersection`]'s own doc comment for why a
    /// camera-parallel plane, not the ray's first world-surface hit.
    Dragging { node: NodeIndex, plane_point: Vec3, plane_normal: Vec3 },
}

/// Sustained `alphaTarget` a node drag holds the sim at while active
/// (owner's own spec: "~0.3", mirrors 2D's own `DRAG_ALPHA_TARGET`
/// convention in spirit though not value — 2D's engine.rs constant is
/// private and 3D's drag physics needs its own tuning pass regardless).
const NODE_DRAG_ALPHA_TARGET: f32 = 0.3;

/// Shared unit-sphere node mesh geometry (plan §1.3) — latitude/longitude
/// resolution tuned for a smooth silhouette at typical node screen sizes
/// without an excessive vertex count. One shared mesh serves every node
/// via `uzor-urx-3d`'s Arc-identity instancing, so this cost is paid
/// once per engine, not once per node.
///
/// Bumped from `12`/`16` (round-1 visual-quality wave — owner: large
/// spheres showed visible FACETING/low tessellation up close). `rings`
/// = latitude bands (pole-to-pole steps), `slices` = longitude bands
/// (segments around the equator) — `MeshLit::sphere`'s own vertex/index
/// generation already emits per-vertex (not per-face) normals (each
/// `(ring, slice)` grid point gets its own shared vertex, normal =
/// unit-sphere position direction — verified by direct read, no change
/// needed there), so this is a pure tessellation-density bump: cost is
/// `(rings+1)*(slices+1)` vertices for ONE shared instanced mesh (paid
/// once per engine, not once per node) — negligible at these numbers.
const NODE_SPHERE_RINGS: u32 = 22;
const NODE_SPHERE_SLICES: u32 = 30;

/// Label text offset from its node's projected screen position (Wave 4
/// — [`GraphEngine3D::draw_overlay`]). The 2D engine's own
/// `render::draw_nodes` offsets by `node_screen_radius + 4.0` (a
/// per-node value); 3D's `visible_labels` doesn't expose each node's
/// own projected screen radius (only `label_grid::select_labels`'s
/// internal tie-break sees it, per that method's own doc comment) —
/// recomputing the pinhole-projection formula a second time here, only
/// for a text offset, is more machinery than this wave's "labels + card
/// are the deliverable" scope needs. A fixed offset is the documented
/// Wave 4 simplification; see `uzor-graph/CLAUDE.md`'s divergence log.
const OVERLAY_LABEL_OFFSET_X: f64 = 6.0;
const OVERLAY_LABEL_OFFSET_Y: f64 = 4.0;

/// [`Camera3D::fit_bounds`]'s own multiplicative margin, applied by
/// [`GraphEngine3D::fit_view`] — the owner's own spec ("~1.1").
const FIT_VIEW_PADDING: f32 = 1.1;

/// Below this node count, a bounding-box fit doesn't mean much (there's
/// no real graph SHAPE yet) — [`GraphEngine3D::fit_view`] falls back to
/// [`Camera3D::default`]'s own distance instead of zooming to an
/// arbitrary (possibly degenerate) extent, per this wave's own spec.
const FIT_VIEW_MIN_NODES: usize = 3;

/// A particle cloud whose half-diagonal sits below this world-unit
/// threshold is treated as "effectively a single point" for fit purposes
/// (e.g. every particle still sitting at [`GraphEngine3D::new`]'s shared
/// origin, before any seeding/settling has spread them out) — guards the
/// same degenerate case [`FIT_VIEW_MIN_NODES`] targets by COUNT, but by
/// actual EXTENT instead, since 3+ coincident nodes are just as
/// meaningless a `fit_bounds` target as 0-2 nodes are.
const FIT_VIEW_MIN_HALF_DIAGONAL: f32 = 1e-3;

/// The 3D sibling of [`crate::engine::GraphEngine`] — see the module doc.
pub struct GraphEngine3D<N, E, L: Layout = ForceDirectedLayout3D> {
    pub graph: Graph<N, E>,
    pub particles: Vec<Particle>,
    pub layout: L,
    pub camera: Camera3D,
    pub hovered: Option<NodeIndex>,
    pub selected: Option<NodeIndex>,
    /// Shared unit sphere every node instances from (plan §1.3) — built
    /// once at construction, never mutated.
    node_mesh: Arc<MeshLit>,
    /// Shared unit edge-quad every edge instances from (Wave C/D — see
    /// `crate::render3d`'s own module doc for why edges are a
    /// screen-space billboarded quad, not a cylinder or a hardware
    /// `LineList`, as of the edge-quality overhaul).
    edge_mesh: Arc<Mesh>,
    /// Held keyboard-modifier state (Wave 2) — mirrors the 2D engine's
    /// own `PlatformEvent::ModifiersChanged` tracking pattern
    /// (`engine.rs`), read by [`GraphEngine3D::on_pointer_down`] to pick
    /// orbit vs. shift-drag pan (plan §1.4).
    modifiers: ModifierKeys,
    mode: Pointer3DMode,
    /// Last pointer position in screen px — [`GraphEngine3D::on_scroll`]'s
    /// "is the cursor currently over this viewport" gate (mirrors
    /// `GraphEngine::on_scroll`, which reads its own
    /// `last_pointer_screen` the same way).
    last_pointer_screen: (f64, f64),
    /// Every graph node id, cached once at construction (Wave 3) — the
    /// CPU ray-picking candidate slice `pick3d::nearest_node_3d` needs.
    /// 3D has no visibility-culling infrastructure yet (plan §5
    /// exclusion), so every node is always a pick candidate; caching
    /// avoids rebuilding this `Vec` on every guarded hover/click pick.
    all_node_ids: Vec<NodeIndex>,
    /// Screen position at the last hover pick (Wave 3) — `None` forces
    /// the very next `PointerMoved` inside the viewport to re-pick
    /// unconditionally (mirrors `GraphEngine`'s own
    /// `last_hover_pick_screen`).
    last_hover_pick_screen: Option<(f64, f64)>,
    /// Sigma `LabelGrid` per-cell quota density (Wave 3) — see
    /// [`GraphEngine3D::visible_labels`]. Defaults to
    /// `label_grid::DEFAULT_LABEL_DENSITY`, same as the 2D engine.
    label_density: f64,
    /// Shared unit sphere for the GPU color-ID id-pass (Wave 4) — plain
    /// `Unlit` geometry, distinct from `node_mesh`'s `MeshLit` sphere
    /// (same tessellation, different vertex format — see
    /// [`crate::render3d::build_id_pass_mesh`]'s own doc comment).
    id_pass_mesh: Arc<Mesh>,
    /// Node-count threshold above which hover picking escalates to the
    /// GPU color-ID pass (Wave 4, plan §1.5) — defaults to
    /// [`pick3d::GPU_PICK_NODE_THRESHOLD`], overridable via
    /// [`GraphEngine3D::set_gpu_pick_threshold`] so a test can exercise
    /// the switch without a literal >10k-node fixture.
    gpu_pick_threshold: usize,
    /// Deferred GPU pick request/result state machine (Wave 4) — see
    /// [`pick3d::GpuPickPipeline`]'s own doc comment. Driving the real
    /// `wgpu` I/O ([`pick3d::request_gpu_pick`]/[`pick3d::poll_gpu_pick`])
    /// is the CALLER's job (this engine owns no `wgpu::Device`); see
    /// [`GraphEngine3D::apply_gpu_pick_result`].
    gpu_pick_pipeline: pick3d::GpuPickPipeline,
    /// Ground-reference grid toggle (Wave 5) — OFF by default: a force
    /// graph has no semantic axes of its own, so the grid is purely an
    /// opt-in orientation aid, not a default-on feature. See
    /// [`GraphEngine3D::set_grid_enabled`]/[`GraphEngine3D::grid_enabled`].
    grid_enabled: bool,
}

impl<N, E, L: Layout> GraphEngine3D<N, E, L> {
    pub fn new(graph: Graph<N, E>, layout: L) -> Self {
        let n = graph.node_count();
        let all_node_ids: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
        Self {
            graph,
            particles: vec![Particle::default(); n],
            layout,
            camera: Camera3D::default(),
            hovered: None,
            selected: None,
            node_mesh: Arc::new(MeshLit::sphere(1.0, NODE_SPHERE_RINGS, NODE_SPHERE_SLICES, [1.0, 1.0, 1.0, 1.0])),
            edge_mesh: Arc::new(Mesh::unit_edge_quad([1.0, 1.0, 1.0, 1.0])),
            modifiers: ModifierKeys::default(),
            mode: Pointer3DMode::Idle,
            last_pointer_screen: (0.0, 0.0),
            all_node_ids,
            last_hover_pick_screen: None,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
            id_pass_mesh: Arc::new(crate::render3d::build_id_pass_mesh(NODE_SPHERE_RINGS, NODE_SPHERE_SLICES)),
            gpu_pick_threshold: pick3d::GPU_PICK_NODE_THRESHOLD,
            gpu_pick_pipeline: pick3d::GpuPickPipeline::new(),
            grid_enabled: false,
        }
    }

    /// Shared node-sphere mesh — exposed read-only so a future render
    /// pass (Wave 2) can reuse it without re-generating geometry.
    pub fn node_mesh(&self) -> &Arc<MeshLit> {
        &self.node_mesh
    }

    /// Shared edge-line mesh — see [`GraphEngine3D::node_mesh`].
    pub fn edge_mesh(&self) -> &Arc<Mesh> {
        &self.edge_mesh
    }

    /// Advance the 3D force simulation by `dt` real seconds — the whole
    /// of this wave's proof surface (plan §4 Wave 1).
    pub fn tick(&mut self, dt: f32) -> LayoutTickResult {
        let topo = self.graph.topology();
        self.layout.tick(&topo, &mut self.particles, dt)
    }

    /// Seed every particle's `(x, y)` from `positions` (mirrors
    /// [`crate::engine::GraphEngine::seed_positions`]'s own 2D-position
    /// signature — the common bootstrap shape, migrating an existing
    /// flat/2D layout into 3D), then [`GraphEngine3D::ensure_z_variance`]s
    /// the result. This is the ENGINE-level fix for a live-caught Wave 2
    /// defect (`uzor-graph/CLAUDE.md`'s divergence log has the full
    /// writeup): seeding every node at `z = 0` makes every 3D force
    /// z-symmetric, so the sim never leaves the plane on its own — this
    /// is the one seeding entry point that guards against it, so any
    /// caller (not just `force_graph_demo`) gets the fix for free.
    pub fn seed_positions(&mut self, positions: &[(f32, f32)]) {
        for (p, &(x, y)) in self.particles.iter_mut().zip(positions.iter()) {
            p.x = x;
            p.y = y;
        }
        self.ensure_z_variance();
    }

    /// If the particles' current z half-extent is degenerate relative to
    /// their xy half-extent (below [`Z_DEGENERACY_RATIO`] — covers both
    /// "every z is exactly 0" and "z has only trivial noise"),
    /// deterministically jitters every particle's `z` by an
    /// index-seeded, uniformly-distributed offset in
    /// `±(xy_half_extent * Z_JITTER_XY_FRACTION)` (splitmix64-style LCG —
    /// no `Math::random`/time, same convention this crate's own
    /// deterministic test fixtures already use, e.g.
    /// `force_directed_3d.rs`'s `deterministic_particles_3d`), then
    /// reheats so the sim actively resolves into a true volume instead of
    /// just sitting on the jittered starting positions.
    ///
    /// A degenerate XY extent (every particle at the same `x`/`y` too —
    /// e.g. right after [`GraphEngine3D::new`], before any seeding) is
    /// left untouched: there's no meaningful scale to derive a jitter
    /// range from yet, and [`GraphEngine3D::seed_positions`] is the
    /// caller that gives this a real xy extent to work with.
    pub fn ensure_z_variance(&mut self) {
        if self.particles.is_empty() {
            return;
        }
        let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
        let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
        let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
        for p in &self.particles {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }
        let xy_half_extent = ((max_x - min_x).max(max_y - min_y)) * 0.5;
        if xy_half_extent <= 1e-6 {
            return;
        }
        let z_half_extent = (max_z - min_z) * 0.5;
        if z_half_extent > xy_half_extent * Z_DEGENERACY_RATIO {
            // Already has a real z spread — an intentional 3D seed from
            // some other source, don't disturb it.
            return;
        }
        let jitter_range = xy_half_extent * Z_JITTER_XY_FRACTION;
        for (i, p) in self.particles.iter_mut().enumerate() {
            p.z += z_jitter_unit(i) * jitter_range;
        }
        self.layout.reheat(Z_JITTER_REHEAT_ALPHA);
    }

    /// Raw `PlatformEvent` handler — orbit-drag, wheel-dolly,
    /// middle-drag/shift-drag pan (plan §1.4), hover on `PointerMoved` and click-to-select on a
    /// low-movement `PointerDown`->`PointerUp` pair (Wave 3, plan §1.5).
    /// Wire this from the app's 3D dispatch (see
    /// `uzor-desktop::scene3d_app`'s divergence log for how
    /// `force_graph_demo` routes events to whichever dimension is
    /// active). Returns `true` if the event was consumed.
    pub fn on_event(&mut self, event: &PlatformEvent, viewport: Rect) -> bool {
        match event {
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => self.on_pointer_down(*x, *y, viewport),
            PlatformEvent::PointerDown { x, y, button: MouseButton::Middle } => {
                self.on_pan_pointer_down(*x, *y, MouseButton::Middle, viewport)
            }
            PlatformEvent::PointerMoved { x, y } => self.on_pointer_moved(*x, *y, viewport),
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => {
                self.on_pointer_up(*x, *y, MouseButton::Left, viewport)
            }
            PlatformEvent::PointerUp { x, y, button: MouseButton::Middle } => {
                self.on_pointer_up(*x, *y, MouseButton::Middle, viewport)
            }
            PlatformEvent::Scroll { dy, .. } => self.on_scroll(*dy, viewport),
            PlatformEvent::ModifiersChanged { modifiers } => {
                self.modifiers = *modifiers;
                true
            }
            PlatformEvent::KeyDown { modifiers, .. } | PlatformEvent::KeyUp { modifiers, .. } => {
                self.modifiers = *modifiers;
                false
            }
            _ => false,
        }
    }

    /// Shift-held left-drag pans; a plain left-drag starting ON a node
    /// drags THAT node (owner-ordered live fix — see [`Pointer3DMode::Dragging`]'s
    /// own doc comment), everything else orbits the camera — the drag
    /// KIND is resolved once here, at drag-start, and held for the whole
    /// gesture (mirrors the 2D engine's own box-select-mode resolution,
    /// `box_select_mode_for` in `engine.rs`). `false` (event not
    /// consumed) if `(x, y)` lands outside `viewport`.
    fn on_pointer_down(&mut self, x: f64, y: f64, viewport: Rect) -> bool {
        if !viewport.contains(x, y) {
            return false;
        }
        if self.modifiers.shift {
            return self.on_pan_pointer_down(x, y, MouseButton::Left, viewport);
        }
        // Node-drag ray-pick — ALWAYS the CPU path (`nearest_node_3d`),
        // never the GPU color-ID pass, regardless of
        // `should_use_gpu_pick()`: drag-start needs a synchronous,
        // same-frame answer, and the plan's GPU escalation is scoped to
        // hover refinement only (see `GraphEngine3D::apply_gpu_pick_result`'s
        // own doc comment).
        let aspect = (viewport.width / viewport.height.max(1.0)) as f32;
        let camera = self.camera.to_perspective(aspect);
        let (ray_origin, ray_dir) = pick3d::screen_to_ray(&camera, viewport, (x, y));
        if let Some(hit) = pick3d::nearest_node_3d(&self.graph, &self.particles, ray_origin, ray_dir, &self.all_node_ids) {
            if let Some(p) = self.particles.get(hit.index()) {
                let plane_point = Vec3::new(p.x, p.y, p.z);
                let plane_normal = (camera.target - camera.eye).normalize_or_zero();
                self.mode = Pointer3DMode::Dragging { node: hit, plane_point, plane_normal };
                if let Some(pm) = self.particles.get_mut(hit.index()) {
                    pm.pin3(plane_point.x, plane_point.y, plane_point.z);
                }
                // Sustained reheat (mirrors 2D's own drag contract,
                // `engine.rs`): hold alpha at the drag target for the
                // whole gesture instead of a one-shot bump that starts
                // cooling right away.
                self.layout.set_alpha_target(NODE_DRAG_ALPHA_TARGET);
                self.layout.reheat(NODE_DRAG_ALPHA_TARGET);
                return true;
            }
        }
        self.mode = Pointer3DMode::Orbiting { last: (x, y), total: 0.0 };
        true
    }

    fn on_pan_pointer_down(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
        viewport: Rect,
    ) -> bool {
        if !viewport.contains(x, y) {
            return false;
        }
        self.mode = Pointer3DMode::Panning { last: (x, y), button };
        true
    }

    fn on_pointer_moved(&mut self, x: f64, y: f64, viewport: Rect) -> bool {
        self.last_pointer_screen = (x, y);
        let mut handled = false;

        match self.mode {
            Pointer3DMode::Orbiting { last, total } => {
                let dx = x - last.0;
                let dy = y - last.1;
                self.camera.orbit(dx as f32, dy as f32);
                self.mode = Pointer3DMode::Orbiting { last: (x, y), total: total + (dx * dx + dy * dy).sqrt() };
                handled = true;
            }
            Pointer3DMode::Panning { last, button } => {
                self.camera.pan((x - last.0) as f32, (y - last.1) as f32);
                self.mode = Pointer3DMode::Panning { last: (x, y), button };
                handled = true;
            }
            Pointer3DMode::Dragging { node, plane_point, plane_normal } => {
                // Camera-parallel drag plane, fixed at drag-start (the
                // camera itself never orbits/pans/dollies during a node
                // drag — `on_pointer_down` chose `Dragging` INSTEAD of
                // `Orbiting`/`Panning`, so `self.camera` is frozen for
                // the whole gesture) — recompute the ray fresh from the
                // CURRENT cursor position every move and intersect it
                // against that same plane (see
                // `pick3d::ray_plane_intersection`'s own doc comment).
                let aspect = (viewport.width / viewport.height.max(1.0)) as f32;
                let camera = self.camera.to_perspective(aspect);
                let (ray_origin, ray_dir) = pick3d::screen_to_ray(&camera, viewport, (x, y));
                if let Some(world) = pick3d::ray_plane_intersection(ray_origin, ray_dir, plane_point, plane_normal) {
                    if let Some(p) = self.particles.get_mut(node.index()) {
                        p.pin3(world.x, world.y, world.z);
                    }
                }
                handled = true;
            }
            Pointer3DMode::Idle => {}
        }

        // Wave 3 hover pick — same movement-guard idiom as 2D's own
        // `on_pointer_moved` (`engine.rs`): re-run the O(candidates)
        // ray-sphere scan only once the pointer has actually moved
        // `HOVER_PICK_MIN_MOVE_PX` since the last pick, and regardless of
        // whether this move is ALSO orbiting/panning the camera (2D does
        // the same — hover keeps tracking the cursor's current screen
        // position against whatever the camera looks like right now,
        // even mid-drag).
        if viewport.contains(x, y) {
            let moved_enough = match self.last_hover_pick_screen {
                Some((lx, ly)) => {
                    let ddx = x - lx;
                    let ddy = y - ly;
                    (ddx * ddx + ddy * ddy).sqrt() >= HOVER_PICK_MIN_MOVE_PX
                }
                None => true,
            };
            if moved_enough {
                self.last_hover_pick_screen = Some((x, y));
                // CPU ray-pick is ALWAYS the same-frame answer (plan §4
                // Wave 4: "the CPU ray pick as the same-frame answer
                // while a GPU result is in flight") — above the GPU-pick
                // threshold this ALSO kicks off a deferred GPU color-ID
                // request (see `GraphEngine3D::apply_gpu_pick_result`),
                // which refines `hovered` a frame or two later once it
                // resolves. Below the threshold the GPU pipeline is
                // never touched at all.
                self.hovered = self.pick_at(x, y, viewport);
                if self.should_use_gpu_pick() {
                    self.gpu_pick_pipeline.request((x, y));
                }
            }
            handled = true;
        } else if self.hovered.is_some() {
            self.hovered = None;
            self.last_hover_pick_screen = None;
        }

        handled
    }

    /// Ends the in-progress drag/orbit/pan/node-drag gesture; a plain
    /// orbit-drag that travelled less than [`CLICK_DRAG_THRESHOLD_PX`]
    /// since `PointerDown` resolves as a click — a Replace-select
    /// ray-pick at the release point (`None` on empty space deselects,
    /// mirroring 2D's own `clear_selection()` on a background click).
    /// Shift-drag pan never resolves a click (see [`Pointer3DMode::Panning`]'s
    /// doc comment). A node drag ALWAYS selects the dragged node on
    /// release (a plain click on a node and a click-and-drag both end in
    /// the node being selected — there's no ambiguous "was this a click
    /// or a drag" question for a node hit the way there is for
    /// background) and applies the STICKY drag-end policy (owner order,
    /// same default [`crate::engine::DragEndPolicy::Sticky`] the 2D
    /// engine uses): the node stays pinned exactly where the last
    /// `PointerMoved` left it — `fx`/`fy`/`fz` already hold that
    /// position via `pin3`, nothing further to do here beyond releasing
    /// `alpha_target`.
    fn on_pointer_up(&mut self, x: f64, y: f64, button: MouseButton, viewport: Rect) -> bool {
        let mode = self.mode;
        if let Pointer3DMode::Panning { button: active_button, .. } = mode {
            if button != active_button {
                return false;
            }
        }
        self.mode = Pointer3DMode::Idle;
        match mode {
            Pointer3DMode::Orbiting { total, .. } => {
                if total < CLICK_DRAG_THRESHOLD_PX && viewport.contains(x, y) {
                    self.selected = self.pick_at(x, y, viewport);
                }
                true
            }
            Pointer3DMode::Panning { .. } => true,
            Pointer3DMode::Dragging { node, .. } => {
                self.layout.set_alpha_target(0.0);
                self.selected = Some(node);
                true
            }
            Pointer3DMode::Idle => false,
        }
    }

    /// Shared CPU ray-pick primitive (plan §1.5 v1) — resolves the
    /// nearest node the ray through `(x, y)` in `viewport` hits, using
    /// the CURRENT camera state. Both hover ([`GraphEngine3D::on_pointer_moved`])
    /// and click-select ([`GraphEngine3D::on_pointer_up`]) funnel
    /// through this one function so they can never diverge on which
    /// camera/candidate set they pick against.
    fn pick_at(&self, x: f64, y: f64, viewport: Rect) -> Option<NodeIndex> {
        let aspect = (viewport.width / viewport.height.max(1.0)) as f32;
        let camera = self.camera.to_perspective(aspect);
        let (origin, dir) = pick3d::screen_to_ray(&camera, viewport, (x, y));
        pick3d::nearest_node_3d(&self.graph, &self.particles, origin, dir, &self.all_node_ids)
    }

    /// Wheel-to-dolly, gated on the cursor currently sitting over
    /// `viewport` (mirrors `GraphEngine::on_scroll`'s own gate). Positive
    /// `dy` means "zoom in" (matches the 2D engine's own convention) —
    /// in orbit-camera terms that means a SMALLER `distance`, the
    /// inverse of 2D's "bigger `zoom`", so the sign is flipped relative
    /// to the 2D formula this mirrors.
    fn on_scroll(&mut self, dy: f64, viewport: Rect) -> bool {
        if !viewport.contains(self.last_pointer_screen.0, self.last_pointer_screen.1) {
            return false;
        }
        let factor = (1.0 - dy as f32 * DOLLY_SENSITIVITY).clamp(DOLLY_FACTOR_MIN, DOLLY_FACTOR_MAX);
        self.camera.dolly(factor);
        true
    }

    /// Frame the WHOLE graph in view — dolly + retarget only, current
    /// yaw/pitch are kept (Wave 5, mirrors the 2D engine's own
    /// `GraphEngine::fit_view`'s "keep the user's orientation" spirit).
    /// `aspect` is the caller's real render-surface aspect — this engine
    /// has no viewport of its own to derive one from (same reason
    /// [`GraphEngine3D::camera`] takes an explicit `aspect` too). A no-op
    /// if there isn't a single particle yet; a too-small or
    /// near-degenerate (effectively coincident) particle cloud (see
    /// [`FIT_VIEW_MIN_NODES`]/[`FIT_VIEW_MIN_HALF_DIAGONAL`]) recenters on
    /// whatever IS there but falls back to [`Camera3D::default`]'s own
    /// distance rather than an arbitrary [`Camera3D::fit_bounds`] result.
    pub fn fit_view(&mut self, aspect: f32) {
        let Some((min, max)) = particle_aabb(&self.particles) else { return };
        let half_diagonal = (max - min).length() * 0.5;
        if self.particles.len() < FIT_VIEW_MIN_NODES || half_diagonal < FIT_VIEW_MIN_HALF_DIAGONAL {
            self.camera.target = (min + max) * 0.5;
            self.camera.distance = Camera3D::default().distance;
            return;
        }
        self.camera.fit_bounds(min, max, aspect, FIT_VIEW_PADDING);
    }

    /// Whether [`GraphEngine3D::build_scene`]/[`GraphEngine3D::draw_overlay`]
    /// currently emit the ground-reference grid + axis tick labels (Wave
    /// 5) — default `false`. See [`GraphEngine3D::set_grid_enabled`].
    pub fn grid_enabled(&self) -> bool {
        self.grid_enabled
    }

    /// Toggle the ground-reference grid (Wave 5) — see
    /// [`GraphEngine3D::grid_enabled`]'s own doc comment for why it
    /// defaults off.
    pub fn set_grid_enabled(&mut self, enabled: bool) {
        self.grid_enabled = enabled;
    }

    /// Instanced sphere nodes + billboarded edge quads (plan §1.3) — wires
    /// straight into [`crate::render3d::build_scene`], which is
    /// independently unit-tested (no GPU needed) for the node/edge
    /// instance construction itself; see `uzor-graph/tests/render3d_gpu.rs`
    /// for the headless-GPU proof that the result actually renders
    /// visually-distinct pixels.
    ///
    /// **Wave 5**: while [`GraphEngine3D::grid_enabled`], also appends the
    /// ground-reference grid's own instanced lines
    /// (`crate::render3d::build_grid_plan`/`build_grid_instances`),
    /// sharing the exact same edge-quad mesh edges instance from. `viewport_height_px`
    /// is the render surface's pixel height — needed for the grid's
    /// distance-LOD step (`crate::render3d::grid_step_for_scale`); a
    /// caller not using the grid can pass any positive value. A no-op
    /// (no grid appended) while there isn't a single particle yet.
    pub fn build_scene(&self, viewport_height_px: f64) -> Scene3D {
        let mut scene = crate::render3d::build_scene(&self.graph, &self.particles, &self.node_mesh, &self.edge_mesh);
        if self.grid_enabled {
            if let Some((min, max)) = particle_aabb(&self.particles) {
                let fov_y = self.camera.to_perspective(1.0).fov_y;
                let step = crate::render3d::grid_step_for_scale(self.camera.distance, fov_y, viewport_height_px);
                let plan = crate::render3d::build_grid_plan(min, max, step);
                scene.nodes.extend(crate::render3d::build_grid_instances(&plan, &self.edge_mesh));
            }
        }
        scene
    }

    /// Shared id-pass sphere mesh (Wave 4) — see
    /// [`GraphEngine3D::build_id_pass_scene`].
    pub fn id_pass_mesh(&self) -> &Arc<Mesh> {
        &self.id_pass_mesh
    }

    /// GPU color-ID id-pass scene (Wave 4, plan §1.5/§4) — wires into
    /// [`crate::render3d::build_id_pass_scene`]. A caller with a live
    /// `wgpu::Device`/`Renderer3D` renders this via
    /// [`pick3d::request_gpu_pick`] instead of the ordinary
    /// [`GraphEngine3D::build_scene`] Lit scene.
    pub fn build_id_pass_scene(&self) -> Scene3D {
        crate::render3d::build_id_pass_scene(&self.graph, &self.particles, &self.id_pass_mesh)
    }

    /// Current GPU-pick escalation threshold (Wave 4) — see
    /// [`GraphEngine3D::should_use_gpu_pick`].
    pub fn gpu_pick_threshold(&self) -> usize {
        self.gpu_pick_threshold
    }

    /// Override the GPU-pick escalation threshold — see the field's own
    /// doc comment. `0` forces GPU picking for any non-empty graph; a
    /// very large value pins the engine to CPU-only picking regardless
    /// of graph size.
    pub fn set_gpu_pick_threshold(&mut self, threshold: usize) {
        self.gpu_pick_threshold = threshold;
    }

    /// `true` once `graph.node_count()` exceeds
    /// [`GraphEngine3D::gpu_pick_threshold`] (plan §1.5's own escalation
    /// rule) — the "which path" probe a test/caller can read without
    /// touching any `wgpu` state.
    pub fn should_use_gpu_pick(&self) -> bool {
        self.graph.node_count() > self.gpu_pick_threshold
    }

    /// Total number of GPU pick requests [`GraphEngine3D::on_event`] has
    /// actually started (Wave 4's own call-counter/probe gate — see
    /// [`pick3d::GpuPickPipeline::requests_started`]).
    pub fn gpu_pick_requests_started(&self) -> usize {
        self.gpu_pick_pipeline.requests_started()
    }

    /// `true` while a GPU pick request is outstanding (see
    /// [`pick3d::GpuPickPipeline::is_in_flight`]).
    pub fn gpu_pick_pending(&self) -> bool {
        self.gpu_pick_pipeline.is_in_flight()
    }

    /// Feed back a resolved GPU pick result (Wave 4) — the caller drives
    /// the real `wgpu` readback externally via
    /// [`pick3d::request_gpu_pick`]/[`pick3d::poll_gpu_pick`] (this
    /// engine owns no `wgpu::Device`) and calls this once
    /// [`pick3d::poll_gpu_pick`] returns `Some(_)`. Refines `hovered`
    /// ONLY — click-select stays CPU-only/synchronous always, so a click
    /// never changes retroactively after the fact once the user has
    /// already acted on it (a deliberate Wave 4 scope decision, see
    /// `uzor-graph/CLAUDE.md`'s own divergence log). A no-op if no GPU
    /// pick is currently in flight (stale/duplicate feed).
    pub fn apply_gpu_pick_result(&mut self, result: Option<NodeIndex>) {
        self.gpu_pick_pipeline.complete(result);
        if let Some(r) = self.gpu_pick_pipeline.poll_consume() {
            self.hovered = r;
        }
    }

    /// Fresh `PerspectiveCamera` for the current orbit-camera state.
    pub fn camera(&self, aspect: f32) -> PerspectiveCamera {
        self.camera.to_perspective(aspect)
    }

    pub fn hovered(&self) -> Option<NodeIndex> {
        self.hovered
    }

    pub fn selected(&self) -> Option<NodeIndex> {
        self.selected
    }

    /// Generic per-node facts for a caller's sidebar/inspector — REUSES
    /// [`crate::engine::NodeFacts`] unchanged (Wave 3, plan §4: "the same
    /// type the 2D engine already exposes, zero-cost reuse, no new
    /// struct"). **Known divergence**: `NodeFacts::position` is a 2D
    /// `(f32, f32)` shape the plan deliberately keeps as-is — a 3D
    /// node's `z` is NOT reported here (`uzor-graph/CLAUDE.md`'s Wave 3
    /// divergence log has the full reasoning). `pinned` reads
    /// [`Particle::is_pinned_3d`] — always `false` this wave, since node
    /// drag/pin has no 3D implementation yet (plan §5 exclusion), but
    /// wired correctly for whenever that lands.
    pub fn node_facts(&self, node: NodeIndex) -> Option<NodeFacts<'_>> {
        let n = self.graph.get_node(node)?;
        let p = self.particles.get(node.index())?;
        Some(NodeFacts { index: node, label: &n.label, category: &n.category, degree: self.graph.degree(node), position: (p.x, p.y), pinned: p.is_pinned_3d() })
    }

    /// LOD-selected label positions for `camera`/`viewport` this frame
    /// (plan §1.3/§4 Wave 3, the "project + LOD-select" half of the
    /// label-overlay design): projects every node's world position
    /// through `camera` (silently dropping any behind the eye — see
    /// [`crate::interaction::pick3d::project_world_to_screen`]), hands
    /// the resulting screen positions to the EXISTING
    /// `label_grid::select_labels` quota/ranking pass completely
    /// unchanged, and returns the `(node, screen_x, screen_y)` triples a
    /// caller would draw text at.
    ///
    /// `screen_radius` (an input `label_grid::select_labels` uses only
    /// for its crowded-cell tie-break, not for culling) is approximated
    /// from the standard pinhole-camera size formula
    /// (`node.radius / (distance * tan(fov_y / 2)) * viewport.height /
    /// 2`) — exact enough for ranking, not claimed pixel-perfect. The
    /// per-cell quota's "zoom" input (2D's `Camera2D::zoom`, which this
    /// engine has no direct equivalent of) is approximated as
    /// `Camera3D::default().distance / self.camera.distance` — `1.0` at
    /// the default orbit distance (matching 2D's own zoom-identity
    /// convention, `quota == ceil(density)`), growing as the user dollies
    /// in, same qualitative "more room, more labels" behavior 2D's zoom
    /// drives.
    ///
    /// **Wave 3 overlay-draw scope note** (`uzor-graph/CLAUDE.md`/
    /// `uzor-desktop/CLAUDE.md` divergence logs have the full grounding):
    /// this method is the real, tested, engine-level API a label-overlay
    /// DRAW call needs, but that draw call is NOT wired into the live
    /// 3D-composed frame this wave. `uzor-desktop`'s `Manager` skips
    /// `app.ui()`/the whole 2D chrome pass entirely on a 3D-active frame
    /// (Wave 2's own forced divergence), and `submit_urx_composed`'s own
    /// 2D pass reads a render channel (`state.urx_ctx`/`active_urx`)
    /// this `Manager` never populates — painting a real label overlay
    /// needs new `uzor-render-hub`/`Manager` integration surface (wiring
    /// `set_active_urx`, or teaching `submit_urx_composed` to accept a
    /// pre-built 2D scene), out of this wave's scope. Reachable this
    /// wave via the demo's agent surface for screenshot/JSON
    /// verification of the underlying pick/project math, not as painted
    /// pixels.
    ///
    /// No forced-label union (collapsed-cluster representatives, hover/
    /// selection-neighbor forcing) — 3D has none of that machinery yet
    /// (plan §5: cluster collapse and `FocusSet` neighbor-dimming are
    /// explicitly out of this arc).
    pub fn visible_labels(&self, camera: &PerspectiveCamera, viewport: Rect) -> Vec<(NodeIndex, f64, f64)> {
        let mut candidates = Vec::with_capacity(self.all_node_ids.len());
        let mut screen_positions: HashMap<NodeIndex, (f64, f64)> = HashMap::with_capacity(self.all_node_ids.len());
        for &id in &self.all_node_ids {
            let (Some(p), Some(node)) = (self.particles.get(id.index()), self.graph.get_node(id)) else { continue };
            let world = Vec3::new(p.x, p.y, p.z);
            let Some(screen_pos) = pick3d::project_world_to_screen(camera, world, viewport) else { continue };
            let dist = (world - camera.eye).length().max(1e-3);
            let half_fov_tan = (camera.fov_y * 0.5).tan().max(1e-6);
            let screen_radius = ((node.radius / (dist * half_fov_tan)) * (viewport.height as f32 * 0.5)) as f64;
            candidates.push(label_grid::LabelCandidate { node: id, screen_pos, degree: self.graph.degree(id), screen_radius });
            screen_positions.insert(id, screen_pos);
        }
        let zoom_analog = (Camera3D::default().distance / self.camera.distance.max(1e-3)) as f64;
        let shown = label_grid::select_labels(&candidates, viewport, zoom_analog, self.label_density, &HashSet::new());
        shown.into_iter().filter_map(|id| screen_positions.get(&id).map(|&(x, y)| (id, x, y))).collect()
    }

    /// Current label-LOD quota density (Wave 3) — see
    /// [`GraphEngine3D::visible_labels`]. Defaults to
    /// `label_grid::DEFAULT_LABEL_DENSITY`, same as the 2D engine.
    pub fn label_density(&self) -> f64 {
        self.label_density
    }

    /// Set the label-LOD quota density; negative input clamps to `0.0`
    /// (empty per-cell quota) — mirrors `GraphEngine::set_label_density`'s
    /// own clamp convention.
    pub fn set_label_density(&mut self, density: f64) {
        self.label_density = density.max(0.0);
    }

    /// Paint the label overlay + hover info card for `camera`/`viewport`
    /// this frame (Wave 4 / W3D arc plan §1.3 label-overlay gap, closed
    /// here — `uzor-graph/CLAUDE.md`'s Wave 3 divergence log has the full
    /// grounding for why the actual DRAW call was deferred to this wave).
    /// `render` is an ordinary 2D `RenderContext` in the SAME logical
    /// pixel space as `viewport` (screen-space overlay, no z-test — the
    /// plan's own §1.3 "industry standard for 3D graph labels" call, same
    /// approach vasturiano's `3d-force-graph` uses) — a caller wires this
    /// via `uzor-desktop::Scene3DFrame::overlay`, see that field's own
    /// doc comment for the exact composition point.
    ///
    /// Labels: every `(node, screen_x, screen_y)` [`GraphEngine3D::visible_labels`]
    /// returns (which already culls anything behind the camera via
    /// [`crate::interaction::pick3d::project_world_to_screen`] returning
    /// `None`, and applies the LOD grid quota) is drawn with the SAME
    /// font/fill-color/alpha-fade convention `crate::render::draw_nodes`
    /// uses for the 2D engine's own labels (`label_grid::label_alpha`,
    /// degree-boosted fade) — "same quality as 2D mode's labels" per the
    /// plan's own goal — just at a fixed text offset (see
    /// [`OVERLAY_LABEL_OFFSET_X`]/[`OVERLAY_LABEL_OFFSET_Y`]'s own doc
    /// comment for why, unlike 2D, this isn't `node_screen_radius`-based).
    ///
    /// Hover card: reuses [`crate::render::draw_hover_card`] (the SAME
    /// function the 2D engine's own hover card calls) anchored at the
    /// hovered node's projected screen position, built from
    /// [`GraphEngine3D::node_facts`] — one hover-card implementation for
    /// both dimensions, not a second one invented here.
    ///
    /// **Selection ring deliberately NOT drawn** — no 3D-side selected/
    /// hovered highlight exists yet (`render3d.rs::build_scene` tints
    /// every node by category only, confirmed by direct read — no
    /// selection-aware branch), and adding a screen-space ring around a
    /// selected node's projected position (without the matching 3D-side
    /// glow 2D's own selection ring visually pairs with) was explicitly
    /// scoped out by this task's own instruction ("if not, skip — labels
    /// + card are the deliverable"). A natural Wave 5+ follow-up once a
    /// 3D-side highlight exists to pair it with.
    ///
    /// **Wave 5**: while [`GraphEngine3D::grid_enabled`], also paints a
    /// numeric axis-tick label for every STRONG gridline — see
    /// [`GraphEngine3D::draw_grid_overlay`]'s own doc comment for why
    /// this is a direct walk of `crate::render3d`'s `GridLine`s rather
    /// than a `label_grid::LabelGrid` pass.
    pub fn draw_overlay(&self, render: &mut dyn RenderContext, camera: &PerspectiveCamera, viewport: Rect) -> OverlayDrawStats {
        let max_degree = self.graph.nodes().map(|(id, _)| self.graph.degree(id)).max().unwrap_or(0).max(1);
        let zoom_analog = (Camera3D::default().distance / self.camera.distance.max(1e-3)) as f64;

        let mut labels_drawn = 0usize;
        for (id, sx, sy) in self.visible_labels(camera, viewport) {
            let Some(node) = self.graph.get_node(id) else { continue };
            let normalized_degree = self.graph.degree(id) as f64 / max_degree as f64;
            let alpha = label_grid::label_alpha(zoom_analog, normalized_degree);
            if alpha <= 0.01 {
                continue;
            }
            render.set_global_alpha(alpha);
            render.set_fill_color("#e6e6ea");
            render.set_font("11px sans-serif");
            render.fill_text(&node.label, sx + OVERLAY_LABEL_OFFSET_X, sy + OVERLAY_LABEL_OFFSET_Y);
            render.set_global_alpha(1.0);
            labels_drawn += 1;
        }

        let grid_labels_drawn = if self.grid_enabled { self.draw_grid_overlay(render, camera, viewport) } else { 0 };

        let mut hover_card_drawn = false;
        if let Some(hovered) = self.hovered {
            if let (Some(facts), Some(p)) = (self.node_facts(hovered), self.particles.get(hovered.index())) {
                let world = Vec3::new(p.x, p.y, p.z);
                if let Some(anchor) = pick3d::project_world_to_screen(camera, world, viewport) {
                    let info = HoverCardInfo { label: facts.label, category: facts.category, degree: facts.degree, pinned: facts.pinned };
                    draw_hover_card(render, anchor, &info, viewport);
                    hover_card_drawn = true;
                }
            }
        }

        OverlayDrawStats { labels_drawn, grid_labels_drawn, hover_card_drawn }
    }

    /// Axis tick labels for every STRONG gridline (Wave 5) — see
    /// `crate::render3d`'s own module doc for why this is a direct walk
    /// of the same [`crate::render3d::GridLine`] list
    /// [`GraphEngine3D::build_scene`] instances from, not a
    /// `label_grid::LabelGrid` pass: axis ticks are already sparse and
    /// perfectly regular, so a flat off-viewport cull plus a count cap
    /// ([`crate::render3d::GRID_MAX_AXIS_LABELS`]) is the whole LOD this
    /// needs. Recomputes the SAME `crate::render3d::grid_step_for_scale`/
    /// `build_grid_plan` [`GraphEngine3D::build_scene`] used, from
    /// `viewport.height` — a caller feeding a different height here than
    /// it fed `build_scene` this frame would see labels that don't quite
    /// match the rendered grid; the demo wiring keeps both fed from the
    /// SAME real surface height every tick, per this method's own
    /// contract. Returns `0` (no-op) once there isn't a single particle.
    fn draw_grid_overlay(&self, render: &mut dyn RenderContext, camera: &PerspectiveCamera, viewport: Rect) -> usize {
        let Some((min, max)) = particle_aabb(&self.particles) else { return 0 };
        let step = crate::render3d::grid_step_for_scale(self.camera.distance, camera.fov_y, viewport.height);
        let plan = crate::render3d::build_grid_plan(min, max, step);

        render.set_font("10px sans-serif");
        render.set_fill_color("#8a93a6");
        let mut drawn = 0usize;
        for line in plan.lines.iter().filter(|l| l.strong) {
            if drawn >= crate::render3d::GRID_MAX_AXIS_LABELS {
                break;
            }
            let Some((sx, sy)) = pick3d::project_world_to_screen(camera, line.from, viewport) else { continue };
            if sx < viewport.x || sx > viewport.x + viewport.width || sy < viewport.y || sy > viewport.y + viewport.height {
                continue;
            }
            let text = crate::render3d::format_tick_value(line.tick_value, plan.step);
            render.fill_text(&text, sx + OVERLAY_LABEL_OFFSET_X, sy + OVERLAY_LABEL_OFFSET_Y);
            drawn += 1;
        }
        drawn
    }
}

/// Per-frame draw counts for [`GraphEngine3D::draw_overlay`] — a
/// test/verification aid (mirrors [`crate::render::NodeDrawStats`]'s own
/// role for the 2D engine), not consumed by any production call site.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverlayDrawStats {
    pub labels_drawn: usize,
    /// Numeric axis-tick labels painted this frame (Wave 5) — always `0`
    /// while [`GraphEngine3D::grid_enabled`] is `false`.
    pub grid_labels_drawn: usize,
    pub hover_card_drawn: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    // ── `RecordingRenderContext` — Wave 4's `draw_overlay` test double ──
    //
    // Same boilerplate-minimal "implement every RenderContext supertrait
    // with a no-op body" pattern already used throughout the workspace
    // (`uzor/src/core/render/context.rs`'s own `NoImageContext` test,
    // `uzor/src/ui/themes/macos/widgets/switch_toggle.rs`'s own
    // `MockContext`) — the ONE addition here is recording every
    // `fill_text` call (text + position), since `draw_overlay`'s own
    // gate is "assert via a recording RenderContext mock", not just
    // "compiles against a mock".

    struct RecordingRenderContext {
        fill_texts: Vec<(String, f64, f64)>,
    }

    impl RecordingRenderContext {
        fn new() -> Self {
            Self { fill_texts: Vec::new() }
        }
    }

    impl uzor::render::Painter for RecordingRenderContext {
        fn save(&mut self) {}
        fn restore(&mut self) {}
        fn translate(&mut self, _x: f64, _y: f64) {}
        fn rotate(&mut self, _angle: f64) {}
        fn scale(&mut self, _x: f64, _y: f64) {}
        fn set_fill_color(&mut self, _color: &str) {}
        fn set_global_alpha(&mut self, _alpha: f64) {}
        fn set_stroke_color(&mut self, _color: &str) {}
        fn set_stroke_width(&mut self, _width: f64) {}
        fn set_line_dash(&mut self, _pattern: &[f64]) {}
        fn set_line_cap(&mut self, _cap: &str) {}
        fn set_line_join(&mut self, _join: &str) {}
        fn begin_path(&mut self) {}
        fn move_to(&mut self, _x: f64, _y: f64) {}
        fn line_to(&mut self, _x: f64, _y: f64) {}
        fn close_path(&mut self) {}
        fn rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn arc(&mut self, _cx: f64, _cy: f64, _r: f64, _s: f64, _e: f64) {}
        fn ellipse(&mut self, _cx: f64, _cy: f64, _rx: f64, _ry: f64, _rot: f64, _s: f64, _e: f64) {}
        fn quadratic_curve_to(&mut self, _cpx: f64, _cpy: f64, _x: f64, _y: f64) {}
        fn bezier_curve_to(&mut self, _cp1x: f64, _cp1y: f64, _cp2x: f64, _cp2y: f64, _x: f64, _y: f64) {}
        fn stroke(&mut self) {}
        fn fill(&mut self) {}
    }
    impl uzor::render::TextRenderer for RecordingRenderContext {
        fn set_font(&mut self, _font: &str) {}
        fn set_text_align(&mut self, _align: uzor::render::TextAlign) {}
        fn set_text_baseline(&mut self, _baseline: uzor::render::TextBaseline) {}
        fn fill_text(&mut self, text: &str, x: f64, y: f64) {
            self.fill_texts.push((text.to_owned(), x, y));
        }
        fn stroke_text(&mut self, _text: &str, _x: f64, _y: f64) {}
    }
    impl uzor::render::TextMetrics for RecordingRenderContext {
        fn measure_text(&self, _text: &str) -> f64 {
            0.0
        }
        fn text_bounds(&self, _text: &str, _font: &str) -> uzor::render::TextBounds {
            uzor::render::TextBounds { x: 0.0, y: 0.0, w: 0.0, h: 0.0, ascent: 0.0, descent: 0.0 }
        }
    }
    impl uzor::render::Masking for RecordingRenderContext {
        fn clip(&mut self) {}
    }
    impl uzor::render::Effects for RecordingRenderContext {}
    impl uzor::render::ShapeHelpers for RecordingRenderContext {
        fn fill_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
        fn stroke_rect(&mut self, _x: f64, _y: f64, _w: f64, _h: f64) {}
    }
    impl uzor::render::GradientPainter for RecordingRenderContext {}
    impl uzor::render::UiEffectHelpers for RecordingRenderContext {}
    impl uzor::render::BatchPainter for RecordingRenderContext {}
    impl uzor::render::RenderContext for RecordingRenderContext {
        fn dpr(&self) -> f64 {
            1.0
        }
    }

    type DemoGraph = Graph<(), ()>;

    fn triangle() -> DemoGraph {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        graph.push_edge(c, a, 1.0, ());
        graph
    }

    #[test]
    fn new_seeds_one_particle_per_node_and_ticks_without_panicking() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        assert_eq!(engine.particles.len(), 3);
        let result = engine.tick(1.0 / 60.0);
        assert!(result.alpha <= 1.0);
    }

    #[test]
    fn build_scene_returns_one_instanced_sphere_per_node_and_one_edge_quad_per_edge() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        // `new()` seeds every particle at the origin — non-coincident
        // positions are needed here so none of the 3 edges gets skipped
        // as degenerate (see `render3d.rs`'s own
        // `build_edge_instances_skips_a_coincident_degenerate_edge`).
        engine.particles[0] = Particle::at3(-4.0, 0.0, 0.0);
        engine.particles[1] = Particle::at3(4.0, 0.0, 0.0);
        engine.particles[2] = Particle::at3(0.0, 4.0, 0.0);

        let scene = engine.build_scene(900.0);

        // Triangle fixture: 3 nodes, 3 edges.
        assert_eq!(scene.nodes.len(), 6, "build_scene must emit one Node per graph node plus one per edge (Wave 2)");
        assert!(!scene.lights.is_empty(), "build_scene must light the scene so MeshLit tints are visible");
    }

    #[test]
    fn on_event_plain_drag_orbits_the_camera_and_is_consumed() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_yaw = engine.camera.yaw;

        assert!(engine.on_event(&PlatformEvent::PointerDown { x: 50.0, y: 50.0, button: uzor::input::MouseButton::Left }, viewport));
        assert!(engine.on_event(&PlatformEvent::PointerMoved { x: 150.0, y: 50.0 }, viewport));
        assert!(engine.camera.yaw != before_yaw, "a plain left-drag must orbit the camera (plan §1.4)");
        assert!(engine.on_event(&PlatformEvent::PointerUp { x: 150.0, y: 50.0, button: uzor::input::MouseButton::Left }, viewport));

        let handled_outside =
            engine.on_event(&PlatformEvent::PointerDown { x: -10.0, y: -10.0, button: uzor::input::MouseButton::Left }, viewport);
        assert!(!handled_outside, "a pointer-down outside the viewport must not start a drag");
    }

    #[test]
    fn on_event_shift_drag_pans_instead_of_orbiting() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_yaw = engine.camera.yaw;
        let before_target = engine.camera.target;

        engine.on_event(&PlatformEvent::ModifiersChanged { modifiers: uzor::input::ModifierKeys::shift() }, viewport);
        engine.on_event(&PlatformEvent::PointerDown { x: 50.0, y: 50.0, button: uzor::input::MouseButton::Left }, viewport);
        engine.on_event(&PlatformEvent::PointerMoved { x: 150.0, y: 50.0 }, viewport);

        assert_eq!(engine.camera.yaw, before_yaw, "shift-drag must pan, not orbit — yaw stays put");
        assert!(engine.camera.target != before_target, "shift-drag must move the pan target");
    }

    #[test]
    fn on_event_middle_drag_pans_camera_target() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_yaw = engine.camera.yaw;
        let before_target = engine.camera.target;

        assert!(engine.on_event(&PlatformEvent::PointerDown {
            x: 50.0,
            y: 50.0,
            button: uzor::input::MouseButton::Middle,
        }, viewport));
        assert!(engine.on_event(&PlatformEvent::PointerMoved { x: 150.0, y: 90.0 }, viewport));

        assert_eq!(engine.camera.yaw, before_yaw, "middle-drag must pan, not orbit");
        assert!(engine.camera.target != before_target, "middle-drag must move the camera target");
        assert!(engine.on_event(&PlatformEvent::PointerUp {
            x: 150.0,
            y: 90.0,
            button: uzor::input::MouseButton::Middle,
        }, viewport));
    }

    #[test]
    fn on_event_scroll_dollies_only_when_the_cursor_is_over_the_viewport() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_distance = engine.camera.distance;

        engine.on_event(&PlatformEvent::PointerMoved { x: 200.0, y: 150.0 }, viewport);
        assert!(engine.on_event(&PlatformEvent::Scroll { dx: 0.0, dy: 10.0 }, viewport));
        assert!(engine.camera.distance < before_distance, "positive scroll dy must dolly in (smaller distance)");

        let before_distance = engine.camera.distance;
        engine.on_event(&PlatformEvent::PointerMoved { x: -50.0, y: -50.0 }, viewport);
        let handled = engine.on_event(&PlatformEvent::Scroll { dx: 0.0, dy: 10.0 }, viewport);
        assert!(!handled, "scroll must not be consumed while the cursor sits outside the viewport");
        assert_eq!(engine.camera.distance, before_distance);
    }

    // ── Wave 3: hover/click picking, node_facts, visible_labels ────────────

    /// Fixture: triangle with distinct, well-separated 3D positions so a
    /// `PointerMoved` over one node's projected screen position never
    /// accidentally also lands on another node's projection.
    fn spread_triangle_engine() -> GraphEngine3D<(), (), ForceDirectedLayout3D> {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        engine.particles[0] = Particle::at3(-40.0, 0.0, 0.0);
        engine.particles[1] = Particle::at3(40.0, 0.0, 0.0);
        engine.particles[2] = Particle::at3(0.0, 40.0, 0.0);
        engine
    }

    #[test]
    fn on_event_pointer_moved_over_a_projected_node_hovers_it_and_a_click_selects_it() {
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        let (sx, sy) = pick3d::project_world_to_screen(&camera, Vec3::new(-40.0, 0.0, 0.0), viewport)
            .expect("node 0 must project inside the default orbit view for this deterministic fixture");

        assert_eq!(engine.hovered(), None, "nothing is hovered before any pointer event");
        assert!(engine.on_event(&PlatformEvent::PointerMoved { x: sx, y: sy }, viewport));
        assert_eq!(engine.hovered(), Some(NodeIndex(0)), "moving over node 0's own projected screen position must hover it");

        assert_eq!(engine.selected(), None, "nothing is selected before any click");
        assert!(engine.on_event(&PlatformEvent::PointerDown { x: sx, y: sy, button: uzor::input::MouseButton::Left }, viewport));
        assert!(engine.on_event(&PlatformEvent::PointerUp { x: sx, y: sy, button: uzor::input::MouseButton::Left }, viewport));
        assert_eq!(engine.selected(), Some(NodeIndex(0)), "a down/up pair at the same spot (within the click-drag threshold) must select it");
    }

    #[test]
    fn on_event_a_background_drag_past_the_click_threshold_orbits_but_does_not_select() {
        // Node-drag (added later in this same file's Wave 4/5 test
        // block below) means a `PointerDown` ON a node no longer
        // orbits at all — it ALWAYS drags that node and ALWAYS selects
        // it on release, regardless of drag distance (there's no
        // ambiguous "was this a click or a drag" question for a node
        // hit the way there is for background). This test now starts
        // on EMPTY SPACE, the only case the click-vs-drag distance
        // threshold still governs.
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);

        engine.on_event(&PlatformEvent::PointerDown { x: 5.0, y: 5.0, button: uzor::input::MouseButton::Left }, viewport);
        engine.on_event(&PlatformEvent::PointerMoved { x: 65.0, y: 5.0 }, viewport);
        engine.on_event(&PlatformEvent::PointerUp { x: 65.0, y: 5.0, button: uzor::input::MouseButton::Left }, viewport);

        assert_eq!(engine.selected(), None, "a background drag past the click-drag threshold must orbit, not select");
    }

    #[test]
    fn on_event_click_on_empty_space_clears_the_selection() {
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        let (sx, sy) = pick3d::project_world_to_screen(&camera, Vec3::new(-40.0, 0.0, 0.0), viewport).expect("node 0 must project inside the view");
        engine.on_event(&PlatformEvent::PointerDown { x: sx, y: sy, button: uzor::input::MouseButton::Left }, viewport);
        engine.on_event(&PlatformEvent::PointerUp { x: sx, y: sy, button: uzor::input::MouseButton::Left }, viewport);
        assert_eq!(engine.selected(), Some(NodeIndex(0)));

        // Click far from every node's projection — empty space.
        engine.on_event(&PlatformEvent::PointerDown { x: 5.0, y: 5.0, button: uzor::input::MouseButton::Left }, viewport);
        engine.on_event(&PlatformEvent::PointerUp { x: 5.0, y: 5.0, button: uzor::input::MouseButton::Left }, viewport);
        assert_eq!(engine.selected(), None, "clicking empty space must clear the previous selection");
    }

    #[test]
    fn node_facts_reuses_the_2d_engines_nodefacts_shape() {
        let engine = spread_triangle_engine();
        let facts = engine.node_facts(NodeIndex(0)).expect("node 0 exists in the triangle fixture");
        assert_eq!(facts.index, NodeIndex(0));
        assert_eq!(facts.label, "a");
        assert_eq!(facts.category, "x");
        assert_eq!(facts.degree, 2);
        assert_eq!(facts.position, (-40.0, 0.0), "position reports (x, y) only — z is a known Wave 3 divergence, see CLAUDE.md");
        assert!(!facts.pinned, "nothing is pinned this wave (no 3D node-drag/pin yet)");

        assert!(engine.node_facts(NodeIndex(99)).is_none(), "an out-of-range node must yield None, not panic");
    }

    #[test]
    fn visible_labels_returns_a_screen_position_for_every_unoccluded_node() {
        let mut engine = spread_triangle_engine();
        // At the default orbit distance the 3 fixture nodes project close
        // enough together to share a single 100px `label_grid` cell — a
        // high density removes the LOD quota as a confound so this test
        // proves the project+return plumbing itself, not the (separately
        // tested, in `label_grid.rs`) quota math.
        engine.set_label_density(100.0);
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);

        let labels = engine.visible_labels(&camera, viewport);

        assert_eq!(labels.len(), 3, "all 3 well-separated triangle nodes should get a label slot at this camera/density");
        let ids: std::collections::HashSet<NodeIndex> = labels.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(ids, [NodeIndex(0), NodeIndex(1), NodeIndex(2)].into_iter().collect());

        // Each returned screen position must match `project_world_to_screen`
        // for that node's own particle position — no drift between the
        // LOD pass and the underlying projection.
        for (id, sx, sy) in labels {
            let p = engine.particles[id.index()];
            let expected = pick3d::project_world_to_screen(&camera, Vec3::new(p.x, p.y, p.z), viewport).expect("visible node must project");
            assert!((sx - expected.0).abs() < 1e-6 && (sy - expected.1).abs() < 1e-6);
        }
    }

    #[test]
    fn label_density_defaults_and_set_label_density_updates_the_getter_and_clamps_negative_input() {
        let mut engine = spread_triangle_engine();
        assert_eq!(engine.label_density(), label_grid::DEFAULT_LABEL_DENSITY);

        engine.set_label_density(3.5);
        assert_eq!(engine.label_density(), 3.5);

        engine.set_label_density(-1.0);
        assert_eq!(engine.label_density(), 0.0, "negative density must clamp to 0.0, not go negative");
    }

    // ── Live-caught Wave 2 defect, fixed in Wave 3: z-plane degeneracy ─────
    //
    // Every 3D force in `ForceDirectedLayout3D` is z-symmetric (repulsion/
    // link/center all scale the SAME `(dx, dy, dz)` direction vector), so
    // seeding every node at `z = 0` (as `force_graph_demo`'s old manual
    // `p.x = x; p.y = y;` loop did, leaving `z` at its `Particle::default()`
    // value) makes the z-force identically zero forever — the sim can
    // never leave the plane on its own. `seed_positions` is the fix.

    #[test]
    fn seed_positions_escapes_a_degenerate_z_plane_and_settles_into_a_true_3d_volume() {
        let mut graph = DemoGraph::new();
        let n = 20;
        for i in 0..n {
            graph.push_node((), format!("n{i}"), "x", 4.0);
        }
        // Ring positions in the xy plane — every particle starts at
        // `z = 0` (`Particle::default()`), the exact degenerate seed the
        // owner's live screenshot caught.
        let positions: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                (a.cos() * 100.0, a.sin() * 100.0)
            })
            .collect();

        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> = GraphEngine3D::new(graph, ForceDirectedLayout3D::default());
        engine.seed_positions(&positions);

        assert!(engine.particles.iter().any(|p| p.z != 0.0), "seed_positions must jitter z off the degenerate plane immediately, before any tick");

        let mut settled = false;
        for _ in 0..2000 {
            let r = engine.tick(1.0 / 60.0);
            for p in &engine.particles {
                assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "position went non-finite: {p:?}");
            }
            if r.settled {
                settled = true;
                break;
            }
        }
        assert!(settled, "the sim must settle within 2000 ticks after the z-jitter reheat");

        let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
        let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
        let (mut min_z, mut max_z) = (f32::MAX, f32::MIN);
        for p in &engine.particles {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }
        let xy_extent = (max_x - min_x).max(max_y - min_y);
        let z_extent = max_z - min_z;
        assert!(
            z_extent > xy_extent * 0.1,
            "the settled sim must occupy a true 3D volume, not a flat pancake: xy_extent={xy_extent}, z_extent={z_extent}"
        );
    }

    #[test]
    fn seed_positions_z_jitter_is_deterministic_across_identical_runs() {
        let build_graph = || {
            let mut graph = DemoGraph::new();
            for i in 0..10 {
                graph.push_node((), format!("n{i}"), "x", 4.0);
            }
            graph
        };
        let positions: Vec<(f32, f32)> = (0..10).map(|i| (i as f32 * 15.0, 0.0)).collect();

        let mut e1: GraphEngine3D<(), (), ForceDirectedLayout3D> = GraphEngine3D::new(build_graph(), ForceDirectedLayout3D::default());
        e1.seed_positions(&positions);
        let mut e2: GraphEngine3D<(), (), ForceDirectedLayout3D> = GraphEngine3D::new(build_graph(), ForceDirectedLayout3D::default());
        e2.seed_positions(&positions);

        for (p1, p2) in e1.particles.iter().zip(e2.particles.iter()) {
            assert_eq!(p1.z, p2.z, "identical seeds must jitter to the identical z on every run — no Math::random/time");
        }
    }

    #[test]
    fn ensure_z_variance_leaves_an_already_3d_seed_untouched() {
        let mut engine = spread_triangle_engine();
        engine.particles[0].z = 30.0;
        engine.particles[1].z = -25.0;
        engine.particles[2].z = 15.0;
        let before: Vec<f32> = engine.particles.iter().map(|p| p.z).collect();

        engine.ensure_z_variance();

        let after: Vec<f32> = engine.particles.iter().map(|p| p.z).collect();
        assert_eq!(before, after, "an intentional, already-varied z seed must not be disturbed");
    }

    #[test]
    fn ensure_z_variance_is_a_no_op_on_a_fully_degenerate_xy_seed() {
        // Right after `new()` every particle sits at the origin — no
        // meaningful xy scale exists yet to derive a jitter range from.
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        engine.ensure_z_variance();
        assert!(engine.particles.iter().all(|p| p.z == 0.0), "with a degenerate xy seed too, there's nothing to scale a jitter against");
    }

    #[test]
    fn camera_reflects_the_default_orbit_state() {
        let engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        let persp = engine.camera(16.0 / 9.0);
        assert!((persp.eye - engine.camera.eye()).length() < 1e-4);
    }

    // ── Wave 4: `draw_overlay` — label + hover-card overlay draw calls ──

    #[test]
    fn draw_overlay_paints_label_text_for_every_visible_node_in_a_deterministic_fixture() {
        let mut engine = spread_triangle_engine();
        // Same "remove the LOD quota as a confound" convention as
        // `visible_labels_returns_a_screen_position_for_every_unoccluded_node`
        // — this test's own job is proving the paint plumbing, not
        // re-proving `label_grid.rs`'s already-covered quota math.
        engine.set_label_density(100.0);
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        let mut ctx = RecordingRenderContext::new();

        let stats = engine.draw_overlay(&mut ctx, &camera, viewport);

        assert_eq!(stats.labels_drawn, 3, "all 3 well-separated triangle nodes must get a painted label at this camera/density");
        assert_eq!(ctx.fill_texts.len(), 3, "draw_overlay must issue exactly one fill_text draw call per shown label");
        let drawn_labels: HashSet<&str> = ctx.fill_texts.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(drawn_labels, HashSet::from(["a", "b", "c"]), "every fixture node's own label text must be drawn");
        assert!(!stats.hover_card_drawn, "nothing is hovered in this fixture, so no hover card should be painted");
    }

    #[test]
    fn draw_overlay_culls_a_label_for_a_node_behind_the_camera() {
        let mut engine = spread_triangle_engine();
        engine.set_label_density(100.0);
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);

        // Move node 0 directly behind the camera eye, opposite the view
        // direction — `project_world_to_screen`'s own behind-the-eye
        // guard (`clip.w <= 1e-5`) must reject it, and `visible_labels`
        // (which `draw_overlay` iterates) must therefore never surface
        // it as a label candidate.
        let forward = (camera.target - camera.eye).normalize();
        let behind = camera.eye - forward * 50.0;
        engine.particles[0] = Particle::at3(behind.x, behind.y, behind.z);
        assert!(
            pick3d::project_world_to_screen(&camera, behind, viewport).is_none(),
            "fixture sanity check: the behind-camera point must itself fail to project"
        );

        let mut ctx = RecordingRenderContext::new();
        let stats = engine.draw_overlay(&mut ctx, &camera, viewport);

        assert_eq!(stats.labels_drawn, 2, "the behind-camera node's label must be culled, the other two must still draw");
        assert!(
            !ctx.fill_texts.iter().any(|(t, _, _)| t == "a"),
            "node 0's own label text must never be drawn while it sits behind the camera"
        );
    }

    #[test]
    fn draw_overlay_paints_a_hover_card_for_the_hovered_node() {
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        engine.hovered = Some(NodeIndex(0));

        let mut ctx = RecordingRenderContext::new();
        let stats = engine.draw_overlay(&mut ctx, &camera, viewport);

        assert!(stats.hover_card_drawn, "a hovered node with a valid projection must paint a hover card");
        // `draw_hover_card` (`crate::render::draw_hover_card`, reused
        // verbatim from the 2D engine) issues a key/value `fill_text`
        // pair per `NodeFacts` field — the hovered node's own label
        // text must show up among the card's drawn text.
        assert!(
            ctx.fill_texts.iter().any(|(t, _, _)| t == "a"),
            "the hover card must show the hovered node's own label text"
        );
    }

    #[test]
    fn draw_overlay_draws_no_hover_card_when_nothing_is_hovered() {
        let engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);

        let mut ctx = RecordingRenderContext::new();
        let stats = engine.draw_overlay(&mut ctx, &camera, viewport);

        assert!(!stats.hover_card_drawn, "no hover, no card — draw_overlay must not paint a hover card without a hovered node");
    }

    // ── Wave 4: GPU color-ID picking escalation ─────────────────────────

    #[test]
    fn should_use_gpu_pick_reflects_the_threshold_override() {
        let mut engine = spread_triangle_engine(); // 3 nodes
        assert_eq!(engine.gpu_pick_threshold(), pick3d::GPU_PICK_NODE_THRESHOLD);
        assert!(!engine.should_use_gpu_pick(), "3 nodes must stay under the default 10_000 threshold");

        engine.set_gpu_pick_threshold(2);
        assert_eq!(engine.gpu_pick_threshold(), 2);
        assert!(engine.should_use_gpu_pick(), "3 nodes > an overridden threshold of 2 must flip to the GPU path");
    }

    #[test]
    fn hover_above_the_gpu_pick_threshold_starts_exactly_one_gpu_pick_request() {
        let mut engine = spread_triangle_engine();
        engine.set_gpu_pick_threshold(2);
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);

        assert_eq!(engine.gpu_pick_requests_started(), 0);
        engine.on_event(&PlatformEvent::PointerMoved { x: 200.0, y: 150.0 }, viewport);
        assert_eq!(engine.gpu_pick_requests_started(), 1, "an above-threshold hover move must start a GPU pick request");
        assert!(engine.gpu_pick_pending());
    }

    #[test]
    fn hover_below_the_gpu_pick_threshold_never_touches_the_gpu_pick_pipeline() {
        let mut engine = spread_triangle_engine(); // default threshold, 3 nodes well under it
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);

        engine.on_event(&PlatformEvent::PointerMoved { x: 200.0, y: 150.0 }, viewport);
        assert_eq!(engine.gpu_pick_requests_started(), 0);
        assert!(!engine.gpu_pick_pending());
    }

    #[test]
    fn hover_still_resolves_via_cpu_pick_as_the_same_frame_answer_even_above_the_gpu_pick_threshold() {
        let mut engine = spread_triangle_engine();
        engine.set_gpu_pick_threshold(2);
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        let (sx, sy) = pick3d::project_world_to_screen(&camera, Vec3::new(-40.0, 0.0, 0.0), viewport).expect("node 0 projects inside the view");

        engine.on_event(&PlatformEvent::PointerMoved { x: sx, y: sy }, viewport);

        assert_eq!(engine.hovered(), Some(NodeIndex(0)), "even in GPU-pick mode, the CPU ray-pick must still resolve this SAME frame");
        assert!(engine.gpu_pick_pending(), "a GPU refinement request must also be in flight in parallel");
    }

    #[test]
    fn apply_gpu_pick_result_refines_hovered_once_the_deferred_readback_resolves() {
        let mut engine = spread_triangle_engine();
        engine.set_gpu_pick_threshold(2);
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        engine.on_event(&PlatformEvent::PointerMoved { x: 200.0, y: 150.0 }, viewport);
        assert!(engine.gpu_pick_pending());

        engine.apply_gpu_pick_result(Some(NodeIndex(2)));

        assert!(!engine.gpu_pick_pending(), "applying the result must consume the in-flight request");
        assert_eq!(engine.hovered(), Some(NodeIndex(2)), "the resolved GPU pick result must refine hovered");
    }

    #[test]
    fn apply_gpu_pick_result_with_nothing_in_flight_is_a_no_op() {
        let mut engine = spread_triangle_engine();
        let before = engine.hovered();

        engine.apply_gpu_pick_result(Some(NodeIndex(1)));

        assert_eq!(engine.hovered(), before, "feeding a stray GPU result with no request in flight must not touch hovered");
    }

    #[test]
    fn build_id_pass_scene_emits_one_unlit_node_per_graph_node() {
        let engine = spread_triangle_engine();

        let scene = engine.build_id_pass_scene();

        assert_eq!(scene.nodes.len(), 3);
        assert!(scene.nodes.iter().all(|n| !n.is_lit()), "the id-pass must use Unlit geometry");
        assert_eq!(scene.clear_color, [1.0, 1.0, 1.0, 1.0]);
    }

    // ── Owner-ordered live fix: 3D node drag ────────────────────────────

    #[test]
    fn pointer_down_on_a_node_drags_it_along_the_camera_parallel_plane_and_it_stays_pinned_after_release() {
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        let (sx, sy) = pick3d::project_world_to_screen(&camera, Vec3::new(-40.0, 0.0, 0.0), viewport).expect("node 0 must project inside the default orbit view");

        assert!(engine.on_event(&PlatformEvent::PointerDown { x: sx, y: sy, button: uzor::input::MouseButton::Left }, viewport));
        assert!(engine.particles[0].is_pinned_3d(), "drag-start must pin the grabbed node immediately");

        let move_x = sx + 30.0;
        let move_y = sy + 15.0;
        assert!(engine.on_event(&PlatformEvent::PointerMoved { x: move_x, y: move_y }, viewport));

        // Independently recompute the expected ray-plane intersection —
        // NOT copy-pasted from `on_pointer_moved`'s own implementation —
        // to actually prove the production wiring, not just restate it.
        let (origin, dir) = pick3d::screen_to_ray(&camera, viewport, (move_x, move_y));
        let plane_point = Vec3::new(-40.0, 0.0, 0.0);
        let plane_normal = (camera.target - camera.eye).normalize_or_zero();
        let expected = pick3d::ray_plane_intersection(origin, dir, plane_point, plane_normal).expect("the moved cursor ray must still cross the drag plane");

        let p = engine.particles[0];
        assert!(
            (p.x - expected.x).abs() < 1e-3 && (p.y - expected.y).abs() < 1e-3 && (p.z - expected.z).abs() < 1e-3,
            "dragged node must move to the exact ray-plane intersection: got ({}, {}, {}), expected {expected:?}",
            p.x,
            p.y,
            p.z
        );
        assert!(p.is_pinned_3d(), "a node being actively dragged must stay pinned so the sim doesn't fight the drag");

        assert!(engine.on_event(&PlatformEvent::PointerUp { x: move_x, y: move_y, button: uzor::input::MouseButton::Left }, viewport));
        assert_eq!(engine.selected(), Some(NodeIndex(0)), "releasing a node drag must select the dragged node");

        let released_pos = (engine.particles[0].x, engine.particles[0].y, engine.particles[0].z);
        for _ in 0..30 {
            engine.tick(1.0 / 60.0);
        }
        let after = engine.particles[0];
        assert_eq!(
            (after.x, after.y, after.z),
            released_pos,
            "Sticky drag-end policy: the node must stay exactly where it was released, unaffected by subsequent ticks"
        );
        assert!(after.is_pinned_3d(), "Sticky policy must leave the node pinned after release");
    }

    #[test]
    fn pointer_down_on_empty_space_still_orbits_the_camera_and_drags_no_node() {
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let before_positions: Vec<Particle> = engine.particles.clone();
        let before_yaw = engine.camera.yaw;

        // Far from every fixture node's projected screen position —
        // background.
        assert!(engine.on_event(&PlatformEvent::PointerDown { x: 5.0, y: 5.0, button: uzor::input::MouseButton::Left }, viewport));
        assert!(engine.on_event(&PlatformEvent::PointerMoved { x: 105.0, y: 5.0 }, viewport));

        assert!(engine.camera.yaw != before_yaw, "a miss on pointer-down must fall back to orbiting the camera exactly as before this fix");
        for (before, after) in before_positions.iter().zip(engine.particles.iter()) {
            assert_eq!((before.x, before.y, before.z), (after.x, after.y, after.z), "no node position may move from an orbit drag");
            assert!(!after.is_pinned_3d(), "no node may become pinned from an orbit drag");
        }
    }

    // ── Round 2 of the edge-quality overhaul: sphere tessellation bump ──

    /// A headless-GPU pixel-level "no long straight facet run" silhouette
    /// check turned out too fiddly to make robust at this crate's own
    /// 128×128 test-target resolution (a facet edge's own screen-space
    /// length depends on camera distance/fov in a way that's easy to
    /// mistune into either a flaky test or a vacuously-passing one) — per
    /// the task's own "if too fiddly headlessly, document and leave to
    /// the coordinator's visual check" allowance, that's what this test
    /// does NOT attempt. What it DOES prove, cheaply and robustly: the
    /// actual tessellation density the owner asked for
    /// ("28-32 longitudinal / 18-24 latitudinal") is really wired into
    /// the shared node-sphere mesh every graph node instances from — a
    /// regression that silently dropped `NODE_SPHERE_RINGS`/`SLICES`
    /// back down would otherwise pass every other test in this file
    /// (none of them inspect mesh density).
    #[test]
    fn shared_node_sphere_mesh_uses_the_re_tuned_tessellation_density() {
        let engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());

        assert_eq!(NODE_SPHERE_SLICES, 30, "longitudinal segments must land in the owner's requested 28-32 band");
        assert!((18..=24).contains(&NODE_SPHERE_RINGS), "latitudinal bands must land in the owner's requested 18-24 band");

        let mesh = engine.node_mesh();
        let expected_vertices = ((NODE_SPHERE_RINGS + 1) * (NODE_SPHERE_SLICES + 1)) as usize;
        let expected_indices = (NODE_SPHERE_RINGS * NODE_SPHERE_SLICES * 6) as usize;
        assert_eq!(mesh.vertices.len(), expected_vertices, "the shared node-sphere mesh's own vertex grid must match rings/slices exactly");
        assert_eq!(mesh.indices.len(), expected_indices, "the shared node-sphere mesh's own index count must match rings/slices exactly");

        // Every vertex's own normal must equal its (unit-radius) position
        // direction — the per-vertex, not per-face, smoothness [`crate::render3d`]'s
        // own module doc already asserted from a direct code read; this
        // re-confirms it holds at the NEW, bumped density too (a
        // regression that duplicated vertices per-face, breaking smooth
        // shading, wouldn't show up in the count assertions above).
        for v in &mesh.vertices {
            let p = glam::Vec3::from_array(v.pos);
            let n = glam::Vec3::from_array(v.normal);
            assert!((p.normalize() - n).length() < 1e-4, "vertex normal must equal its own unit-sphere position direction: pos={p:?} normal={n:?}");
        }
    }

    // ── Wave 5: Camera3D::fit_bounds fit_view wiring ────────────────────

    #[test]
    fn fit_view_frames_every_node_inside_the_ndc_unit_square_and_keeps_yaw_pitch() {
        let mut engine = spread_triangle_engine();
        engine.camera.yaw = 0.9;
        engine.camera.pitch = -0.25;
        let aspect = 16.0 / 9.0;

        engine.fit_view(aspect);

        assert_eq!(engine.camera.yaw, 0.9, "fit_view must keep the existing yaw — dolly+retarget only");
        assert_eq!(engine.camera.pitch, -0.25, "fit_view must keep the existing pitch");

        let persp = engine.camera(aspect);
        let view_proj = persp.view_proj();
        for p in &engine.particles {
            let world = Vec3::new(p.x, p.y, p.z);
            let clip = view_proj * world.extend(1.0);
            assert!(clip.w > 1e-5, "node at {world:?} must sit in front of the fitted camera");
            let ndc_x = clip.x / clip.w;
            let ndc_y = clip.y / clip.w;
            assert!(ndc_x.abs() <= 1.0 + 1e-3 && ndc_y.abs() <= 1.0 + 1e-3, "node at {world:?} escaped NDC: ({ndc_x}, {ndc_y})");
        }
    }

    #[test]
    fn fit_view_on_a_fresh_engine_with_a_degenerate_zero_extent_seed_falls_back_to_the_default_distance() {
        // Right after `new()`, every particle sits at the shared origin —
        // 3 nodes but a fully degenerate (zero-extent) AABB.
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        engine.camera.yaw = 0.4;

        engine.fit_view(16.0 / 9.0);

        assert_eq!(engine.camera.distance, Camera3D::default().distance);
        assert_eq!(engine.camera.target, Vec3::ZERO);
        assert_eq!(engine.camera.yaw, 0.4, "the degenerate fallback must still keep yaw/pitch untouched");
    }

    #[test]
    fn fit_view_on_too_few_nodes_falls_back_to_the_default_distance_even_with_a_real_extent() {
        let mut two_node_graph = DemoGraph::new();
        two_node_graph.push_node((), "a", "x", 4.0);
        two_node_graph.push_node((), "b", "x", 4.0);
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(two_node_graph, ForceDirectedLayout3D::default());
        engine.particles[0] = Particle::at3(-100.0, 0.0, 0.0);
        engine.particles[1] = Particle::at3(100.0, 0.0, 0.0);

        engine.fit_view(16.0 / 9.0);

        assert_eq!(engine.camera.distance, Camera3D::default().distance, "2 nodes is too few for a meaningful bounding-box fit, regardless of extent");
        assert_eq!(engine.camera.target, Vec3::ZERO);
    }

    #[test]
    fn fit_view_on_an_empty_graph_is_a_no_op() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(DemoGraph::new(), ForceDirectedLayout3D::default());
        let before = engine.camera;
        engine.fit_view(16.0 / 9.0);
        assert_eq!(engine.camera, before, "fit_view must not touch the camera when there are zero particles");
    }

    // ── Wave 5: ground-reference grid toggle + build_scene/draw_overlay ─

    #[test]
    fn grid_enabled_defaults_to_off_and_the_setter_toggles_it() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        assert!(!engine.grid_enabled(), "a force graph has no semantic axes of its own — the grid must default OFF");
        engine.set_grid_enabled(true);
        assert!(engine.grid_enabled());
        engine.set_grid_enabled(false);
        assert!(!engine.grid_enabled());
    }

    #[test]
    fn build_scene_appends_grid_lines_only_once_enabled() {
        let mut engine = spread_triangle_engine();

        let scene_without_grid = engine.build_scene(900.0);
        assert_eq!(scene_without_grid.nodes.len(), 6, "disabled grid must emit zero grid nodes — 3 spheres + 3 edges only");

        engine.set_grid_enabled(true);
        let scene_with_grid = engine.build_scene(900.0);
        assert!(
            scene_with_grid.nodes.len() > scene_without_grid.nodes.len(),
            "enabling the grid must append extra instanced line nodes"
        );
    }

    #[test]
    fn build_scene_grid_is_a_no_op_on_an_empty_graph() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(DemoGraph::new(), ForceDirectedLayout3D::default());
        engine.set_grid_enabled(true);
        let scene = engine.build_scene(900.0);
        assert!(scene.nodes.is_empty(), "an empty graph has no AABB to grid — must not panic or fabricate geometry");
    }

    #[test]
    fn draw_overlay_paints_axis_tick_labels_for_strong_gridlines_once_the_grid_is_enabled() {
        let mut engine = spread_triangle_engine();
        engine.set_grid_enabled(true);
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);

        let mut ctx = RecordingRenderContext::new();
        let stats = engine.draw_overlay(&mut ctx, &camera, viewport);

        assert!(stats.grid_labels_drawn > 0, "at least one strong gridline must be labeled once the grid is enabled");
        assert!(
            ctx.fill_texts.iter().any(|(text, _, _)| text.parse::<f64>().is_ok()),
            "a numeric axis-tick label must actually be painted once the grid is enabled: {:?}",
            ctx.fill_texts
        );
    }

    #[test]
    fn draw_overlay_paints_no_axis_tick_labels_while_the_grid_stays_disabled() {
        let engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);

        let mut ctx = RecordingRenderContext::new();
        let stats = engine.draw_overlay(&mut ctx, &camera, viewport);

        assert_eq!(stats.grid_labels_drawn, 0, "grid_labels_drawn must stay 0 while the grid defaults off");
        assert!(
            !ctx.fill_texts.iter().any(|(text, _, _)| text.parse::<f64>().is_ok()),
            "no numeric axis-tick label should be painted while the grid is off: {:?}",
            ctx.fill_texts
        );
    }
}
