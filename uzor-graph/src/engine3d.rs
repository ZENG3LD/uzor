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
use uzor_urx_3d::{MeshLit, PerspectiveCamera, Scene3D};

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
    /// Shift-drag pan (plan §1.4) — middle-drag is not wired this wave
    /// (the `PlatformEvent` stream this engine consumes only carries a
    /// `MouseButton::Left`-gated `PointerDown`/`Up`, matching the 2D
    /// engine's own `on_event` match arms; a middle-button chord is a
    /// natural, small follow-up if ever requested). Deliberately does
    /// NOT resolve a click-select on release — shift-drag is a
    /// dedicated pan gesture, not the plain-drag/click gesture.
    Panning { last: (f64, f64) },
}

/// Shared unit-sphere node mesh geometry (plan §1.3) — latitude/longitude
/// resolution tuned for a smooth silhouette at typical node screen sizes
/// without an excessive vertex count. One shared mesh serves every node
/// via `uzor-urx-3d`'s Arc-identity instancing, so this cost is paid
/// once per engine, not once per node.
const NODE_SPHERE_RINGS: u32 = 12;
const NODE_SPHERE_SLICES: u32 = 16;
/// Shared unit-cylinder edge mesh geometry (plan §1.3).
const EDGE_CYLINDER_SLICES: u32 = 8;

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
    /// Shared unit cylinder every edge instances from (plan §1.3).
    edge_mesh: Arc<MeshLit>,
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
            edge_mesh: Arc::new(MeshLit::cylinder(1.0, 1.0, EDGE_CYLINDER_SLICES, [1.0, 1.0, 1.0, 1.0])),
            modifiers: ModifierKeys::default(),
            mode: Pointer3DMode::Idle,
            last_pointer_screen: (0.0, 0.0),
            all_node_ids,
            last_hover_pick_screen: None,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
        }
    }

    /// Shared node-sphere mesh — exposed read-only so a future render
    /// pass (Wave 2) can reuse it without re-generating geometry.
    pub fn node_mesh(&self) -> &Arc<MeshLit> {
        &self.node_mesh
    }

    /// Shared edge-cylinder mesh — see [`GraphEngine3D::node_mesh`].
    pub fn edge_mesh(&self) -> &Arc<MeshLit> {
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

    /// Raw `PlatformEvent` handler — orbit-drag, wheel-dolly, shift-drag
    /// pan (plan §1.4), hover on `PointerMoved` and click-to-select on a
    /// low-movement `PointerDown`->`PointerUp` pair (Wave 3, plan §1.5).
    /// Wire this from the app's 3D dispatch (see
    /// `uzor-desktop::scene3d_app`'s divergence log for how
    /// `force_graph_demo` routes events to whichever dimension is
    /// active). Returns `true` if the event was consumed.
    pub fn on_event(&mut self, event: &PlatformEvent, viewport: Rect) -> bool {
        match event {
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => self.on_pointer_down(*x, *y, viewport),
            PlatformEvent::PointerMoved { x, y } => self.on_pointer_moved(*x, *y, viewport),
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => self.on_pointer_up(*x, *y, viewport),
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

    /// Plain left-drag orbits; shift-held left-drag pans instead (plan
    /// §1.4) — the drag KIND is resolved once here, at drag-start, and
    /// held for the whole gesture (mirrors the 2D engine's own
    /// box-select-mode resolution, `box_select_mode_for` in `engine.rs`).
    /// `false` (event not consumed) if `(x, y)` lands outside `viewport`.
    fn on_pointer_down(&mut self, x: f64, y: f64, viewport: Rect) -> bool {
        if !viewport.contains(x, y) {
            return false;
        }
        self.mode = if self.modifiers.shift {
            Pointer3DMode::Panning { last: (x, y) }
        } else {
            Pointer3DMode::Orbiting { last: (x, y), total: 0.0 }
        };
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
            Pointer3DMode::Panning { last } => {
                self.camera.pan((x - last.0) as f32, (y - last.1) as f32);
                self.mode = Pointer3DMode::Panning { last: (x, y) };
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
                self.hovered = self.pick_at(x, y, viewport);
            }
            handled = true;
        } else if self.hovered.is_some() {
            self.hovered = None;
            self.last_hover_pick_screen = None;
        }

        handled
    }

    /// Ends the in-progress drag/orbit/pan gesture; a plain orbit-drag
    /// that travelled less than [`CLICK_DRAG_THRESHOLD_PX`] since
    /// `PointerDown` resolves as a click — a Replace-select ray-pick at
    /// the release point (`None` on empty space deselects, mirroring
    /// 2D's own `clear_selection()` on a background click). Shift-drag
    /// pan never resolves a click (see [`Pointer3DMode::Panning`]'s doc
    /// comment).
    fn on_pointer_up(&mut self, x: f64, y: f64, viewport: Rect) -> bool {
        let mode = self.mode;
        self.mode = Pointer3DMode::Idle;
        match mode {
            Pointer3DMode::Orbiting { total, .. } => {
                if total < CLICK_DRAG_THRESHOLD_PX && viewport.contains(x, y) {
                    self.selected = self.pick_at(x, y, viewport);
                }
                true
            }
            Pointer3DMode::Panning { .. } => true,
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

    /// Instanced sphere nodes + cylinder edges (plan §1.3) — wires
    /// straight into [`crate::render3d::build_scene`], which is
    /// independently unit-tested (no GPU needed) for the node/edge
    /// instance construction itself; see `uzor-graph/tests/render3d_gpu.rs`
    /// for the headless-GPU proof that the result actually renders
    /// visually-distinct pixels.
    pub fn build_scene(&self) -> Scene3D {
        crate::render3d::build_scene(&self.graph, &self.particles, &self.node_mesh, &self.edge_mesh, crate::render3d::DEFAULT_EDGE_WIDTH)
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

        OverlayDrawStats { labels_drawn, hover_card_drawn }
    }
}

/// Per-frame draw counts for [`GraphEngine3D::draw_overlay`] — a
/// test/verification aid (mirrors [`crate::render::NodeDrawStats`]'s own
/// role for the 2D engine), not consumed by any production call site.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverlayDrawStats {
    pub labels_drawn: usize,
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
    fn build_scene_returns_one_instanced_sphere_per_node_and_one_cylinder_per_edge() {
        let mut engine: GraphEngine3D<(), (), ForceDirectedLayout3D> =
            GraphEngine3D::new(triangle(), ForceDirectedLayout3D::default());
        // `new()` seeds every particle at the origin — non-coincident
        // positions are needed here so none of the 3 edges gets skipped
        // as degenerate (see `render3d.rs`'s own
        // `build_edge_instances_skips_a_coincident_degenerate_edge`).
        engine.particles[0] = Particle::at3(-4.0, 0.0, 0.0);
        engine.particles[1] = Particle::at3(4.0, 0.0, 0.0);
        engine.particles[2] = Particle::at3(0.0, 4.0, 0.0);

        let scene = engine.build_scene();

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
    fn on_event_a_drag_past_the_click_threshold_orbits_but_does_not_select() {
        let mut engine = spread_triangle_engine();
        let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
        let aspect = (viewport.width / viewport.height) as f32;
        let camera = engine.camera(aspect);
        let (sx, sy) = pick3d::project_world_to_screen(&camera, Vec3::new(-40.0, 0.0, 0.0), viewport).expect("node 0 must project inside the view");

        engine.on_event(&PlatformEvent::PointerDown { x: sx, y: sy, button: uzor::input::MouseButton::Left }, viewport);
        engine.on_event(&PlatformEvent::PointerMoved { x: sx + 60.0, y: sy }, viewport);
        engine.on_event(&PlatformEvent::PointerUp { x: sx + 60.0, y: sy, button: uzor::input::MouseButton::Left }, viewport);

        assert_eq!(engine.selected(), None, "a drag past the click-drag threshold must orbit, not select");
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
}
