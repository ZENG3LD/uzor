//! `GraphEngine` — the facade tying graph + particles + camera + layout
//! + interaction + render + agent surface together into one owned
//! object an app registers as a blackbox and drives from `App::ui`/
//! `App::on_event`/`App::regions`.

use std::time::Instant;

use std::collections::{BTreeSet, HashSet};

use uzor::input::{KeyCode, ModifierKeys, MouseButton, PlatformEvent};
use uzor::render::{RenderContext, RenderRegion, UNCAPPED_FPS};
use uzor::types::Rect;
use uzor_figures::interact::FocusSet;

use crate::camera::{Aabb, Camera2D};
use crate::cluster::{ClusterRegistry, GroupId};
use crate::graph::{Graph, NodeIndex, SimEdge, SimTopology};
use crate::interaction::drag::DragController;
use crate::interaction::pick;
use crate::label_grid;
use crate::layout::force_directed::ForceDirectedLayout;
use crate::layout::{ForceParams, GraphLayoutMode, Layout, LayoutTickResult};
use crate::particle::Particle;
use crate::render as gr_render;
use crate::theme::GraphTheme;

/// Screen-space distance a `PointerDown`->`PointerUp` pair may travel
/// while panning the camera and still count as a click-to-deselect.
const CLICK_DRAG_THRESHOLD_PX: f64 = 4.0;
const ZOOM_SENSITIVITY: f64 = 0.0015;
/// Alpha to reheat the sim to when a node is unpinned, a cluster
/// expands, or force params change — enough to visibly resettle the
/// local neighborhood without a full restart-from-scratch jolt. NOT used
/// for drag-start any more — see [`DRAG_ALPHA_TARGET`] for the sustained
/// (not one-shot) reheat an active drag holds.
const DRAG_REHEAT_ALPHA: f32 = 0.35;

/// Sustained "`alphaTarget`"-equivalent held for the whole duration of an
/// active node drag (Wave 2.1 drag-physics contract — d3-force canon
/// range 0.1-0.3, obsidian doc §drag/d3-canon; picked the top of that
/// range so the local neighborhood keeps visibly simmering for the
/// entire gesture, not just at drag-start). Set via
/// [`Layout::set_alpha_target`] on drag-start, cleared back to `0.0` on
/// drag-end so alpha eases back down instead of free-decaying from
/// wherever it happened to be — the one-shot [`DRAG_REHEAT_ALPHA`] bump
/// this replaces for the drag path didn't hold a target, it just bumped
/// once and let ordinary decay take back over immediately.
const DRAG_ALPHA_TARGET: f32 = 0.3;

/// Screen-space distance the pointer must travel since the last hover
/// pick before `nearest_node` is re-run on `PointerMoved` (Wave 2.2 perf
/// guard — cosmos.gl/sigma's "skip the readback if the mouse hasn't
/// moved" idiom, adapted for CPU distance-scan picking: at 534+ nodes a
/// re-pick on every single-pixel jitter is wasted work the render loop
/// doesn't need). Below this threshold the previous hover result is kept
/// as-is.
const HOVER_PICK_MIN_MOVE_PX: f64 = 2.0;

/// Default hover-neighbor-highlight depth (Wave 2.2 — oss doc §2.1: "no
/// surveyed engine ships depth-2 as a built-in option... depth is a free
/// parameter"). `0` highlights only the hovered node itself; `1` (this
/// default) adds its direct neighbors + connecting edges.
const DEFAULT_HOVER_DEPTH: u8 = 1;

/// Default node-label halo color (owner defect report: thin edge strokes
/// crossing node label text made it unreadable, e.g. in the hierarchical/
/// radial exhibits) — matches `force-graph-demo`'s/the showcase's own
/// `GraphExhibit` canvas background (`"#0d0f14"`), so the default reads as
/// invisible on the crate's own demo canvas while still protecting a
/// label wherever a caller's own background happens to differ. See
/// [`GraphEngine::label_halo`]/[`GraphEngine::set_label_halo`].
pub(crate) const DEFAULT_LABEL_HALO: &str = "#0d0f14";

/// Wave 2.5 default transition duration (ms) for `zoom_to_fit`/
/// `zoom_to_node` when the caller doesn't specify one. `pub(crate)` so
/// `agent.rs`'s `zoom_to_fit`/`zoom_to_node` actions default to the exact
/// same value the engine API itself would use for an omitted argument.
pub(crate) const DEFAULT_TRANSITION_MS: f64 = 400.0;
/// Wave 2.5 default screen-space padding (px) for `zoom_to_fit` —
/// `pub(crate)` for the same reason as [`DEFAULT_TRANSITION_MS`].
pub(crate) const DEFAULT_FIT_PADDING_PX: f64 = 40.0;
/// Wave 2.6 default local-subgraph BFS depth ("Depth default 2" — the
/// task's own spec).
const DEFAULT_LOCAL_DEPTH: u8 = 2;
/// Wave 2.5 keyboard-nav pan speed, screen px/sec — scaled by the real
/// per-tick `dt` (framerate-independent, vis-network's own "tie the
/// repeat rate to the render loop" idiom, oss doc §2.15) rather than a
/// fixed px-per-tick amount.
const KEY_PAN_SPEED_PX_PER_S: f64 = 480.0;
/// Wave 2.5 keyboard-nav zoom rate, multiplicative fraction per second
/// (e.g. holding zoom-in for 1s multiplies zoom by roughly `1.0 +
/// KEY_ZOOM_RATE_PER_S`).
const KEY_ZOOM_RATE_PER_S: f64 = 1.2;
/// `on_scroll`'s per-event zoom-factor clamp — bounds how much a single
/// wheel tick can zoom in/out regardless of `dy`'s own magnitude.
const SCROLL_ZOOM_FACTOR_MIN: f64 = 0.8;
const SCROLL_ZOOM_FACTOR_MAX: f64 = 1.25;

/// Every keyboard/scroll/drag "feel" constant this engine owns, bundled
/// into one caller-configurable struct (graph-strengthening arc Wave G2 —
/// the 2D audit's own configurability inventory flagged every one of
/// these `✗`, no override anywhere). [`Default`] reproduces the
/// pre-existing module constants byte-identically.
///
/// One `interaction: GraphInteractionConfig` field on [`GraphEngine`] with
/// an `interaction_config()`/`set_interaction_config()` accessor pair —
/// the same established shape [`GraphEngine::label_halo`]/
/// `set_label_halo` already use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphInteractionConfig {
    /// Screen-space distance a `PointerDown`->`PointerUp` pair may travel
    /// while panning the camera and still count as a click-to-deselect —
    /// was [`CLICK_DRAG_THRESHOLD_PX`].
    pub click_drag_threshold_px: f64,
    /// Scroll-wheel zoom speed multiplier — was [`ZOOM_SENSITIVITY`].
    pub zoom_sensitivity: f64,
    /// `on_scroll`'s per-event zoom-factor clamp bounds — were the inline
    /// literal `.clamp(0.8, 1.25)`.
    pub scroll_zoom_factor_min: f64,
    pub scroll_zoom_factor_max: f64,
    /// Keyboard-nav pan speed, screen px/sec — was [`KEY_PAN_SPEED_PX_PER_S`].
    pub key_pan_speed_px_per_s: f64,
    /// Keyboard-nav zoom rate, multiplicative fraction/sec — was
    /// [`KEY_ZOOM_RATE_PER_S`].
    pub key_zoom_rate_per_s: f64,
    /// One-shot reheat alpha on unpin/filter-change — was
    /// [`DRAG_REHEAT_ALPHA`].
    pub drag_reheat_alpha: f32,
    /// Sustained alpha target held for the whole duration of an active
    /// node drag — was [`DRAG_ALPHA_TARGET`].
    pub drag_alpha_target: f32,
    /// Screen-space distance the pointer must travel since the last hover
    /// pick before re-picking — was [`HOVER_PICK_MIN_MOVE_PX`].
    pub hover_pick_min_move_px: f64,
}

impl Default for GraphInteractionConfig {
    fn default() -> Self {
        Self {
            click_drag_threshold_px: CLICK_DRAG_THRESHOLD_PX,
            zoom_sensitivity: ZOOM_SENSITIVITY,
            scroll_zoom_factor_min: SCROLL_ZOOM_FACTOR_MIN,
            scroll_zoom_factor_max: SCROLL_ZOOM_FACTOR_MAX,
            key_pan_speed_px_per_s: KEY_PAN_SPEED_PX_PER_S,
            key_zoom_rate_per_s: KEY_ZOOM_RATE_PER_S,
            drag_reheat_alpha: DRAG_REHEAT_ALPHA,
            drag_alpha_target: DRAG_ALPHA_TARGET,
            hover_pick_min_move_px: HOVER_PICK_MIN_MOVE_PX,
        }
    }
}

/// Default viewport-cull world-space margin — was `render::cull_visible`'s
/// own local `const MARGIN: f64 = 64.0`. See
/// [`GraphEngine::cull_margin_world`]/[`GraphEngine::set_cull_margin_world`].
pub(crate) const DEFAULT_CULL_MARGIN_WORLD: f64 = 64.0;

/// Wave 2.5 keyboard nav — logical pan/zoom directions, decoupled from
/// the specific physical [`KeyCode`] that triggers them (several
/// physical keys can map to the same logical zoom direction, see
/// [`nav_key_for`] — vis-network's own "multiple physical keys per
/// logical action" robustness convention, oss doc §2.15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NavKey {
    PanUp,
    PanDown,
    PanLeft,
    PanRight,
    ZoomIn,
    ZoomOut,
}

/// Physical key -> logical nav direction. Arrows pan; `+`/`-` zoom, with
/// `BracketRight`/`PageUp` and `BracketLeft`/`PageDown` as extra physical
/// aliases for zoom-in/zoom-out respectively (vis-network binds 4
/// physical keys per logical zoom action for keyboard-layout robustness —
/// this crate's `KeyCode` enum only has 3 candidates per direction, so
/// that's what's bound). Any other key is `None` (not a nav key at all).
fn nav_key_for(key: KeyCode) -> Option<NavKey> {
    match key {
        KeyCode::ArrowUp => Some(NavKey::PanUp),
        KeyCode::ArrowDown => Some(NavKey::PanDown),
        KeyCode::ArrowLeft => Some(NavKey::PanLeft),
        KeyCode::ArrowRight => Some(NavKey::PanRight),
        KeyCode::Plus | KeyCode::BracketRight | KeyCode::PageUp => Some(NavKey::ZoomIn),
        KeyCode::Minus | KeyCode::BracketLeft | KeyCode::PageDown => Some(NavKey::ZoomOut),
        _ => None,
    }
}

/// Ease-in-out cubic, monotonically increasing on `[0, 1]` (`t=0 -> 0`,
/// `t=1 -> 1`) — sigma.js `Camera.animate`'s own convention (oss doc
/// §2.6): only the PROGRESS FRACTION is eased; pan and zoom are then each
/// linearly interpolated in transform-space against that same eased
/// fraction (see [`CameraTransition::step`]), not a curved "fly-to" path
/// through zoom-space (d3's optional `interpolateZoom`/Van Wijk-Nuij
/// convention is a heavier alternative this crate doesn't need at demo
/// scale — the research doc explicitly allows "simple linear-in-
/// transform-space" here).
///
/// `pub(crate)` (not private) so [`crate::engine3d::GraphEngine3D`]'s own
/// dimension-transition wave (the animated 2D<->3D switch) reuses this
/// EXACT easing function rather than a duplicated copy — same "reuse the
/// SAME logic, not a parallel one" convention `box_select_mode_for`/
/// `normalized_rect`/`FilterSpec::matches` already established for the 3D
/// waves.
pub(crate) fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// An in-flight animated camera move (Wave 2.5 — `GraphEngine::
/// zoom_to_fit`/`zoom_to_node`), ticked once per [`GraphEngine::tick`]
/// call via [`GraphEngine::advance_camera_transition`]. d3-zoom's own
/// "translate-then-scale" ordering (oss doc §2.6) is honored
/// structurally by `Camera2D` itself (`world_to_screen` already applies
/// pan THEN scale in that exact order) — this transition interpolates
/// pan and zoom TOGETHER, each field linearly against the SAME eased
/// progress fraction, so the target node/AABB stays visually anchored at
/// every intermediate frame rather than the pan and zoom drifting out of
/// sync with each other.
#[derive(Debug, Clone, Copy)]
struct CameraTransition {
    start_pan: (f64, f64),
    start_zoom: f64,
    target_pan: (f64, f64),
    target_zoom: f64,
    elapsed_s: f32,
    duration_s: f32,
}

impl CameraTransition {
    fn new(camera: Camera2D, target_pan: (f64, f64), target_zoom: f64, duration_ms: f64) -> Self {
        Self {
            start_pan: (camera.pan_x, camera.pan_y),
            start_zoom: camera.zoom,
            target_pan,
            target_zoom: target_zoom.clamp(crate::camera::ZOOM_MIN, crate::camera::ZOOM_MAX),
            elapsed_s: 0.0,
            duration_s: (duration_ms.max(0.0) / 1000.0) as f32,
        }
    }

    /// Advance by `dt` seconds. Returns the eased `(pan, zoom)` for this
    /// frame and whether the transition just reached its target — sigma's
    /// own "on `t>=1` snap to the exact final state" (the caller assigns
    /// the returned values straight to the camera every step, including
    /// the final one, so there's no separate snap needed: at `t==1` the
    /// eased lerp already lands EXACTLY on `target_pan`/`target_zoom`).
    /// `duration_s <= 0.0` (an explicit instant snap) short-circuits `t`
    /// to `1.0` on the very first call.
    fn step(&mut self, dt: f32) -> ((f64, f64), f64, bool) {
        self.elapsed_s += dt.max(0.0);
        let t = if self.duration_s <= 0.0 { 1.0 } else { (self.elapsed_s / self.duration_s).clamp(0.0, 1.0) as f64 };
        let eased = ease_in_out_cubic(t);
        let pan = (
            self.start_pan.0 + (self.target_pan.0 - self.start_pan.0) * eased,
            self.start_pan.1 + (self.target_pan.1 - self.start_pan.1) * eased,
        );
        let zoom = self.start_zoom + (self.target_zoom - self.start_zoom) * eased;
        (pan, zoom, t >= 1.0)
    }
}

/// Wave 2.6 query filter (obsidian doc §12 — DELIBERATELY decoupled from
/// color groups, which this engine doesn't even have yet: this is the
/// "filter" half of Obsidian's own Filters-vs-Groups split, ready for a
/// future color-group feature to sit alongside it on the same query
/// grammar without the two ever being conflated). Typed AND-semantics
/// predicate — no query-language strings inside the engine; a caller/
/// agent boundary is free to compile one of these from a string DSL, but
/// that translation lives OUTSIDE this crate (`agent.rs`'s JSON args are
/// already exactly that kind of boundary, and take the fields directly,
/// no string grammar). A `None` field means "don't filter on this axis";
/// every `Some` field must pass for a node to remain visible.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterSpec {
    pub label_substring: Option<String>,
    pub categories: Option<Vec<String>>,
    pub min_degree: Option<u32>,
}

impl FilterSpec {
    /// Whether `id` passes every `Some` clause. Label matching is
    /// case-insensitive substring (the common "search box" convention);
    /// category matching is exact-string equality against any entry in
    /// the list. `false` for an out-of-range `id` (fails closed, not
    /// open).
    ///
    /// `pub(crate)` (not private) so [`crate::engine3d::GraphEngine3D`]'s
    /// own filter/local-subgraph wave reuses this EXACT matching logic
    /// (both the `tick` force-topology filtering and the exclusion-set
    /// computation) rather than a duplicated copy — same "reuse the
    /// SAME typed struct, not a parallel one" convention
    /// `box_select_mode_for`/`normalized_rect` already established for
    /// the 3D selection wave.
    pub(crate) fn matches<N, E>(&self, graph: &Graph<N, E>, id: NodeIndex) -> bool {
        let Some(node) = graph.get_node(id) else { return false };
        if let Some(sub) = &self.label_substring {
            if !node.label.to_lowercase().contains(&sub.to_lowercase()) {
                return false;
            }
        }
        if let Some(categories) = &self.categories {
            if !categories.iter().any(|c| c == &node.category) {
                return false;
            }
        }
        if let Some(min_degree) = self.min_degree {
            if graph.degree(id) < min_degree {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
enum PointerMode {
    Idle,
    PanningCamera { last: (f64, f64), total: f64 },
    DraggingNode,
    /// Wave 2.4 box-select drag — `origin` is the fixed down-point,
    /// `current` tracks the live cursor (updated every `PointerMoved`,
    /// what [`GraphEngine::box_select_rect`] reads for the live rubber-
    /// band overlay); `mode` was resolved ONCE at drag-start from the
    /// held modifiers (see [`box_select_mode_for`]) and never changes for
    /// the rest of the gesture, even if the user releases/re-holds a
    /// modifier mid-drag (matches every surveyed engine's own convention
    /// — the activating chord is read at mousedown, not mouseup).
    BoxSelecting { origin: (f64, f64), current: (f64, f64), mode: SelectMode },
}

/// How [`GraphEngine::apply_selection`]/[`GraphEngine::box_select`]
/// combine newly-hit nodes with the EXISTING [`GraphEngine::selection`]
/// (AntV G6's box-select mode model, oss doc §2.3 — narrowed to the 3
/// modes this engine's own modifier scheme reaches, see
/// [`box_select_mode_for`]'s doc comment for the full mapping; G6's 4th
/// mode, `intersect`, has no modifier chord assigned and is deliberately
/// not ported — nothing in the task's own gate list exercises it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    /// Selection becomes EXACTLY the new node set (everything outside it
    /// is deselected) — G6's "default" mode.
    Replace,
    /// New nodes are ADDED to the existing selection; anything already
    /// selected stays selected.
    Union,
    /// Each new node TOGGLES: removed if already selected, added
    /// otherwise (G6's own "invert" framing).
    Diff,
}

/// Wave 2.4 modifier -> box-select-mode mapping. Shift is the box-select
/// ACTIVATION key and, held alone, ALWAYS wins over node-grab dispatch at
/// `on_pointer_down` — even when the down-point lands directly on a node
/// (the oss doc's own cytoscape-issue-1583 note: "modifier-held mousedown
/// should win over node-grab, regardless of node type"). An additional
/// modifier on top of Shift selects which of the 3 combine-modes applies:
/// - **Shift alone** -> [`SelectMode::Replace`] — G6 names its own
///   no-extra-decoration case "default"; Shift here plays only the
///   activation role (distinguishing box-select from camera pan, which
///   owns plain drag), so the undecorated chord maps to the undecorated
///   mode.
/// - **Shift+Ctrl** -> [`SelectMode::Union`] (Ctrl = "additive" — the
///   same role cytoscape/vis-network already give Ctrl for their own
///   additive gestures).
/// - **Shift+Alt** -> [`SelectMode::Diff`].
///
/// Divergence note: the wave-2 spec's own illustrative text paired bare
/// "Shift+drag" with Union, which is unreachable as written — SOME
/// modifier beyond plain drag is mandatory just to enter box-select mode
/// at all (plain drag is already camera pan), so "no extra modifier"
/// (the literal wording for Replace) and "Shift held" (the literal
/// wording for Union) can't both mean "Shift alone." This mapping keeps
/// Shift-alone for the undecorated (Replace) mode and reserves the two
/// two-key chords for Union/Diff — see `uzor-graph/CLAUDE.md`'s
/// divergence log for the full reasoning.
///
/// `pub(crate)` (not private) so [`crate::engine3d::GraphEngine3D`]'s own
/// box-select entry point reuses this EXACT modifier mapping rather than
/// a duplicated copy — the 3D box-select wave.
pub(crate) fn box_select_mode_for(modifiers: ModifierKeys) -> Option<SelectMode> {
    if !modifiers.shift {
        return None;
    }
    Some(if modifiers.ctrl {
        SelectMode::Union
    } else if modifiers.alt {
        SelectMode::Diff
    } else {
        SelectMode::Replace
    })
}

/// Corner-normalize two arbitrary screen points into a non-negative-size
/// `Rect` — shared by [`GraphEngine::box_select_rect`] (live overlay) and
/// [`GraphEngine::box_select`] (final containment test), so the drawn
/// rectangle and the actually-tested area can never diverge. `pub(crate)`
/// so [`crate::engine3d::GraphEngine3D`]'s own box-select reuses this
/// exact helper (3D box-select wave) instead of a duplicated copy.
pub(crate) fn normalized_rect(a: (f64, f64), b: (f64, f64)) -> Rect {
    let x = a.0.min(b.0);
    let y = a.1.min(b.1);
    Rect::new(x, y, (a.0 - b.0).abs(), (a.1 - b.1).abs())
}

/// One member of an in-progress drag gesture (Wave 2.4 group drag),
/// captured at drag-start. `offset_from_anchor` is a FIXED world-space
/// offset from the grabbed (anchor) node's own position at grab time —
/// every subsequent tick (including the very first, at mousedown itself)
/// recomputes this member's `fx`/`fy` as `anchor_world_now +
/// offset_from_anchor` (see `GraphEngine::apply_drag_shift`). This is the
/// single-shared-shift convention cytoscape's own `silentShift` applies
/// to a whole collection at once (oss doc §2.4) — relative offsets
/// between drag-set members are preserved EXACTLY by construction (never
/// incrementally accumulated from a stale per-tick delta, which is where
/// per-node drift would creep in). For a solo drag (drag set = `{anchor}`
/// only) `offset_from_anchor` is `(0, 0)`, which reproduces the engine's
/// pre-Wave-2.4 single-node behavior byte-for-byte: the anchor still
/// snaps to be centered exactly under the cursor at grab, regardless of
/// where inside the hit-tolerance radius the down-point landed.
#[derive(Debug, Clone, Copy)]
struct DragMember {
    node: NodeIndex,
    offset_from_anchor: (f32, f32),
    /// Snapshot of [`GraphEngine::is_pinned`] for this member at
    /// drag-start — what [`DragEndPolicy::RestorePrior`] restores on
    /// release, now per-member instead of the old single-node bool (Wave
    /// 2.1's `drag_prior_pinned`, superseded here since a group drag can
    /// release many nodes at once, each with its own prior pin state).
    prior_pinned: bool,
}

/// What happens to a node's pin state when a drag ends. Wave 2.1 owner
/// order (2026-07-18): the DEFAULT is [`DragEndPolicy::Sticky`] ("хочу
/// вывести и оставить" — drag a node out, release, it stays exactly
/// there); [`DragEndPolicy::RestorePrior`] is the classic d3-force
/// convention (obsidian doc §drag/d3-canon) kept available as the
/// explicit non-default mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragEndPolicy {
    /// Drag-end always pins the node at the release position — an
    /// explicit unpin ([`GraphEngine::unpin_node`] / the `unpin_node`
    /// agent action) is required to rejoin the simulation.
    #[default]
    Sticky,
    /// Drag-end unfixes the node UNLESS it was already explicitly pinned
    /// (via [`GraphEngine::pin_node`]) before the drag started — pin
    /// composes with drag (a pre-pinned node stays pinned, now at the
    /// drag's release position) instead of every drag producing a pin.
    RestorePrior,
}

/// Generic facts about one node, for a caller's sidebar/inspector —
/// engine-level, so no domain-specific field beyond what `Graph` itself
/// exposes (id/label/category/degree/position/pin state).
#[derive(Debug, Clone, Copy)]
pub struct NodeFacts<'a> {
    pub index: NodeIndex,
    pub label: &'a str,
    pub category: &'a str,
    pub degree: u32,
    pub position: (f32, f32),
    pub pinned: bool,
}

/// The engine's owned state: graph topology, simulated positions,
/// camera, the active layout algorithm, and interaction/selection
/// state. Implements [`uzor::layout::agent::BlackboxAgentSurface`] (see
/// `agent.rs`) so it can be registered directly as a blackbox — the
/// same `Arc<Mutex<...>>` a human-driven `App::ui`/`on_event` and the
/// HTTP agent control plane both mutate.
pub struct GraphEngine<N, E, L: Layout = ForceDirectedLayout> {
    pub graph: Graph<N, E>,
    pub particles: Vec<Particle>,
    pub camera: Camera2D,
    pub layout: L,
    /// The last node an actual CLICK (or a solo drag-then-release — see
    /// `on_pointer_up`'s `DraggingNode` arm) selected — the facts-panel
    /// value (Wave 2.4: this stays a plain `Option`, superseded for
    /// FOCUS/RING purposes by [`GraphEngine::selection`] below, but kept
    /// as-is for "what should the sidebar show" — see that field's own
    /// doc comment for exactly which gestures update it).
    pub selected: Option<NodeIndex>,
    pub hovered: Option<NodeIndex>,
    /// The current multi-selection (Wave 2.4), iterated in deterministic
    /// ascending `NodeIndex` order. Superset of the single-selection
    /// concept `selected` used to carry alone: a click/solo-drag-release
    /// still ends up here as a one-element set (`select` sets BOTH
    /// `selected` and `selection`), but box-select
    /// ([`GraphEngine::box_select`]/[`GraphEngine::apply_selection`]) and
    /// the `select_nodes`/`box_select` agent actions mutate ONLY this
    /// field, deliberately leaving `selected` (the "last individually
    /// clicked" value) untouched — see `apply_selection`'s doc comment.
    /// Render (`draw_nodes`'s selection ring) and the hover/selection
    /// dim-highlight reducer (`refresh_focus`) both derive from this SET,
    /// not from `selected`.
    pub selection: BTreeSet<NodeIndex>,
    pub focus: FocusSet,
    pub clusters: ClusterRegistry,

    pinned: Vec<bool>,
    drag: DragController,
    /// Every node moved by the in-progress drag gesture, captured at
    /// drag-start (Wave 2.4 group drag) — empty when nothing is being
    /// dragged. See [`DragMember`].
    drag_group: Vec<DragMember>,
    /// Whether the grabbed (anchor) node was ALREADY in `selection` at
    /// drag-start — decides `on_pointer_up`'s DraggingNode arm: `true`
    /// means the whole multi-selection just moved together and stays
    /// selected as-is; `false` means a single un-selected node was
    /// dragged solo and Replace-selects itself on release (see
    /// `on_pointer_down`'s doc comment).
    drag_was_group: bool,
    drag_end_policy: DragEndPolicy,
    /// Current keyboard-modifier state (Wave 2.4) — updated from
    /// `PlatformEvent::ModifiersChanged`, read by `on_pointer_down` to
    /// decide box-select vs. camera-pan/node-drag (see
    /// [`box_select_mode_for`]).
    modifiers: ModifierKeys,
    mode: PointerMode,
    canvas_rect: Rect,
    last_pointer_screen: (f64, f64),
    /// Screen position at the last `nearest_node` hover pick — `None`
    /// once the pointer has left the canvas (so re-entering always picks
    /// again immediately, regardless of where it left off). See
    /// [`HOVER_PICK_MIN_MOVE_PX`].
    last_hover_pick_screen: Option<(f64, f64)>,
    hover_depth: u8,
    hover_card: bool,
    /// Label-LOD density param (Wave 2.3 — `crate::label_grid`'s
    /// `labelDensity`, "labels per 100px cell at zoom 1.0"). See
    /// [`GraphEngine::label_density`]/[`GraphEngine::set_label_density`].
    label_density: f64,
    /// Node-label halo color — see [`DEFAULT_LABEL_HALO`]'s own doc
    /// comment. See [`GraphEngine::label_halo`]/[`GraphEngine::
    /// set_label_halo`].
    label_halo: String,
    /// Labels actually drawn on the last [`GraphEngine::draw`] call —
    /// see [`GraphEngine::labels_drawn_last_frame`].
    labels_drawn_last_frame: usize,
    visible: Vec<NodeIndex>,
    last_tick: LayoutTickResult,
    last_frame_at: Option<Instant>,
    dirty: bool,
    /// Wave 2.5 — in-flight animated `zoom_to_fit`/`zoom_to_node` move,
    /// ticked from [`GraphEngine::tick`]. `None` when idle.
    camera_transition: Option<CameraTransition>,
    /// Wave 2.5 keyboard-nav hold-to-repeat state (vis-network pattern) —
    /// every currently-held nav key gets its per-frame pan/zoom delta
    /// applied every [`GraphEngine::tick`] for as long as it stays in
    /// this set (inserted on `KeyDown`, removed on `KeyUp`).
    held_nav_keys: HashSet<NavKey>,
    /// Wave 2.6 local subgraph root + BFS depth — `None` shows the full
    /// graph. See [`GraphEngine::set_local_root`].
    local_root: Option<(NodeIndex, u8)>,
    /// Wave 2.6 query filter — `None` shows every node. See
    /// [`GraphEngine::set_filter`].
    filter: Option<FilterSpec>,
    /// Every paint color/font `crate::render`'s draw functions read (graph-
    /// strengthening arc Wave G2). See [`GraphEngine::theme`]/
    /// [`GraphEngine::set_theme`].
    theme: GraphTheme,
    /// Label-LOD tuning (grid cell size, zoom fade window, degree shift —
    /// Wave G2). See [`GraphEngine::label_lod`]/[`GraphEngine::set_label_lod`].
    label_lod: label_grid::LabelLodConfig,
    /// Keyboard/scroll/drag "feel" constants (Wave G2). See
    /// [`GraphEngine::interaction_config`]/[`GraphEngine::set_interaction_config`].
    interaction: GraphInteractionConfig,
    /// Viewport-cull world-space margin (Wave G2 — was `render::
    /// cull_visible`'s own fixed `MARGIN` constant). See
    /// [`GraphEngine::cull_margin_world`]/[`GraphEngine::set_cull_margin_world`].
    cull_margin_world: f64,
    pub(crate) agent_slot_id: String,
}

impl<N, E, L: Layout> GraphEngine<N, E, L> {
    pub fn new(graph: Graph<N, E>, layout: L) -> Self {
        let n = graph.node_count();
        Self {
            graph,
            particles: vec![Particle::default(); n],
            camera: Camera2D::default(),
            layout,
            selected: None,
            hovered: None,
            selection: BTreeSet::new(),
            focus: FocusSet::empty(),
            clusters: ClusterRegistry::default(),
            pinned: vec![false; n],
            drag: DragController::default(),
            drag_group: Vec::new(),
            drag_was_group: false,
            drag_end_policy: DragEndPolicy::default(),
            modifiers: ModifierKeys::default(),
            mode: PointerMode::Idle,
            canvas_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            last_pointer_screen: (0.0, 0.0),
            last_hover_pick_screen: None,
            hover_depth: DEFAULT_HOVER_DEPTH,
            hover_card: true,
            label_density: label_grid::DEFAULT_LABEL_DENSITY,
            label_halo: DEFAULT_LABEL_HALO.to_owned(),
            labels_drawn_last_frame: 0,
            visible: Vec::new(),
            last_tick: LayoutTickResult { alpha: 1.0, max_displacement: 0.0, settled: false },
            last_frame_at: None,
            dirty: true,
            camera_transition: None,
            held_nav_keys: HashSet::new(),
            local_root: None,
            filter: None,
            theme: GraphTheme::dark(),
            label_lod: label_grid::LabelLodConfig::default(),
            interaction: GraphInteractionConfig::default(),
            cull_margin_world: DEFAULT_CULL_MARGIN_WORLD,
            agent_slot_id: "graph".to_owned(),
        }
    }

    pub fn set_agent_slot_id(&mut self, id: impl Into<String>) {
        self.agent_slot_id = id.into();
    }

    /// Seed initial particle positions in `NodeIndex` order. Entries
    /// past `positions.len()` stay at the origin (which the sim will
    /// spread out on its own — just slower to settle).
    pub fn seed_positions(&mut self, positions: &[(f32, f32)]) {
        for (p, &(x, y)) in self.particles.iter_mut().zip(positions.iter()) {
            p.x = x;
            p.y = y;
        }
        self.dirty = true;
    }

    pub fn set_canvas_rect(&mut self, rect: Rect) {
        self.canvas_rect = rect;
    }

    pub fn canvas_rect(&self) -> Rect {
        self.canvas_rect
    }

    /// Whether the render loop should keep redrawing every frame — the
    /// unsettled-physics case ([`GraphEngine::last_tick`]), PLUS (Wave
    /// 2.5) an in-flight animated camera transition or a held keyboard-nav
    /// key: both need continuous ticking to actually animate/repeat even
    /// while the sim itself is fully settled.
    pub fn is_hot(&self) -> bool {
        !self.last_tick.settled || self.camera_transition.is_some() || !self.held_nav_keys.is_empty()
    }

    pub fn last_tick(&self) -> LayoutTickResult {
        self.last_tick
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// The `RenderRegion` this engine wants for its canvas: continuous
    /// while hot, dirty-driven (repaint only when [`GraphEngine::dirty`]
    /// is set) once settled — the freeze/wake split from the engine
    /// design doc §3.5.
    pub fn render_region(&self, id: &'static str) -> RenderRegion {
        if self.is_hot() {
            RenderRegion { id, rect: self.canvas_rect, target_fps: UNCAPPED_FPS, dirty: true }
        } else {
            RenderRegion { id, rect: self.canvas_rect, target_fps: 0, dirty: self.dirty }
        }
    }

    /// Advance the simulation by `dt` real seconds — also advances any
    /// in-flight camera transition (Wave 2.5) and applies held keyboard-
    /// nav keys (Wave 2.5), then ticks the layout against either the raw
    /// topology or, when [`GraphEngine::filter`] is active (Wave 2.6), a
    /// FILTERED one: an edge is dropped the instant EITHER endpoint fails
    /// the filter, so the link-force no longer pulls surviving nodes
    /// toward an excluded one (`GraphEngine::set_filter`'s own doc
    /// comment covers the full "removed from the sim" contract).
    pub fn tick(&mut self, dt: f32) -> LayoutTickResult {
        let was_hot = self.is_hot();

        self.apply_held_nav_keys(dt);
        self.advance_camera_transition(dt);

        let topo = self.graph.topology();
        self.last_tick = match &self.filter {
            Some(filter) => {
                let filtered_edges: Vec<SimEdge> = topo
                    .edges
                    .iter()
                    .copied()
                    .filter(|e| filter.matches(&self.graph, e.from) && filter.matches(&self.graph, e.to))
                    .collect();
                let filtered_topo =
                    SimTopology { node_count: topo.node_count, edges: &filtered_edges, degree: topo.degree, radii: topo.radii };
                self.layout.tick(&filtered_topo, &mut self.particles, dt)
            }
            None => self.layout.tick(&topo, &mut self.particles, dt),
        };

        if was_hot || self.is_hot() {
            self.dirty = true;
        }
        self.last_tick
    }

    /// [`GraphEngine::tick`] using wall-clock elapsed time since the
    /// last call (clamped so a stalled frame can't blow the sim up).
    pub fn tick_real_time(&mut self) -> LayoutTickResult {
        let now = Instant::now();
        let dt = match self.last_frame_at {
            Some(prev) => now.duration_since(prev).as_secs_f32().min(0.1),
            None => 1.0 / 60.0,
        };
        self.last_frame_at = Some(now);
        self.tick(dt)
    }

    pub fn reheat(&mut self, alpha: f32) {
        self.layout.reheat(alpha);
        self.dirty = true;
    }

    /// Mark the canvas dirty without touching the sim's cooling state —
    /// e.g. after an agent-driven camera move, which changes what's on
    /// screen but shouldn't wake the physics.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Apply the per-frame pan/zoom delta for every currently-held
    /// keyboard-nav key (Wave 2.5 — vis-network's hold-to-repeat pattern,
    /// oss doc §2.15, re-expressed in Rust as "loop over the held-key set
    /// every tick" instead of JS's "register/unregister a per-frame
    /// closure on the render loop" — same net effect, tied to the actual
    /// render loop's own `dt` either way, so it auto-adapts to frame rate
    /// with no separate timer). Direction convention: an arrow key pans
    /// exactly like a simulated background drag toward that same screen
    /// direction (`ArrowRight` == `pan_x += step`, matching a real
    /// rightward drag's sign — see `on_pointer_moved`'s `PanningCamera`
    /// arm). Zoom keys zoom around the CANVAS CENTER (keyboard input has
    /// no cursor position of its own to anchor on, unlike wheel-zoom).
    fn apply_held_nav_keys(&mut self, dt: f32) {
        if self.held_nav_keys.is_empty() {
            return;
        }
        let dt = dt.max(0.0) as f64;
        let pan_step = self.interaction.key_pan_speed_px_per_s * dt;
        let key_zoom_rate = self.interaction.key_zoom_rate_per_s;
        let mut dx = 0.0;
        let mut dy = 0.0;
        let mut zoom_factor = 1.0;
        for nav in &self.held_nav_keys {
            match nav {
                NavKey::PanUp => dy -= pan_step,
                NavKey::PanDown => dy += pan_step,
                NavKey::PanLeft => dx -= pan_step,
                NavKey::PanRight => dx += pan_step,
                NavKey::ZoomIn => zoom_factor *= 1.0 + key_zoom_rate * dt,
                NavKey::ZoomOut => zoom_factor /= 1.0 + key_zoom_rate * dt,
            }
        }
        self.camera.pan_x += dx;
        self.camera.pan_y += dy;
        if (zoom_factor - 1.0).abs() > f64::EPSILON {
            let center = (self.canvas_rect.center_x(), self.canvas_rect.center_y());
            self.camera.zoom_at(center, self.canvas_rect, zoom_factor);
        }
        self.dirty = true;
    }

    /// Step the in-flight [`CameraTransition`], if any, and clear it once
    /// it reaches its target (Wave 2.5).
    fn advance_camera_transition(&mut self, dt: f32) {
        let Some(transition) = self.camera_transition.as_mut() else { return };
        let (pan, zoom, finished) = transition.step(dt);
        self.camera.pan_x = pan.0;
        self.camera.pan_y = pan.1;
        self.camera.zoom = zoom.clamp(crate::camera::ZOOM_MIN, crate::camera::ZOOM_MAX);
        self.dirty = true;
        if finished {
            self.camera_transition = None;
        }
    }

    /// Nodes hidden this frame — cluster-collapsed members (existing
    /// behavior) UNIONED with Wave 2.6's two new exclusion sources: the
    /// local-subgraph restriction ([`GraphEngine::local_root`]) and the
    /// query filter ([`GraphEngine::filter`]). Single source of truth for
    /// [`GraphEngine::refresh_visible`] (render/pick/label eligibility —
    /// exclusion is a hard GONE, not a dim, per the task's own spec) AND
    /// [`GraphEngine::draw`]'s `DrawContext::hidden` (drops edges touching
    /// an excluded node) — the SAME set feeds both, so a node can never
    /// be invisible yet still edge-connected on screen, or vice versa.
    fn compute_excluded_nodes(&self) -> HashSet<NodeIndex> {
        let mut excluded: HashSet<NodeIndex> =
            if self.clusters.any_collapsed() { self.clusters.hidden_nodes().collect() } else { HashSet::new() };

        if let Some((root, depth)) = self.local_root {
            let local_set = self.local_bfs_nodes(root, depth);
            for (id, _) in self.graph.nodes() {
                if !local_set.contains(&id) {
                    excluded.insert(id);
                }
            }
        }

        if let Some(filter) = &self.filter {
            for (id, _) in self.graph.nodes() {
                if !filter.matches(&self.graph, id) {
                    excluded.insert(id);
                }
            }
        }

        excluded
    }

    /// BFS depth-`depth` node set from `root` (Wave 2.6 local subgraph) —
    /// reuses [`Graph::neighborhood_focus_keys_depth`]'s existing BFS
    /// (W2.2) rather than a parallel walk, converting its tagged
    /// `FocusSet` keys back to plain `NodeIndex` via the even/odd
    /// convention `graph.rs`'s `From<NodeIndex> for u64` already
    /// established (node keys are even).
    fn local_bfs_nodes(&self, root: NodeIndex, depth: u8) -> HashSet<NodeIndex> {
        self.graph
            .neighborhood_focus_keys_depth(root, depth)
            .into_iter()
            .filter(|k| k % 2 == 0)
            .map(|k| NodeIndex((k >> 1) as u32))
            .collect()
    }

    fn refresh_visible(&mut self) {
        let culled = gr_render::cull_visible(&self.graph, &self.particles, &self.camera, self.canvas_rect, self.cull_margin_world);
        let excluded = self.compute_excluded_nodes();
        self.visible =
            if excluded.is_empty() { culled } else { culled.into_iter().filter(|id| !excluded.contains(id)).collect() };
    }

    pub fn visible_nodes(&self) -> &[NodeIndex] {
        &self.visible
    }

    /// Refresh the culled/visible set and paint nodes+edges (plus, when
    /// any cluster is collapsed, the aggregated cross-cluster edges and
    /// the collapsed-supernode double-ring/count-label overlay — see
    /// `render::{draw_cluster_edges, draw_cluster_supernodes}`). Call
    /// once per frame while the canvas is on screen.
    pub fn draw(&mut self, render: &mut dyn RenderContext) {
        self.refresh_visible();
        // Wave 2.6: the SAME exclusion set `refresh_visible` just used
        // (cluster-hidden ∪ local-subgraph ∪ filter) — `draw_edges` skips
        // any edge touching a member of this set, which is exactly how
        // Wave 2.6's "edges to filtered/local-excluded nodes drop" is
        // satisfied, with zero new render-side logic.
        let hidden = self.compute_excluded_nodes();
        // Collapsed-cluster representatives always keep their label
        // (Wave 2.3 forced-label union) — hover/selection-neighbor
        // forcing needs no entry here, `draw_nodes` derives that
        // straight from `focus`.
        let forced_labels: HashSet<NodeIndex> = self.clusters.collapsed_clusters().map(|c| c.representative).collect();
        let ctx = gr_render::DrawContext {
            camera: &self.camera,
            viewport: self.canvas_rect,
            visible: &self.visible,
            focus: &self.focus,
            selection: &self.selection,
            hovered: self.hovered,
            hidden: &hidden,
            label_density: self.label_density,
            label_halo: &self.label_halo,
            forced_labels: &forced_labels,
            label_lod: &self.label_lod,
            theme: &self.theme,
        };
        gr_render::draw_edges(render, &self.graph, &self.particles, &ctx);
        gr_render::draw_cluster_edges(render, &self.particles, &ctx, &self.clusters);
        let node_stats = gr_render::draw_nodes(render, &self.graph, &self.particles, &ctx);
        self.labels_drawn_last_frame = node_stats.labels_drawn;
        gr_render::draw_cluster_supernodes(render, &self.graph, &self.particles, &ctx, &self.clusters);

        // Wave 2.4 live rubber-band overlay — drawn LAST so it always
        // sits on top of nodes/edges/cluster supernodes, matching every
        // surveyed engine's own convention (the selection box is always
        // the topmost overlay while dragging).
        if let Some(rect) = self.box_select_rect() {
            gr_render::draw_box_select_rect(render, rect, &self.theme);
        }

        if self.hover_card {
            if let Some(id) = self.hovered {
                if let Some(facts) = self.node_facts(id) {
                    let anchor = self.camera.world_to_screen((facts.position.0 as f64, facts.position.1 as f64), self.canvas_rect);
                    let info = gr_render::HoverCardInfo {
                        label: facts.label,
                        category: facts.category,
                        degree: facts.degree,
                        pinned: facts.pinned,
                    };
                    gr_render::draw_hover_card(render, anchor, &info, self.canvas_rect, &self.theme.hover_card);
                }
            }
        }
    }

    pub fn fit_view(&mut self) {
        let points: Vec<(f64, f64)> = self.particles.iter().map(|p| (p.x as f64, p.y as f64)).collect();
        if let Some(aabb) = Aabb::from_points(&points) {
            self.camera.fit_view(aabb, self.canvas_rect);
        }
        self.dirty = true;
    }

    /// Animated zoom-to-fit (Wave 2.5 — oss doc §2.6, d3-zoom/sigma
    /// `Camera.animate` convention). Fits whichever nodes are currently
    /// EFFECTIVE (local-subgraph ∩ filter — [`GraphEngine::
    /// compute_excluded_nodes`]'s complement), NOT literally every
    /// particle like the existing instant [`GraphEngine::fit_view`],
    /// which stays untouched (a new sibling, not a replacement — the task
    /// itself says "keep it"). `padding_px` is a literal screen-space
    /// margin on every side ([`crate::camera::fit_target`]'s convention),
    /// unlike `fit_view`'s existing ratio-based margin. No-op if every
    /// node is currently excluded (an empty AABB has nothing to fit).
    pub fn zoom_to_fit(&mut self, duration_ms: f64, padding_px: f64) {
        let excluded = self.compute_excluded_nodes();
        let points: Vec<(f64, f64)> = self
            .graph
            .nodes()
            .filter(|(id, _)| !excluded.contains(id))
            .filter_map(|(id, _)| self.particles.get(id.index()).map(|p| (p.x as f64, p.y as f64)))
            .collect();
        let Some(aabb) = Aabb::from_points(&points) else { return };
        let (target_pan, target_zoom) = crate::camera::fit_target(aabb, self.canvas_rect, padding_px);
        self.start_camera_transition(target_pan, target_zoom, duration_ms);
    }

    /// Animated pan(+zoom) to center `node` in the viewport (Wave 2.5).
    /// `target_zoom` `None` keeps the CURRENT zoom (a pure pan-to-center);
    /// `Some(z)` animates zoom too. Returns `false` (no-op, camera
    /// untouched) if `node` has no particle (out of range).
    pub fn zoom_to_node(&mut self, node: NodeIndex, duration_ms: f64, target_zoom: Option<f64>) -> bool {
        let Some(p) = self.particles.get(node.index()) else { return false };
        let zoom = target_zoom.unwrap_or(self.camera.zoom).clamp(crate::camera::ZOOM_MIN, crate::camera::ZOOM_MAX);
        let target_pan =
            (self.canvas_rect.width / 2.0 - p.x as f64 * zoom, self.canvas_rect.height / 2.0 - p.y as f64 * zoom);
        self.start_camera_transition(target_pan, zoom, duration_ms);
        true
    }

    fn start_camera_transition(&mut self, target_pan: (f64, f64), target_zoom: f64, duration_ms: f64) {
        self.camera_transition = Some(CameraTransition::new(self.camera, target_pan, target_zoom, duration_ms));
        self.dirty = true;
    }

    /// Whether an animated camera transition is currently in flight —
    /// `agent_state`'s `camera` block and [`GraphEngine::is_hot`] both
    /// read this indirectly; exposed directly for tests/callers that want
    /// to assert an animation is (or isn't) still running.
    pub fn camera_transitioning(&self) -> bool {
        self.camera_transition.is_some()
    }

    /// Wave 2.6 local subgraph mode (Juggl-proven hairball answer,
    /// obsidian doc §9): restrict the visible/pick/label set to the BFS
    /// depth-`depth` neighborhood of `root` — everything else is
    /// EXCLUDED entirely (not dimmed) from render/pick/labels, the same
    /// mechanism a collapsed cluster's hidden members already use. The
    /// simulation itself is UNTOUCHED — every particle keeps ticking
    /// under the full, unrestricted force topology; this is a VIEW
    /// restriction only (contrast [`GraphEngine::set_filter`], which DOES
    /// change the force topology). `depth` defaults to
    /// [`DEFAULT_LOCAL_DEPTH`] (`2`) when `None`. `root = None` restores
    /// the full view. Either way, animates the camera to fit the
    /// resulting effective set — activation frames the local
    /// neighborhood, clearing frames the whole graph again ("restores
    /// full view" read literally as restoring the camera too, not just
    /// the node set).
    pub fn set_local_root(&mut self, root: Option<NodeIndex>, depth: Option<u8>) {
        self.local_root = root.map(|node| (node, depth.unwrap_or(DEFAULT_LOCAL_DEPTH)));
        self.mark_dirty();
        self.zoom_to_fit(DEFAULT_TRANSITION_MS, DEFAULT_FIT_PADDING_PX);
    }

    /// Current local-subgraph root + depth, if active. See
    /// [`GraphEngine::set_local_root`].
    pub fn local_root(&self) -> Option<(NodeIndex, u8)> {
        self.local_root
    }

    /// Wave 2.6 query filter (obsidian doc §12 — decoupled from color
    /// groups per that section's own finding): filtered-out nodes are
    /// EXCLUDED from render/pick/labels (the same mechanism
    /// [`GraphEngine::set_local_root`] uses) AND from the force topology
    /// fed to `Layout::tick` every subsequent [`GraphEngine::tick`] call —
    /// an edge is dropped from the topology the instant EITHER endpoint
    /// fails the filter, so surviving nodes' link-force no longer pulls
    /// toward an excluded one (`tick`'s own filtered-topology branch).
    /// Reheats so the re-settle under the new topology is visible, not
    /// frozen wherever alpha happened to land.
    pub fn set_filter(&mut self, filter: Option<FilterSpec>) {
        self.filter = filter;
        self.reheat(self.interaction.drag_reheat_alpha);
    }

    /// Current query filter, if active. See [`GraphEngine::set_filter`].
    pub fn filter(&self) -> Option<&FilterSpec> {
        self.filter.as_ref()
    }

    /// A CLICK (or a solo drag-then-release — `on_pointer_up`'s
    /// `DraggingNode` arm) always Replace-selects: `selected` (facts
    /// panel) AND `selection` (the multi-select set focus/render derive
    /// from) both become exactly `{node}` — Wave 2.4 scope item 1's
    /// literal "Click = Replace-select {node}."
    pub fn select(&mut self, node: NodeIndex) {
        self.selected = Some(node);
        self.selection = std::iter::once(node).collect();
        self.refresh_focus();
        self.dirty = true;
    }

    /// Clears BOTH the persistent click-selection and the multi-selection
    /// set. A hover already in progress (the pointer never left the
    /// hovered node) resumes driving the highlight/dim focus set
    /// immediately — selection and hover share one `focus`, selection
    /// just takes precedence while it's active (Orb's `isStateOverride`
    /// precedent, oss doc §2.1; Wave 2.4 generalizes this to "a
    /// non-empty `selection`, not just a single `selected`, wins").
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.selection.clear();
        self.refresh_focus();
        self.dirty = true;
    }

    /// Combine `nodes` into the current [`GraphEngine::selection`] per
    /// `mode` (Wave 2.4 — G6's box-select mode model). Drives
    /// [`GraphEngine::box_select`] AND the `select_nodes`/`box_select`
    /// agent actions — one pipeline for both the pointer path and
    /// headless callers. Deliberately does NOT touch
    /// [`GraphEngine::selected`] (the "last individually clicked"
    /// facts-panel value): a bulk selection op is a different gesture
    /// from a click, and only [`GraphEngine::select`]/
    /// [`GraphEngine::clear_selection`] (an actual click, or a solo-node
    /// drag-release) ever change what the facts panel shows.
    pub fn apply_selection(&mut self, nodes: impl IntoIterator<Item = NodeIndex>, mode: SelectMode) {
        match mode {
            SelectMode::Replace => self.selection = nodes.into_iter().collect(),
            SelectMode::Union => self.selection.extend(nodes),
            SelectMode::Diff => {
                for node in nodes {
                    if !self.selection.remove(&node) {
                        self.selection.insert(node);
                    }
                }
            }
        }
        self.refresh_focus();
        self.dirty = true;
    }

    /// Screen-space rectangle box-select (Wave 2.4) — drives BOTH the
    /// pointer path (Shift/Ctrl+Shift/Alt+Shift+drag, see
    /// [`box_select_mode_for`]) and the `box_select` agent action, so a
    /// headless caller reaches the exact same selection pipeline a real
    /// drag does. `corner_a`/`corner_b` are any two opposite corners in
    /// screen px, order-independent (corner-normalized internally).
    /// Candidates are drawn from [`GraphEngine::visible_nodes`] — matches
    /// every other pick/render pass in this crate, an off-screen node
    /// can't be box-selected; containment is a node's SCREEN CENTER
    /// falling inside the rect (cytoscape's own simpler default, oss doc
    /// §2.3 — not full node-bounds overlap).
    pub fn box_select(&mut self, corner_a: (f64, f64), corner_b: (f64, f64), mode: SelectMode) {
        let rect = normalized_rect(corner_a, corner_b);
        let nodes = self.nodes_in_screen_rect(rect);
        self.apply_selection(nodes, mode);
    }

    fn nodes_in_screen_rect(&self, rect: Rect) -> Vec<NodeIndex> {
        self.visible
            .iter()
            .copied()
            .filter(|&id| match self.particles.get(id.index()) {
                Some(p) => {
                    let (sx, sy) = self.camera.world_to_screen((p.x as f64, p.y as f64), self.canvas_rect);
                    rect.contains(sx, sy)
                }
                None => false,
            })
            .collect()
    }

    /// Live screen-space rectangle of an in-progress box-select drag,
    /// already corner-normalized — `None` when no box-select is active.
    /// [`GraphEngine::draw`] paints the rubber-band overlay from this
    /// every frame; `on_pointer_up`'s `BoxSelecting` arm resolves the
    /// FINAL node set from the exact same corners via
    /// [`GraphEngine::box_select`], so the drawn rectangle and the
    /// actually-selected area can never diverge.
    pub fn box_select_rect(&self) -> Option<Rect> {
        match self.mode {
            PointerMode::BoxSelecting { origin, current, .. } => Some(normalized_rect(origin, current)),
            _ => None,
        }
    }

    /// Current keyboard-modifier state (Wave 2.4) — see
    /// [`box_select_mode_for`] for how `on_pointer_down` reads it.
    pub fn modifiers(&self) -> ModifierKeys {
        self.modifiers
    }

    /// Define a cluster over the CURRENT [`GraphEngine::selection`] and
    /// immediately collapse it (Wave 2.4 — the interactive-grouping path
    /// the spec promised: [`ClusterRegistry::define`]/
    /// [`GraphEngine::collapse_cluster`] already existed, this is the
    /// "from whatever's currently selected" entry point). Members are the
    /// selection's CURRENT contents in `BTreeSet`'s deterministic
    /// ascending-`NodeIndex` order — the same selection always yields the
    /// same collapse representative (the lowest index) regardless of
    /// click/box-select order. `None` if the selection is empty.
    pub fn collapse_selection(&mut self) -> Option<GroupId> {
        if self.selection.is_empty() {
            return None;
        }
        let members: Vec<NodeIndex> = self.selection.iter().copied().collect();
        let id = self.define_cluster(members)?;
        self.collapse_cluster(id);
        Some(id)
    }

    /// The [`GroupId`] whose FULL member set is EXACTLY the current
    /// selection and which is currently collapsed — backs `agent_state`'s
    /// `selection.collapsed_group`. Purely derived (no separate "which
    /// cluster did I last collapse" bookkeeping needed): `None` once
    /// expanded, if the selection was never turned into a cluster, or if
    /// the selection has since changed.
    pub fn selection_collapsed_group(&self) -> Option<GroupId> {
        if self.selection.is_empty() {
            return None;
        }
        self.clusters.iter().find_map(|(id, cluster)| {
            if !cluster.is_collapsed() {
                return None;
            }
            let members: BTreeSet<NodeIndex> = cluster.members.iter().copied().collect();
            (members == self.selection).then_some(id)
        })
    }

    /// Persistently pin every node in the current selection at its
    /// current position (Wave 2.4 group op — see
    /// [`GraphEngine::pin_node`]).
    pub fn pin_selection(&mut self) {
        let nodes: Vec<NodeIndex> = self.selection.iter().copied().collect();
        for node in nodes {
            self.pin_node(node);
        }
    }

    /// Release the persistent pin on every node in the current selection
    /// (Wave 2.4 group op — see [`GraphEngine::unpin_node`]).
    pub fn unpin_selection(&mut self) {
        let nodes: Vec<NodeIndex> = self.selection.iter().copied().collect();
        for node in nodes {
            self.unpin_node(node);
        }
    }

    /// Hover-neighbor-highlight depth (Wave 2.2 — oss doc §2.1). `0`
    /// highlights only the hovered node; `1` (default) adds its direct
    /// neighbors + connecting edges; higher values walk further hops
    /// (see [`Graph::neighborhood_focus_keys_depth`]).
    pub fn hover_depth(&self) -> u8 {
        self.hover_depth
    }

    /// Change the hover-neighbor-highlight depth, immediately
    /// recomputing the focus set from the CURRENT hover (if any and if
    /// no click-selection is overriding it) so the new depth is visible
    /// without waiting for the next pointer move.
    pub fn set_hover_depth(&mut self, depth: u8) {
        self.hover_depth = depth;
        if self.selection.is_empty() {
            self.refresh_focus();
            self.dirty = true;
        }
    }

    /// Whether [`GraphEngine::draw`] paints the floating hover info card
    /// (label/category/degree/pinned) near the hovered node. Default
    /// `true`.
    pub fn hover_card_enabled(&self) -> bool {
        self.hover_card
    }

    pub fn set_hover_card_enabled(&mut self, enabled: bool) {
        self.hover_card = enabled;
        self.dirty = true;
    }

    /// Label-LOD density param (Wave 2.3 — `crate::label_grid`'s sigma
    /// `LabelGrid` port). "Labels per 100px grid cell at zoom 1.0" —
    /// default [`label_grid::DEFAULT_LABEL_DENSITY`].
    pub fn label_density(&self) -> f64 {
        self.label_density
    }

    /// Negative values clamp to `0.0` (an empty per-cell quota — only
    /// forced labels, e.g. hover/selection/collapsed-cluster
    /// representatives, would show).
    pub fn set_label_density(&mut self, density: f64) {
        self.label_density = density.max(0.0);
        self.dirty = true;
    }

    /// Node-label halo color (a 4-direction offset-fill "stroke text"
    /// painted in this color under the real label fill — see
    /// `crate::render::draw_nodes`'s own doc comment) — default
    /// [`DEFAULT_LABEL_HALO`], matching this crate's own demo/showcase
    /// canvas background.
    pub fn label_halo(&self) -> &str {
        &self.label_halo
    }

    /// Set this engine's own node-label halo color to match a caller's OWN
    /// canvas background — `GraphEngine::draw` never paints a background
    /// itself (a caller's own `fill_rect` does, before calling `draw`), so
    /// this can't be inferred automatically.
    pub fn set_label_halo(&mut self, color: impl Into<String>) {
        self.label_halo = color.into();
        self.dirty = true;
    }

    /// Every paint color/font `crate::render`'s draw functions read (graph-
    /// strengthening arc Wave G2 / 2D quality audit A3) — default
    /// [`GraphTheme::dark`], byte-identical to this crate's pre-existing
    /// hardcoded literals.
    pub fn theme(&self) -> &GraphTheme {
        &self.theme
    }

    /// Set this engine's own theme — e.g. [`GraphTheme::light`]/
    /// [`GraphTheme::high_contrast`], or a caller-built [`GraphTheme`]
    /// (see that struct's own doc comment for a caller wanting the
    /// colorblind-safe Okabe-Ito categorical palette via
    /// `GraphTheme { category_palette: ..., ..GraphTheme::dark() }`).
    pub fn set_theme(&mut self, theme: GraphTheme) {
        self.theme = theme;
        self.dirty = true;
    }

    /// Label-LOD tuning (grid cell size, zoom fade window, degree shift —
    /// Wave G2) — default [`label_grid::LabelLodConfig::default`],
    /// byte-identical to this crate's pre-existing module constants.
    pub fn label_lod(&self) -> &label_grid::LabelLodConfig {
        &self.label_lod
    }

    pub fn set_label_lod(&mut self, lod: label_grid::LabelLodConfig) {
        self.label_lod = lod;
        self.dirty = true;
    }

    /// Keyboard/scroll/drag "feel" constants (Wave G2) — default
    /// [`GraphInteractionConfig::default`], byte-identical to this crate's
    /// pre-existing module constants.
    pub fn interaction_config(&self) -> &GraphInteractionConfig {
        &self.interaction
    }

    pub fn set_interaction_config(&mut self, config: GraphInteractionConfig) {
        self.interaction = config;
    }

    /// Viewport-cull world-space margin (Wave G2 — see [`DEFAULT_CULL_MARGIN_WORLD`]).
    /// Does NOT scale with zoom (a known, tracked limitation — 2D audit
    /// B4 — out of this configurability wave's scope, which changes
    /// nothing about WHAT the margin does, only that it's overridable).
    pub fn cull_margin_world(&self) -> f64 {
        self.cull_margin_world
    }

    /// Negative values clamp to `0.0` (an empty margin — nodes may pop at
    /// the exact viewport edge).
    pub fn set_cull_margin_world(&mut self, margin: f64) {
        self.cull_margin_world = margin.max(0.0);
        self.dirty = true;
    }

    /// Labels actually drawn on the last [`GraphEngine::draw`] call — a
    /// test/verification aid (Wave 2.3 gate) surfaced in `agent_state`'s
    /// `labels.drawn_last_frame` field.
    pub fn labels_drawn_last_frame(&self) -> usize {
        self.labels_drawn_last_frame
    }

    /// The currently hovered node, if any — same value `agent_state`'s
    /// `hover` field and [`GraphEngine::draw`]'s info card read.
    pub fn hovered(&self) -> Option<NodeIndex> {
        self.hovered
    }

    /// Set (or clear, `None`) the hovered node directly — the same path
    /// `on_pointer_moved`'s picking drives, exposed so the `hover_node`
    /// agent action (and any other headless driver) can reach the exact
    /// same behavior, including the reducer-style focus-set replace and
    /// the sigma-style "already this node — no-op" dedup guard.
    pub(crate) fn set_hovered(&mut self, hit: Option<NodeIndex>) {
        if hit == self.hovered {
            return;
        }
        self.hovered = hit;
        if self.selection.is_empty() {
            self.refresh_focus();
        }
        self.dirty = true;
    }

    /// Reducer-style paint override (oss doc §2.1: sigma `nodeReducer`/
    /// `edgeReducer`, replace-not-merge), Wave 2.4-generalized to a SET:
    /// recompute the WHOLE `focus` from `self.selection` (if non-empty —
    /// the UNION of every selected node's 1-hop neighborhood) else
    /// `self.hovered` at `self.hover_depth` — pure function of current
    /// state, never an incremental patch. A non-empty selection (single
    /// OR multi) always wins over a concurrent hover (Orb's
    /// `isStateOverride` precedent, unchanged from W2.2 — "a
    /// multi-selection counts as selection" is the literal
    /// generalization of that same precedence rule, not a new one).
    fn refresh_focus(&mut self) {
        if !self.selection.is_empty() {
            let mut keys: Vec<u64> = Vec::new();
            for &node in &self.selection {
                keys.extend(self.graph.neighborhood_focus_keys(node));
            }
            self.focus.select_many(keys);
            return;
        }
        match self.hovered {
            Some(node) => self.focus.select_many(self.graph.neighborhood_focus_keys_depth(node, self.hover_depth)),
            None => {
                self.focus.clear_selection();
            }
        }
    }

    /// Declare a cluster over `members` (first member becomes the
    /// collapse representative — see `cluster.rs` module docs). `None`
    /// if `members` is empty.
    pub fn define_cluster(&mut self, members: Vec<NodeIndex>) -> Option<GroupId> {
        self.clusters.define(&self.graph, members)
    }

    pub fn is_collapsed(&self, id: GroupId) -> bool {
        self.clusters.is_collapsed(id)
    }

    /// Collapse `id` into one super-node — centroid position, member-
    /// count radius, aggregated cross-cluster edge weights (see
    /// `cluster.rs`). No-op (`false`) if `id` is unknown or already
    /// collapsed.
    pub fn collapse_cluster(&mut self, id: GroupId) -> bool {
        let ok = self.clusters.collapse(id, &mut self.graph, &mut self.particles);
        if ok {
            self.dirty = true;
        }
        ok
    }

    /// Expand `id` back to its individual members, restoring EXACT
    /// pre-collapse positions (round-trip identity — see `cluster.rs`),
    /// then reheats so the layout visibly resettles around them (same
    /// convention as [`GraphEngine::unpin_node`]). No-op (`false`) if
    /// `id` is unknown or not currently collapsed.
    pub fn expand_cluster(&mut self, id: GroupId) -> bool {
        let ok = self.clusters.expand(id, &mut self.graph, &mut self.particles);
        if ok {
            self.reheat(self.interaction.drag_reheat_alpha);
        }
        ok
    }

    pub fn is_pinned(&self, node: NodeIndex) -> bool {
        self.pinned.get(node.index()).copied().unwrap_or(false)
    }

    /// Current drag-release behavior — see [`DragEndPolicy`]. Defaults
    /// to [`DragEndPolicy::Sticky`].
    pub fn drag_end_policy(&self) -> DragEndPolicy {
        self.drag_end_policy
    }

    pub fn set_drag_end_policy(&mut self, policy: DragEndPolicy) {
        self.drag_end_policy = policy;
    }

    /// Persistently pin `node` at its current position (survives drag
    /// release — Obsidian's explicit-pin affordance, distinct from
    /// "drag holds position while the button is down").
    pub fn pin_node(&mut self, node: NodeIndex) {
        if let Some(p) = self.particles.get_mut(node.index()) {
            p.pin(p.x, p.y);
        }
        if let Some(flag) = self.pinned.get_mut(node.index()) {
            *flag = true;
        }
        self.dirty = true;
    }

    /// Release a persistent pin and reheat so the node visibly rejoins
    /// the simulation instead of sitting frozen with nothing to prove
    /// it's alive again.
    pub fn unpin_node(&mut self, node: NodeIndex) {
        if let Some(p) = self.particles.get_mut(node.index()) {
            p.unpin();
        }
        if let Some(flag) = self.pinned.get_mut(node.index()) {
            *flag = false;
        }
        self.reheat(self.interaction.drag_reheat_alpha);
    }

    /// Current force-model parameters, if the engine's active layout has
    /// one — a bare [`ForceDirectedLayout`] (`L = ForceDirectedLayout`),
    /// or a [`GraphLayoutMode`] dispatcher (`L = GraphLayoutMode`,
    /// regardless of which concrete mode is currently active — the force
    /// instance persists even while hierarchical/radial is selected,
    /// same as `GraphLayoutMode::hierarchical_params`/`radial_params`).
    /// `None` for any other `L` (hierarchical/radial layouts have their
    /// own differently-shaped `*Params`, not this one).
    pub fn force_params(&self) -> Option<ForceParams>
    where
        L: 'static,
    {
        let layout_any: &dyn std::any::Any = &self.layout;
        if let Some(force) = layout_any.downcast_ref::<ForceDirectedLayout>() {
            return Some(*force.params());
        }
        if let Some(mode) = layout_any.downcast_ref::<GraphLayoutMode>() {
            return Some(*mode.force_params());
        }
        None
    }

    /// Replace the active force-model parameters wholesale (Wave 2.1
    /// owner order — "хочу иметь возможность изменять силу притяжения")
    /// and reheat so the change is visible instead of sitting inert
    /// until the next unrelated wake. Returns `false` (no-op) if `L`
    /// isn't one of the two force-capable shapes
    /// [`GraphEngine::force_params`] documents.
    pub fn set_force_params(&mut self, params: ForceParams) -> bool
    where
        L: 'static,
    {
        let applied = {
            let layout_any: &mut dyn std::any::Any = &mut self.layout;
            if let Some(force) = layout_any.downcast_mut::<ForceDirectedLayout>() {
                force.set_params(params);
                true
            } else if let Some(mode) = layout_any.downcast_mut::<GraphLayoutMode>() {
                mode.set_force_params(params);
                true
            } else {
                false
            }
        };
        if applied {
            self.reheat(self.interaction.drag_reheat_alpha);
        }
        applied
    }

    pub fn node_facts(&self, node: NodeIndex) -> Option<NodeFacts<'_>> {
        let n = self.graph.get_node(node)?;
        let p = self.particles.get(node.index())?;
        Some(NodeFacts {
            index: node,
            label: &n.label,
            category: &n.category,
            degree: self.graph.degree(node),
            position: (p.x, p.y),
            pinned: self.is_pinned(node),
        })
    }

    pub fn selected_facts(&self) -> Option<NodeFacts<'_>> {
        self.selected.and_then(|id| self.node_facts(id))
    }

    /// Raw `PlatformEvent` handler — canvas pan/zoom, node pick/drag,
    /// click-select, hover. Wire this from `App::on_event`. Returns
    /// `true` if the event was consumed.
    pub fn on_event(&mut self, event: &PlatformEvent) -> bool {
        match event {
            PlatformEvent::PointerDown { x, y, button: MouseButton::Left } => self.on_pointer_down(*x, *y),
            PlatformEvent::PointerMoved { x, y } => self.on_pointer_moved(*x, *y),
            PlatformEvent::PointerUp { x, y, button: MouseButton::Left } => self.on_pointer_up(*x, *y),
            PlatformEvent::Scroll { dy, .. } => self.on_scroll(*dy),
            // Wave 2.4 modifier tracking — `on_pointer_down` reads
            // `self.modifiers` to decide box-select vs. camera-pan/
            // node-drag (see `box_select_mode_for`).
            PlatformEvent::ModifiersChanged { modifiers } => {
                self.modifiers = *modifiers;
                true
            }
            // Wave 2.5 keyboard nav — hold-to-repeat pan/zoom, applied
            // every `tick()` for as long as the key stays held (see
            // `apply_held_nav_keys`).
            PlatformEvent::KeyDown { key, modifiers } => {
                self.modifiers = *modifiers;
                self.on_key_down(*key)
            }
            PlatformEvent::KeyUp { key, modifiers } => {
                self.modifiers = *modifiers;
                self.on_key_up(*key)
            }
            // 2D quality audit A2 / graph-strengthening arc G1.4: a lost
            // `PointerUp` (mouse released outside the window — `uzor-window-
            // desktop` has no pointer-capture call anywhere, confirmed by
            // grep) otherwise wedges `PointerMode::DraggingNode`/
            // `PanningCamera`/`BoxSelecting` forever: `alpha_target` stays
            // raised, `is_hot()` never returns `false` again, and a dragged
            // node ghost-follows the cursor if it re-enters the canvas with
            // no button held. Both events finalize whatever gesture is in
            // progress exactly as a real `PointerUp` at the last known
            // pointer position would, by calling the SAME `on_pointer_up`
            // body — not a parallel cleanup path that could drift from it.
            PlatformEvent::PointerLeft => self.cancel_pointer_gesture(),
            PlatformEvent::WindowFocused(false) => self.cancel_pointer_gesture(),
            _ => false,
        }
    }

    /// Finalize any in-progress drag/pan/box-select at
    /// [`Self::last_pointer_screen`] — see [`Self::on_event`]'s
    /// `PointerLeft`/`WindowFocused(false)` arms for why this exists. A
    /// no-op (`false`, event not consumed) while [`PointerMode::Idle`], so
    /// a spurious `PointerLeft`/defocus with nothing in progress doesn't
    /// spuriously mark the canvas dirty.
    fn cancel_pointer_gesture(&mut self) -> bool {
        if matches!(self.mode, PointerMode::Idle) {
            return false;
        }
        let (x, y) = self.last_pointer_screen;
        self.on_pointer_up(x, y)
    }

    /// A mapped nav key going down (Wave 2.5): a direct user key-press is
    /// real-time input, so it wins over any in-flight PROGRAMMATIC camera
    /// transition (`zoom_to_fit`/`zoom_to_node`) — the same "a new
    /// animation supersedes an old one" precedent sigma's own
    /// `Camera.animate` documents, generalized here to "live user input
    /// supersedes a running animation." Returns `false` (event not
    /// consumed) for any key that isn't a mapped nav key.
    fn on_key_down(&mut self, key: KeyCode) -> bool {
        let Some(nav) = nav_key_for(key) else { return false };
        self.camera_transition = None;
        self.held_nav_keys.insert(nav);
        self.dirty = true;
        true
    }

    fn on_key_up(&mut self, key: KeyCode) -> bool {
        let Some(nav) = nav_key_for(key) else { return false };
        self.held_nav_keys.remove(&nav);
        true
    }

    /// Apply the Wave 2.4 group-drag shared-delta shift to every member
    /// of `self.drag_group`: each member's `fx`/`fy` becomes the
    /// anchor's current world position (re-projected from `screen`
    /// through the camera's own inverse transform — the existing
    /// "delta ÷ zoom, absolute reprojection, no drift" convention, W2.1)
    /// PLUS that member's own FIXED `offset_from_anchor` captured at
    /// drag-start — see [`DragMember`]'s doc comment for why this
    /// preserves relative offsets exactly and reproduces the pre-Wave-2.4
    /// solo-drag behavior byte-for-byte when the drag set has one member.
    fn apply_drag_shift(&mut self, screen: (f64, f64)) {
        let world = self.camera.screen_to_world(screen, self.canvas_rect);
        for member in &self.drag_group {
            if let Some(p) = self.particles.get_mut(member.node.index()) {
                p.fx = Some(world.0 as f32 + member.offset_from_anchor.0);
                p.fy = Some(world.1 as f32 + member.offset_from_anchor.1);
            }
        }
    }

    fn on_pointer_down(&mut self, x: f64, y: f64) -> bool {
        if !self.canvas_rect.contains(x, y) {
            return false;
        }
        // Wave 2.4: a held Shift ALWAYS starts a box-select, even when
        // the down-point lands directly on a node (the oss doc's own
        // cytoscape-issue-1583 note — modifier-held mousedown wins over
        // node-grab dispatch). See `box_select_mode_for`'s doc comment
        // for the full modifier -> mode mapping.
        if let Some(mode) = box_select_mode_for(self.modifiers) {
            self.mode = PointerMode::BoxSelecting { origin: (x, y), current: (x, y), mode };
            self.dirty = true;
            return true;
        }
        if let Some(hit) = pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible) {
            // Wave 2.4 group drag: dragging a node already IN the
            // multi-selection moves the WHOLE selection together;
            // dragging anything else drags just that one node (and, on
            // release, Replace-selects it — see `on_pointer_up`).
            let was_selected = self.selection.contains(&hit);
            self.drag_was_group = was_selected;
            let group: Vec<NodeIndex> = if was_selected { self.selection.iter().copied().collect() } else { vec![hit] };
            let anchor_pos = self.particles.get(hit.index()).map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
            self.drag_group = group
                .into_iter()
                .map(|node| {
                    let pos = self.particles.get(node.index()).map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));
                    DragMember {
                        node,
                        offset_from_anchor: (pos.0 - anchor_pos.0, pos.1 - anchor_pos.1),
                        prior_pinned: self.is_pinned(node),
                    }
                })
                .collect();

            self.mode = PointerMode::DraggingNode;
            self.drag.start(hit, (x, y));
            // Pin-during-drag for EVERY moved member (d3-force canon —
            // `fx`/`fy` are the ONLY pin primitive, drag-in-progress and
            // an explicit persistent pin share the same mechanism),
            // shared-delta shifted from the very first frame.
            self.apply_drag_shift((x, y));
            // Sustained reheat (obsidian doc §drag/d3-canon): hold alpha
            // at the drag target for the whole gesture instead of a
            // one-shot bump that starts cooling right away. `reheat`
            // jumps alpha straight to the target NOW; `set_alpha_target`
            // holds it there every subsequent tick until drag-end clears
            // it back to 0.0.
            self.layout.set_alpha_target(self.interaction.drag_alpha_target);
            self.reheat(self.interaction.drag_alpha_target);
        } else {
            // 2D quality audit A1 / graph-strengthening arc G1.5: a
            // background pan is real-time user input, so it wins over any
            // in-flight PROGRAMMATIC camera transition — the same rule
            // `on_key_down` already applies for the keyboard-nav channel
            // (see that method's own doc comment). Without this,
            // `advance_camera_transition`'s unconditional per-tick write
            // (`engine.rs`) fights the user's own drag every frame until
            // the animation finishes.
            self.camera_transition = None;
            self.mode = PointerMode::PanningCamera { last: (x, y), total: 0.0 };
        }
        self.dirty = true;
        true
    }

    fn on_pointer_moved(&mut self, x: f64, y: f64) -> bool {
        self.last_pointer_screen = (x, y);
        let mut handled = false;

        match self.mode {
            PointerMode::DraggingNode => {
                self.drag.update((x, y));
                if self.drag.dragging_node().is_some() {
                    self.apply_drag_shift((x, y));
                }
                handled = true;
            }
            PointerMode::PanningCamera { last, total } => {
                let dx = x - last.0;
                let dy = y - last.1;
                self.camera.pan_x += dx;
                self.camera.pan_y += dy;
                self.mode = PointerMode::PanningCamera { last: (x, y), total: total + (dx * dx + dy * dy).sqrt() };
                handled = true;
            }
            PointerMode::BoxSelecting { origin, mode, .. } => {
                self.mode = PointerMode::BoxSelecting { origin, current: (x, y), mode };
                handled = true;
            }
            PointerMode::Idle => {}
        }

        if self.canvas_rect.contains(x, y) {
            // Perf guard (Wave 2.2 §4): re-run the O(visible) nearest-node
            // scan only once the pointer has actually moved
            // `HOVER_PICK_MIN_MOVE_PX` since the last pick — a hot loop of
            // sub-pixel `PointerMoved` jitter at 534+ nodes must not
            // re-scan every single event.
            let moved_enough = match self.last_hover_pick_screen {
                Some((lx, ly)) => {
                    let dx = x - lx;
                    let dy = y - ly;
                    (dx * dx + dy * dy).sqrt() >= self.interaction.hover_pick_min_move_px
                }
                None => true,
            };
            if moved_enough {
                self.last_hover_pick_screen = Some((x, y));
                let hit = pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible);
                self.set_hovered(hit);
            }
            handled = true;
        } else if self.hovered.is_some() {
            self.set_hovered(None);
            self.last_hover_pick_screen = None;
        }

        if handled {
            self.dirty = true;
        }
        handled
    }

    fn on_pointer_up(&mut self, x: f64, y: f64) -> bool {
        match self.mode {
            PointerMode::DraggingNode => {
                if let Some((anchor, _response)) = self.drag.stop() {
                    let policy = self.drag_end_policy;
                    let was_group = self.drag_was_group;
                    // Wave 2.4: every MEMBER of the drag set pins/unpins
                    // per the drag-end policy (Sticky pins all of them;
                    // RestorePrior restores each member's OWN prior-pinned
                    // state — not just the anchor's).
                    for member in std::mem::take(&mut self.drag_group) {
                        let stay_pinned = match policy {
                            DragEndPolicy::Sticky => true,
                            DragEndPolicy::RestorePrior => member.prior_pinned,
                        };
                        if stay_pinned {
                            // `fx`/`fy` already hold the release-time world
                            // position from the last drag-move tick — just
                            // flip the persistent-pin bookkeeping flag so
                            // `is_pinned`/`unpin_node` see it correctly.
                            if let Some(flag) = self.pinned.get_mut(member.node.index()) {
                                *flag = true;
                            }
                        } else {
                            if let Some(p) = self.particles.get_mut(member.node.index()) {
                                p.unpin();
                            }
                            if let Some(flag) = self.pinned.get_mut(member.node.index()) {
                                *flag = false;
                            }
                        }
                    }
                    // Drag-end: alphaTarget -> 0 so alpha eases back down
                    // instead of free-decaying from wherever it was held.
                    self.layout.set_alpha_target(0.0);
                    if was_group {
                        // The whole multi-selection just moved together —
                        // it STAYS selected as-is (Wave 2.4 scope item 3);
                        // only the facts-panel "last clicked" value moves
                        // to the physically-grabbed anchor.
                        self.selected = Some(anchor);
                        self.refresh_focus();
                    } else {
                        // A single un-selected node was dragged solo —
                        // Replace-selects itself on release (matches the
                        // pre-Wave-2.4 behavior exactly for this case).
                        self.select(anchor);
                    }
                }
                self.mode = PointerMode::Idle;
                self.dirty = true;
                true
            }
            PointerMode::PanningCamera { total, .. } => {
                self.mode = PointerMode::Idle;
                if total < self.interaction.click_drag_threshold_px && self.canvas_rect.contains(x, y) {
                    match pick::nearest_node(&self.graph, &self.particles, &self.camera, self.canvas_rect, (x, y), &self.visible) {
                        // A collapsed super-node's designated expand
                        // affordance is a single click on it (this raw-
                        // `PlatformEvent` canvas path has no double-click
                        // in its input vocabulary — see `cluster.rs`
                        // module docs / the demo's `expand` agent action
                        // for the alternative route).
                        Some(hit) if self.clusters.cluster_of(hit).is_some_and(|id| self.clusters.is_collapsed(id)) => {
                            if let Some(id) = self.clusters.cluster_of(hit) {
                                self.expand_cluster(id);
                            }
                        }
                        Some(hit) => self.select(hit),
                        None => self.clear_selection(),
                    }
                }
                self.dirty = true;
                true
            }
            PointerMode::BoxSelecting { origin, mode, .. } => {
                self.mode = PointerMode::Idle;
                self.box_select(origin, (x, y), mode);
                self.dirty = true;
                true
            }
            PointerMode::Idle => false,
        }
    }

    fn on_scroll(&mut self, dy: f64) -> bool {
        if !self.canvas_rect.contains(self.last_pointer_screen.0, self.last_pointer_screen.1) {
            return false;
        }
        // Same "live user input supersedes a running animation" rule as
        // `on_key_down`/`on_pointer_down`'s background-pan branch — see
        // their doc comments. A wheel-zoom mid-`zoom_to_fit`/`zoom_to_node`
        // must win immediately, not fight the animation for the rest of
        // its duration.
        self.camera_transition = None;
        let factor = (1.0 + dy * self.interaction.zoom_sensitivity)
            .clamp(self.interaction.scroll_zoom_factor_min, self.interaction.scroll_zoom_factor_max);
        self.camera.zoom_at(self.last_pointer_screen, self.canvas_rect, factor);
        self.dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    type TestEngine = GraphEngine<(), (), ForceDirectedLayout>;

    /// `a - b - c` chain — `b` is `a`'s only 1-hop neighbor, `c` is 2
    /// hops away (outside the neighborhood `select` focuses on).
    fn chain_graph() -> (Graph<(), ()>, NodeIndex, NodeIndex, NodeIndex) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        (graph, a, b, c)
    }

    #[test]
    fn select_populates_the_repointed_figures_focus_set_with_neighborhood_keys() {
        let (graph, a, b, c) = chain_graph();
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.select(a);

        assert!(engine.focus.is_active());
        assert!(engine.focus.is_selected(u64::from(a)));
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(!engine.focus.is_selected(u64::from(c)), "c is 2 hops away — outside the 1-hop neighborhood");

        engine.clear_selection();
        assert!(!engine.focus.is_active());
    }

    #[test]
    fn reselecting_a_different_node_replaces_the_whole_focus_set() {
        let (graph, a, _b, c) = chain_graph();
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.select(a);
        assert!(engine.focus.is_selected(u64::from(a)));

        engine.select(c);
        assert!(!engine.focus.is_selected(u64::from(a)), "stale selection from the previous select() must not leak");
        assert!(engine.focus.is_selected(u64::from(c)));
    }

    // ── P0 pointer-plumbing gate (uzor-window-desktop mapper fix) ──────────
    //
    // These inject a full `PlatformEvent` sequence directly (as the engine
    // is wired from `App::on_event`), asserting the camera pans by exactly
    // the moved delta with no jump — the behavior downstream of the fixed
    // `EventMapper` (which used to stamp `PointerDown`/`Up` at hardcoded
    // `(0.0, 0.0)`, see `uzor-window-desktop/src/event_mapper.rs`). The
    // engine's own pan/pick code was already correct; these prove it stays
    // correct end-to-end once fed real coordinates, and document what the
    // old stale-zero bug looked like from the engine's point of view.

    fn empty_engine_with_canvas(rect: Rect) -> TestEngine {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        engine.set_canvas_rect(rect);
        engine
    }

    /// Down(bg point) -> Moved xN -> Up: each Move pans the camera by
    /// exactly that step's screen-space delta (zoom stays 1.0, so
    /// dividing by it is a no-op) — no jump from a stale (0,0) origin.
    #[test]
    fn background_drag_sequence_pans_camera_by_exact_per_step_delta() {
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        let mut engine = empty_engine_with_canvas(canvas);

        let grab = (300.0, 300.0);
        assert!(engine.on_event(&PlatformEvent::PointerDown {
            x: grab.0,
            y: grab.1,
            button: MouseButton::Left,
        }));
        assert_eq!((engine.camera.pan_x, engine.camera.pan_y), (0.0, 0.0),
            "PointerDown alone must not move the camera");

        let moves = [(310.0, 300.0), (325.0, 305.0), (325.0, 320.0)];
        let mut last = grab;
        for &(mx, my) in &moves {
            let before = (engine.camera.pan_x, engine.camera.pan_y);
            engine.on_event(&PlatformEvent::PointerMoved { x: mx, y: my });
            let (expected_dx, expected_dy) = (mx - last.0, my - last.1);
            assert!((engine.camera.pan_x - (before.0 + expected_dx)).abs() < 1e-9);
            assert!((engine.camera.pan_y - (before.1 + expected_dy)).abs() < 1e-9);
            last = (mx, my);
        }

        engine.on_event(&PlatformEvent::PointerUp { x: last.0, y: last.1, button: MouseButton::Left });
        // Total travel from (300,300) to (325,320): pan_x += 25, pan_y += 20.
        assert!((engine.camera.pan_x - 25.0).abs() < 1e-9);
        assert!((engine.camera.pan_y - 20.0).abs() < 1e-9);
    }

    /// Regression-shaped: a Down at the true grab point followed by a
    /// +10px Move pans by exactly +10px (screen-space, ÷ zoom == 1.0 here
    /// so it's a no-op) — contrasted against the OLD bug shape, where
    /// every `PointerDown` was stamped at `(0.0, 0.0)` regardless of the
    /// real cursor position, so the same physical move read as a
    /// multi-hundred-pixel teleport instead of a +10px pan.
    #[test]
    fn regression_stale_zero_down_would_teleport_vs_fixed_pipeline_pans_by_delta() {
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        let grab = (300.0, 300.0);

        // Fixed pipeline: Down carries the real grab point (what the
        // corrected stateful mapper now stamps from the last CursorMoved).
        let mut fixed = empty_engine_with_canvas(canvas);
        fixed.on_event(&PlatformEvent::PointerDown { x: grab.0, y: grab.1, button: MouseButton::Left });
        fixed.on_event(&PlatformEvent::PointerMoved { x: grab.0 + 10.0, y: grab.1 });
        assert!((fixed.camera.pan_x - 10.0).abs() < 1e-9,
            "a +10px move after a correctly-stamped Down pans by exactly +10px");
        assert!((fixed.camera.pan_y - 0.0).abs() < 1e-9);

        // Old bug shape: EventMapper::map_window_event stamped every
        // PointerDown at (0.0, 0.0) ("position will be updated by cursor
        // moved event" — nothing did). Reproduce that input shape directly
        // against the engine to document the failure it caused downstream.
        let mut buggy = empty_engine_with_canvas(canvas);
        buggy.on_event(&PlatformEvent::PointerDown { x: 0.0, y: 0.0, button: MouseButton::Left });
        buggy.on_event(&PlatformEvent::PointerMoved { x: grab.0 + 10.0, y: grab.1 });

        assert!(buggy.camera.pan_x > 100.0,
            "stale (0,0) Down turns the same +10px physical move into a \
             camera teleport of ~grab_x pixels — this is the P0 bug: {} \
             (fixed pipeline pans by exactly 10.0)", buggy.camera.pan_x);
    }

    // ── W2.1 drag-physics contract (sticky drag + alphaTarget) ─────────────
    //
    // A 2-node chain, camera left at its identity default (pan (0,0),
    // zoom 1.0) so screen == world and picking a node is just "click at
    // its seeded position". `refresh_visible()` (private, but reachable
    // here since `tests` is a descendant of this module) stands in for
    // the `draw()` call a real frame would make to populate the pick
    // candidate list.

    fn two_node_chain_engine() -> (TestEngine, NodeIndex, NodeIndex) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(100.0, 100.0), (300.0, 100.0)]);
        engine.refresh_visible();
        (engine, a, b)
    }

    #[test]
    fn sticky_drag_is_the_default_and_pins_the_node_at_the_release_position() {
        let (mut engine, a, _b) = two_node_chain_engine();
        assert_eq!(engine.drag_end_policy(), DragEndPolicy::Sticky, "Sticky must be the default policy");
        assert!(!engine.is_pinned(a));

        engine.on_event(&PlatformEvent::PointerDown { x: 100.0, y: 100.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 250.0, y: 220.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 250.0, y: 220.0, button: MouseButton::Left });
        // `x`/`y` only resync from the held `fx`/`fy` on the next
        // `tick()` (same as a real render frame would do) — one tick to
        // observe the release position land.
        engine.tick(1.0 / 60.0);

        assert!(engine.is_pinned(a), "a node dragged and released must be reported pinned under Sticky");
        let released = engine.particles[a.index()];
        assert!((released.x - 250.0).abs() < 1e-6);
        assert!((released.y - 220.0).abs() < 1e-6);

        // The pin must hold across many subsequent ticks, even with a
        // linked neighbor still under active force influence.
        for _ in 0..120 {
            engine.tick(1.0 / 60.0);
        }
        assert_eq!(engine.particles[a.index()].x, released.x);
        assert_eq!(engine.particles[a.index()].y, released.y);
    }

    #[test]
    fn restore_prior_policy_unfixes_a_previously_free_node_but_keeps_a_pre_pinned_one_pinned() {
        let (mut engine, a, b) = two_node_chain_engine();
        engine.set_drag_end_policy(DragEndPolicy::RestorePrior);

        // `a` was free before the drag -> released back into the sim.
        engine.on_event(&PlatformEvent::PointerDown { x: 100.0, y: 100.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 250.0, y: 220.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 250.0, y: 220.0, button: MouseButton::Left });
        assert!(!engine.is_pinned(a), "a node that was free before the drag must NOT stay pinned under RestorePrior");

        // `b` was explicitly pinned BEFORE the drag -> stays pinned
        // afterward, at the drag's release position (pin composes with
        // drag instead of the drag clobbering it).
        engine.pin_node(b);
        engine.on_event(&PlatformEvent::PointerDown { x: 300.0, y: 100.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 400.0, y: 150.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 400.0, y: 150.0, button: MouseButton::Left });
        engine.tick(1.0 / 60.0);
        assert!(engine.is_pinned(b), "a node pinned before the drag must stay pinned under RestorePrior");
        assert!((engine.particles[b.index()].x - 400.0).abs() < 1e-6);
        assert!((engine.particles[b.index()].y - 150.0).abs() < 1e-6);
    }

    #[test]
    fn drag_holds_alpha_near_the_sustained_target_and_decays_after_release() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        // Centered on the world origin: `center_strength` pulls the
        // settling pair toward world `(0, 0)`, so a canvas centered
        // there (rather than cornered at it) guarantees the click point
        // computed below — from wherever the pair actually settles —
        // stays inside the canvas regardless of drift direction.
        let canvas = Rect::new(-400.0, -300.0, 800.0, 600.0);
        engine.set_canvas_rect(canvas);
        engine.seed_positions(&[(100.0, 100.0), (300.0, 100.0)]);

        // Fully settle first — the "sustained, not one-shot" contract
        // only bites from a cold/settled start (a one-shot bump also
        // looks fine on the very first frame after seeding). Settling
        // moves `a` away from its seeded position, so the drag below
        // clicks its ACTUAL (post-settle) screen position, computed
        // through the real camera transform, not the stale seed.
        for _ in 0..600 {
            engine.tick(1.0 / 60.0);
        }
        assert!(!engine.is_hot(), "fixture must settle before the drag starts");
        engine.refresh_visible();
        let click = {
            let p = engine.particles[a.index()];
            engine.camera.world_to_screen((p.x as f64, p.y as f64), canvas)
        };

        engine.on_event(&PlatformEvent::PointerDown { x: click.0, y: click.1, button: MouseButton::Left });
        let mut min_alpha = f32::MAX;
        let mut max_alpha = f32::MIN;
        for i in 0..90 {
            if i % 10 == 0 {
                engine.on_event(&PlatformEvent::PointerMoved { x: click.0 + i as f64, y: click.1 });
            }
            let r = engine.tick(1.0 / 60.0);
            min_alpha = min_alpha.min(r.alpha);
            max_alpha = max_alpha.max(r.alpha);
        }
        assert!(
            (min_alpha - DRAG_ALPHA_TARGET).abs() < 0.01 && (max_alpha - DRAG_ALPHA_TARGET).abs() < 0.01,
            "alpha must hold near the sustained drag target for the whole gesture: min {min_alpha} max {max_alpha} (target {DRAG_ALPHA_TARGET})"
        );

        engine.on_event(&PlatformEvent::PointerUp { x: click.0 + 80.0, y: click.1, button: MouseButton::Left });
        let post_release = engine.tick(1.0 / 60.0);
        assert!(
            post_release.alpha < DRAG_ALPHA_TARGET - 1e-4,
            "alpha must start decaying immediately after drag-end clears alpha_target: {}",
            post_release.alpha
        );
        for _ in 0..600 {
            engine.tick(1.0 / 60.0);
        }
        assert!(!engine.is_hot(), "alpha must decay all the way back down once alpha_target is cleared");
    }

    // ── W2.2 hover system (reducer-style neighbor highlight, oss doc §2.1) ─

    /// `a - b - c - d` chain laid out on a line, 100 world units apart —
    /// with the default camera (pan (0,0), zoom 1.0) screen == world, so
    /// picking a node is just "hover at its seeded x".
    fn chain4_engine_on_a_line() -> (TestEngine, [NodeIndex; 4]) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        let d = graph.push_node((), "d", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        graph.push_edge(c, d, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (100.0, 0.0), (200.0, 0.0), (300.0, 0.0)]);
        engine.refresh_visible();
        (engine, [a, b, c, d])
    }

    #[test]
    fn pointer_moved_over_a_node_hovers_it_and_moving_to_empty_space_clears_it() {
        let (mut engine, [a, b, _c, _d]) = chain4_engine_on_a_line();
        assert!(engine.hovered().is_none());
        assert!(!engine.focus.is_active());

        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // exactly on b
        assert_eq!(engine.hovered(), Some(b));
        assert!(engine.focus.is_active());
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(engine.focus.is_selected(u64::from(a)), "b's depth-1 neighbor a must be highlighted too");

        engine.on_event(&PlatformEvent::PointerMoved { x: 700.0, y: 500.0 }); // empty space, far from every node
        assert!(engine.hovered().is_none());
        assert!(!engine.focus.is_active(), "moving off every node must clear the highlight entirely");
    }

    #[test]
    fn hover_neighborhood_highlights_exactly_the_hovered_node_and_its_depth_one_adjacency() {
        let (mut engine, [a, b, c, d]) = chain4_engine_on_a_line();
        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // b
        assert_eq!(engine.hovered(), Some(b));

        assert!(engine.focus.is_selected(u64::from(a)));
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(engine.focus.is_selected(u64::from(c)));
        assert!(!engine.focus.is_selected(u64::from(d)), "d is 2 hops from b — outside a depth-1 hover neighborhood");
    }

    #[test]
    fn hover_depth_zero_highlights_only_the_hovered_node_itself() {
        let (mut engine, [a, b, c, _d]) = chain4_engine_on_a_line();
        engine.set_hover_depth(0);
        assert_eq!(engine.hover_depth(), 0);

        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // b
        assert!(engine.focus.is_selected(u64::from(b)));
        assert!(!engine.focus.is_selected(u64::from(a)));
        assert!(!engine.focus.is_selected(u64::from(c)));
    }

    #[test]
    fn click_selection_takes_precedence_over_a_concurrent_hover_and_resumes_on_clear() {
        let (mut engine, [_a, b, _c, d]) = chain4_engine_on_a_line();
        engine.select(d); // click-select d — its 1-hop neighborhood is {c, d}
        assert!(engine.focus.is_selected(u64::from(d)));

        engine.on_event(&PlatformEvent::PointerMoved { x: 100.0, y: 0.0 }); // hover b, unrelated to the selection
        assert_eq!(engine.hovered(), Some(b));
        assert!(engine.focus.is_selected(u64::from(d)), "click-selection must win over a concurrent hover");
        assert!(!engine.focus.is_selected(u64::from(b)), "hover must not override an active click-selection");

        engine.clear_selection();
        // The pointer never left `b` — hover resumes driving the focus
        // set the instant the click-selection is no longer overriding it.
        assert!(engine.focus.is_selected(u64::from(b)), "clearing the selection must resume hover-driven focus for the node still under the cursor");
        assert!(!engine.focus.is_selected(u64::from(d)));
    }

    #[test]
    fn hover_pick_skips_recompute_for_sub_threshold_pointer_moves() {
        let (mut engine, [_a, b, _c, _d]) = chain4_engine_on_a_line();
        // b sits at world/screen (100, 0). `node_screen_radius(4.0)` at
        // zoom 1.0 is 4.0px, + `pick::HOVER_TOLERANCE_PX` (6.0) = a 10px
        // hit radius.
        engine.on_event(&PlatformEvent::PointerMoved { x: 109.9, y: 0.0 }); // 9.9px from b — inside
        assert_eq!(engine.hovered(), Some(b));

        // A <2px move that would, if re-picked, land JUST outside the hit
        // radius (10.9px from b) — the perf guard must keep the stale
        // hover instead of immediately re-scanning and clearing it.
        engine.on_event(&PlatformEvent::PointerMoved { x: 110.9, y: 0.0 });
        assert_eq!(engine.hovered(), Some(b), "a <2px move must not trigger a re-pick — stale hover kept");

        // A >=2px move (measured from the LAST PICK position, 109.9) does
        // trigger a fresh pick, which correctly clears the now-out-of-range hover.
        engine.on_event(&PlatformEvent::PointerMoved { x: 113.0, y: 0.0 });
        assert_eq!(engine.hovered(), None, "a >=2px move re-picks and correctly clears the hover");
    }

    // ── W2.3 label LOD (sigma LabelGrid port, oss doc §3/§label-lod) ───────

    #[test]
    fn label_density_defaults_and_set_label_density_updates_the_getter_and_marks_dirty() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(engine.label_density(), crate::label_grid::DEFAULT_LABEL_DENSITY);
        assert_eq!(engine.labels_drawn_last_frame(), 0);

        engine.clear_dirty();
        engine.set_label_density(2.5);
        assert_eq!(engine.label_density(), 2.5);
        assert!(engine.dirty(), "changing label_density must mark the canvas dirty");

        // Negative density clamps to 0.0 (an empty per-cell quota).
        engine.set_label_density(-4.0);
        assert_eq!(engine.label_density(), 0.0);
    }

    #[test]
    fn label_halo_defaults_and_set_label_halo_updates_the_getter_and_marks_dirty() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(engine.label_halo(), DEFAULT_LABEL_HALO, "default halo must match the crate's own demo/showcase canvas background");

        engine.clear_dirty();
        engine.set_label_halo("#ffffff");
        assert_eq!(engine.label_halo(), "#ffffff");
        assert!(engine.dirty(), "changing label_halo must mark the canvas dirty");
    }

    // ── Graph-strengthening arc G2: theme/label_lod/interaction_config/
    // cull_margin_world getter-setter round trips ───────────────────────

    #[test]
    fn theme_defaults_to_dark_and_set_theme_updates_the_getter_and_marks_dirty() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(engine.theme().selection_ring_color, GraphTheme::dark().selection_ring_color);

        engine.clear_dirty();
        engine.set_theme(GraphTheme::light());
        assert_eq!(engine.theme().selection_ring_color, GraphTheme::light().selection_ring_color);
        assert!(engine.dirty(), "changing theme must mark the canvas dirty");
    }

    #[test]
    fn label_lod_defaults_and_set_label_lod_updates_the_getter_and_marks_dirty() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(*engine.label_lod(), label_grid::LabelLodConfig::default());

        engine.clear_dirty();
        let custom = label_grid::LabelLodConfig { grid_cell_size_px: 50.0, ..label_grid::LabelLodConfig::default() };
        engine.set_label_lod(custom);
        assert_eq!(engine.label_lod().grid_cell_size_px, 50.0);
        assert!(engine.dirty(), "changing label_lod must mark the canvas dirty");
    }

    #[test]
    fn interaction_config_defaults_and_set_interaction_config_updates_the_getter() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(*engine.interaction_config(), GraphInteractionConfig::default());

        let custom = GraphInteractionConfig { key_pan_speed_px_per_s: 999.0, ..GraphInteractionConfig::default() };
        engine.set_interaction_config(custom);
        assert_eq!(engine.interaction_config().key_pan_speed_px_per_s, 999.0);
    }

    #[test]
    fn cull_margin_world_defaults_and_set_cull_margin_world_updates_the_getter_and_marks_dirty() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        assert_eq!(engine.cull_margin_world(), DEFAULT_CULL_MARGIN_WORLD);

        engine.clear_dirty();
        engine.set_cull_margin_world(200.0);
        assert_eq!(engine.cull_margin_world(), 200.0);
        assert!(engine.dirty(), "changing cull_margin_world must mark the canvas dirty");

        // Negative values clamp to 0.0 (no popup-hiding margin at all).
        engine.set_cull_margin_world(-10.0);
        assert_eq!(engine.cull_margin_world(), 0.0);
    }

    /// The keyboard-nav "feel" constants must actually be read from
    /// `self.interaction`, not the old hardcoded module constants —
    /// proves the Wave G2 wiring is real, not just a stored-but-unused
    /// config struct. A key-pan-speed set to 0 must produce NO pan at all
    /// while a `PanRight` key is held.
    #[test]
    fn interaction_config_key_pan_speed_actually_changes_apply_held_nav_keys() {
        let mut engine: TestEngine = GraphEngine::new(Graph::new(), ForceDirectedLayout::default());
        engine.set_interaction_config(GraphInteractionConfig { key_pan_speed_px_per_s: 0.0, ..GraphInteractionConfig::default() });
        let before = engine.camera.pan_x;
        engine.on_key_down(KeyCode::ArrowRight);
        engine.apply_held_nav_keys(1.0);
        assert_eq!(engine.camera.pan_x, before, "a zeroed key_pan_speed_px_per_s override must produce zero pan");
    }

    /// A real `draw()` call (through `uzor-export`'s headless render path,
    /// same as `lib.rs`'s `proof_tests`) must populate
    /// `labels_drawn_last_frame` from what `render::draw_nodes` actually
    /// drew — not stay stuck at its `0` initial value.
    #[test]
    fn draw_populates_labels_drawn_last_frame_from_the_render_pass() {
        use uzor_export::{render_to_png, ExportSpec};

        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 400.0, 300.0));
        engine.seed_positions(&[(0.0, 0.0), (50.0, 0.0)]);
        engine.camera.zoom = 2.0; // well above LOD_LABEL_FADE_HIGH — labels fully opaque

        let spec = ExportSpec { width_px: 400, height_px: 300, dpr: 1.0, background: None };
        render_to_png(&spec, |ctx| engine.draw(ctx)).expect("headless render must succeed");

        assert_eq!(engine.labels_drawn_last_frame(), 2, "both nodes sit in separate grid cells and must both draw a label");
    }

    // ── W2.4 selection model (box-select, group drag, cluster-from-selection) ─
    //
    // 4 nodes at the corners of a 100x100 square (screen == world at the
    // default camera) — `a`=(0,0) `b`=(100,0) `c`=(0,100) `d`=(100,100),
    // wired into a ring so `neighborhood_focus_keys` has something to
    // walk. `refresh_visible()` stands in for a real `draw()` frame.

    fn four_corner_square_engine() -> (TestEngine, [NodeIndex; 4]) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "x", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        let d = graph.push_node((), "d", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, d, 1.0, ());
        graph.push_edge(d, c, 1.0, ());
        graph.push_edge(c, a, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)]);
        engine.refresh_visible();
        (engine, [a, b, c, d])
    }

    /// The literal gate: Replace/Union/Diff each produce the EXACT
    /// expected set (not just "something changed").
    #[test]
    fn box_select_modes_produce_exact_expected_sets() {
        let (mut engine, [a, b, c, d]) = four_corner_square_engine();

        // Replace: box over the LEFT column (a, c) only.
        engine.box_select((-10.0, -10.0), (10.0, 110.0), SelectMode::Replace);
        assert_eq!(engine.selection, [a, c].into_iter().collect());

        // Union: box over the RIGHT column (b, d) ADDS to the existing {a, c}.
        engine.box_select((90.0, -10.0), (110.0, 110.0), SelectMode::Union);
        assert_eq!(engine.selection, [a, b, c, d].into_iter().collect());

        // Diff: box over the TOP row (a, b) toggles them OFF (both already selected).
        engine.box_select((-10.0, -10.0), (110.0, 10.0), SelectMode::Diff);
        assert_eq!(engine.selection, [c, d].into_iter().collect());

        // The SAME top-row box, Diff again, toggles a/b back ON (they
        // aren't currently selected) without touching c/d.
        engine.box_select((-10.0, -10.0), (110.0, 10.0), SelectMode::Diff);
        assert_eq!(engine.selection, [a, b, c, d].into_iter().collect());

        // Replace with an empty box clears the whole selection — the
        // natural generalization, no special-casing needed.
        engine.box_select((300.0, 300.0), (310.0, 310.0), SelectMode::Replace);
        assert!(engine.selection.is_empty());
    }

    /// Dragging a node that IS in the current multi-selection moves the
    /// WHOLE selection by one shared world-space delta — relative offsets
    /// between the two selected members must be preserved EXACTLY, and a
    /// node outside the selection must not move (or even get pinned) at
    /// all. Checked via `fx`/`fy` directly (set synchronously by
    /// `on_pointer_moved`, no `tick()` needed) so an unrelated node's own
    /// free-physics drift from an intervening tick can never confound the
    /// "did it move" assertion.
    #[test]
    fn group_drag_preserves_relative_offsets_via_a_single_shared_delta() {
        let (mut engine, [a, b, c, _d]) = four_corner_square_engine();
        engine.apply_selection([a, b], SelectMode::Replace);

        let a0 = engine.particles[a.index()];
        let b0 = engine.particles[b.index()];
        let c0 = engine.particles[c.index()];

        // Grab `a` (a selected member) and drag by (+30, -20).
        engine.on_event(&PlatformEvent::PointerDown { x: 0.0, y: 0.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 30.0, y: -20.0 });

        let a_fx = engine.particles[a.index()].fx.expect("a must be pinned mid-drag");
        let a_fy = engine.particles[a.index()].fy.expect("a must be pinned mid-drag");
        let b_fx = engine.particles[b.index()].fx.expect("b (also in the drag set) must be pinned too");
        let b_fy = engine.particles[b.index()].fy.expect("b (also in the drag set) must be pinned too");

        assert!((a_fx - (a0.x + 30.0)).abs() < 1e-4);
        assert!((a_fy - (a0.y - 20.0)).abs() < 1e-4);
        assert!((b_fx - (b0.x + 30.0)).abs() < 1e-4);
        assert!((b_fy - (b0.y - 20.0)).abs() < 1e-4);

        let rel_before = (b0.x - a0.x, b0.y - a0.y);
        let rel_after = (b_fx - a_fx, b_fy - a_fy);
        assert!((rel_after.0 - rel_before.0).abs() < 1e-4, "relative x-offset must be preserved exactly");
        assert!((rel_after.1 - rel_before.1).abs() < 1e-4, "relative y-offset must be preserved exactly");

        // c is NOT in the drag set — never pinned, position untouched.
        assert!(engine.particles[c.index()].fx.is_none());
        assert_eq!(engine.particles[c.index()].x, c0.x);
        assert_eq!(engine.particles[c.index()].y, c0.y);

        engine.on_event(&PlatformEvent::PointerUp { x: 30.0, y: -20.0, button: MouseButton::Left });
        // A group-drag release keeps the WHOLE selection — it does not
        // collapse down to just the grabbed anchor.
        assert_eq!(engine.selection, [a, b].into_iter().collect());
        assert_eq!(engine.selected, Some(a), "the physically-grabbed anchor becomes the facts-panel value");
    }

    /// Dragging a node OUTSIDE the current selection drags just it (not
    /// the rest of the pre-existing selection) and Replace-selects it on
    /// release.
    #[test]
    fn dragging_a_non_selected_node_drags_just_it_and_replace_selects_it() {
        let (mut engine, [a, b, c, _d]) = four_corner_square_engine();
        engine.apply_selection([a, b], SelectMode::Replace);

        engine.on_event(&PlatformEvent::PointerDown { x: 0.0, y: 100.0, button: MouseButton::Left }); // c's position
        engine.on_event(&PlatformEvent::PointerMoved { x: 50.0, y: 150.0 });
        engine.on_event(&PlatformEvent::PointerUp { x: 50.0, y: 150.0, button: MouseButton::Left });
        engine.tick(1.0 / 60.0);

        assert_eq!(engine.selection, [c].into_iter().collect(), "selection replaces to just the dragged node");
        assert_eq!(engine.selected, Some(c));

        assert!(engine.particles[a.index()].fx.is_none(), "a (previously selected) must never have been pinned by this gesture");
        assert!(engine.particles[b.index()].fx.is_none(), "b (previously selected) must never have been pinned by this gesture");
        assert!(engine.is_pinned(c));
        let cp = engine.particles[c.index()];
        assert!((cp.x - 50.0).abs() < 1e-4);
        assert!((cp.y - 150.0).abs() < 1e-4);
    }

    /// Wave 2.4 modifier-tracking gate: Shift held at mousedown starts a
    /// box-select drag (camera pan suppressed) — clearing the modifier
    /// afterward lets a plain drag pan again, unaffected.
    #[test]
    fn shift_held_starts_a_box_select_drag_instead_of_panning_the_camera() {
        let mut engine = empty_engine_with_canvas(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.on_event(&PlatformEvent::ModifiersChanged { modifiers: ModifierKeys::shift() });

        let before_pan = (engine.camera.pan_x, engine.camera.pan_y);
        engine.on_event(&PlatformEvent::PointerDown { x: 100.0, y: 100.0, button: MouseButton::Left });
        assert!(engine.box_select_rect().is_some(), "Shift+mousedown on empty background must start a box-select, not a pan");

        engine.on_event(&PlatformEvent::PointerMoved { x: 250.0, y: 220.0 });
        assert_eq!((engine.camera.pan_x, engine.camera.pan_y), before_pan, "camera must not pan while box-selecting");
        let rect = engine.box_select_rect().expect("still box-selecting mid-drag");
        assert!((rect.x - 100.0).abs() < 1e-9);
        assert!((rect.y - 100.0).abs() < 1e-9);
        assert!((rect.width - 150.0).abs() < 1e-9);
        assert!((rect.height - 120.0).abs() < 1e-9);

        engine.on_event(&PlatformEvent::PointerUp { x: 250.0, y: 220.0, button: MouseButton::Left });
        assert!(engine.box_select_rect().is_none(), "the rubber-band rect clears once the drag ends");
        assert_eq!((engine.camera.pan_x, engine.camera.pan_y), before_pan, "camera must still not have panned");

        // Clearing the modifier lets a plain drag pan again, unaffected.
        engine.on_event(&PlatformEvent::ModifiersChanged { modifiers: ModifierKeys::none() });
        engine.on_event(&PlatformEvent::PointerDown { x: 300.0, y: 300.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 310.0, y: 300.0 });
        assert!((engine.camera.pan_x - (before_pan.0 + 10.0)).abs() < 1e-9, "plain drag (no modifier) still pans the camera");
    }

    /// `collapse_selection` round-trip: define+collapse a cluster from
    /// the CURRENT selection, then expand restores the exact pre-collapse
    /// positions (the underlying `ClusterRegistry` round-trip is already
    /// proven in `cluster.rs` — this proves the NEW selection-driven
    /// entry point wires into it correctly).
    #[test]
    fn collapse_selection_round_trip_via_expand() {
        let (mut engine, [a, b, c, d]) = four_corner_square_engine();
        engine.apply_selection([b, d], SelectMode::Replace);
        assert!(engine.selection_collapsed_group().is_none(), "nothing collapsed yet");

        let b0 = engine.particles[b.index()];
        let d0 = engine.particles[d.index()];

        let id = engine.collapse_selection().expect("a non-empty selection collapses");
        assert!(engine.is_collapsed(id));
        assert_eq!(engine.selection_collapsed_group(), Some(id));

        // `b` (lowest `NodeIndex` in the selection) is the collapse
        // representative and stays visible; `d` is hidden.
        engine.refresh_visible();
        assert!(engine.visible_nodes().contains(&b));
        assert!(!engine.visible_nodes().contains(&d));
        assert!(engine.visible_nodes().contains(&a), "untouched nodes stay visible");
        assert!(engine.visible_nodes().contains(&c));

        assert!(engine.expand_cluster(id));
        assert!(engine.selection_collapsed_group().is_none(), "collapsed_group clears once expanded");

        let b1 = engine.particles[b.index()];
        let d1 = engine.particles[d.index()];
        assert_eq!((b1.x, b1.y), (b0.x, b0.y), "expand restores the EXACT pre-collapse position");
        assert_eq!((d1.x, d1.y), (d0.x, d0.y));
    }

    /// `clear_selection`/`pin_selection`/`unpin_selection` group ops.
    #[test]
    fn pin_selection_and_unpin_selection_apply_to_every_member() {
        let (mut engine, [a, b, _c, _d]) = four_corner_square_engine();
        engine.apply_selection([a, b], SelectMode::Replace);
        assert!(!engine.is_pinned(a) && !engine.is_pinned(b));

        engine.pin_selection();
        assert!(engine.is_pinned(a));
        assert!(engine.is_pinned(b));

        engine.unpin_selection();
        assert!(!engine.is_pinned(a));
        assert!(!engine.is_pinned(b));

        engine.clear_selection();
        assert!(engine.selection.is_empty());
        assert!(engine.selected.is_none());
    }

    /// Wave 2.4 generalization of the W2.2 precedence rule: a
    /// MULTI-selection (not just a single `selected`) also wins over a
    /// concurrent hover, and clearing it resumes hover-driven focus —
    /// added alongside (not replacing) the existing single-selection
    /// precedence test, which already covers the single-node case
    /// unchanged. Uses two DISJOINT edges (not the corner-square ring
    /// fixture) so a selection over one pair's neighborhood can never
    /// accidentally overlap a hover on the other, unrelated pair — the
    /// square's own ring connectivity would make every node reachable
    /// from any 2-node selection at depth 1, masking exactly the
    /// precedence behavior this test exists to prove.
    #[test]
    fn multi_selection_also_takes_precedence_over_a_concurrent_hover_and_resumes_on_clear() {
        let mut graph = Graph::new();
        let p1 = graph.push_node((), "p1", "x", 4.0);
        let p2 = graph.push_node((), "p2", "x", 4.0);
        let q1 = graph.push_node((), "q1", "x", 4.0);
        let q2 = graph.push_node((), "q2", "x", 4.0);
        graph.push_edge(p1, p2, 1.0, ());
        graph.push_edge(q1, q2, 1.0, ());
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (100.0, 0.0), (500.0, 0.0), (600.0, 0.0)]);
        engine.refresh_visible();

        engine.apply_selection([p1, p2], SelectMode::Replace);
        assert!(engine.focus.is_selected(u64::from(p1)));
        assert!(engine.focus.is_selected(u64::from(p2)));

        engine.on_event(&PlatformEvent::PointerMoved { x: 500.0, y: 0.0 }); // hover q1, unrelated to the selection
        assert_eq!(engine.hovered(), Some(q1));
        assert!(engine.focus.is_selected(u64::from(p1)), "multi-selection must win over a concurrent hover");
        assert!(engine.focus.is_selected(u64::from(p2)));
        assert!(!engine.focus.is_selected(u64::from(q1)), "hover must not override an active multi-selection");
        assert!(!engine.focus.is_selected(u64::from(q2)));

        engine.clear_selection();
        // The pointer never left `q1` — hover resumes driving the focus
        // set the instant the multi-selection is no longer overriding it.
        assert!(engine.focus.is_selected(u64::from(q1)), "clearing a multi-selection must resume hover-driven focus");
        assert!(engine.focus.is_selected(u64::from(q2)));
        assert!(!engine.focus.is_selected(u64::from(p1)));
        assert!(!engine.focus.is_selected(u64::from(p2)));
    }

    // ── W2.5 navigation (animated transition + keyboard hold-to-repeat) ────

    /// `zoom_to_node` over simulated ticks converges to the exact target
    /// zoom/pan, with the distance-to-target shrinking monotonically every
    /// tick (never overshoots then backtracks) — the literal easing gate.
    /// The node is pinned so the physics tick can't itself perturb the
    /// position `zoom_to_node` centered on (the transition and the sim
    /// are independent concerns; pinning isolates the camera-only math
    /// this test is actually about).
    #[test]
    fn zoom_to_node_transition_converges_to_the_target_with_monotonic_easing() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        engine.set_canvas_rect(canvas);
        engine.seed_positions(&[(120.0, 40.0)]);
        engine.pin_node(a);

        let target_zoom = 3.0;
        let target_pan = (canvas.width / 2.0 - 120.0 * target_zoom, canvas.height / 2.0 - 40.0 * target_zoom);

        assert!(engine.zoom_to_node(a, 500.0, Some(target_zoom)));
        assert!(engine.camera_transitioning());

        let mut prev_zoom_dist = f64::MAX;
        let mut prev_pan_dist = f64::MAX;
        for _ in 0..40 {
            engine.tick(1.0 / 60.0);
            let zoom_dist = (engine.camera.zoom - target_zoom).abs();
            let pan_dist =
                ((engine.camera.pan_x - target_pan.0).powi(2) + (engine.camera.pan_y - target_pan.1).powi(2)).sqrt();
            assert!(zoom_dist <= prev_zoom_dist + 1e-9, "zoom must converge monotonically: {zoom_dist} > {prev_zoom_dist}");
            assert!(pan_dist <= prev_pan_dist + 1e-9, "pan must converge monotonically: {pan_dist} > {prev_pan_dist}");
            prev_zoom_dist = zoom_dist;
            prev_pan_dist = pan_dist;
        }

        assert!(!engine.camera_transitioning(), "a 500ms transition at 60fps must finish within 40 ticks");
        assert!((engine.camera.zoom - target_zoom).abs() < 1e-6);
        let (sx, sy) = engine.camera.world_to_screen((120.0, 40.0), canvas);
        assert!((sx - canvas.width / 2.0).abs() < 1e-6, "the node must land exactly centered once the transition completes");
        assert!((sy - canvas.height / 2.0).abs() < 1e-6);
    }

    /// A held nav key keeps the render loop hot and pans a fixed amount
    /// every tick for as long as it's held; releasing it lets the loop go
    /// idle again and pan stops changing.
    #[test]
    fn keydown_arrow_pans_the_camera_every_tick_while_held_and_stops_on_keyup() {
        let mut engine = empty_engine_with_canvas(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.tick(1.0 / 60.0); // settle the (empty) sim so `is_hot` starts false
        assert!(!engine.is_hot(), "an idle graph with nothing held must not be hot");

        assert!(engine.on_event(&PlatformEvent::KeyDown { key: KeyCode::ArrowRight, modifiers: ModifierKeys::none() }));
        assert!(engine.is_hot(), "a held nav key alone must keep the render loop hot");

        let mut prev_pan_x = engine.camera.pan_x;
        for _ in 0..5 {
            engine.tick(1.0 / 60.0);
            assert!(engine.camera.pan_x > prev_pan_x, "each tick while ArrowRight is held must pan further right");
            prev_pan_x = engine.camera.pan_x;
        }

        assert!(engine.on_event(&PlatformEvent::KeyUp { key: KeyCode::ArrowRight, modifiers: ModifierKeys::none() }));
        assert!(!engine.is_hot(), "releasing the only held nav key must let the loop go idle again");
        let pan_after_release = engine.camera.pan_x;
        engine.tick(1.0 / 60.0);
        assert_eq!(engine.camera.pan_x, pan_after_release, "pan must stop changing once the key is released");
    }

    // ── W2.6 local subgraph + filter ────────────────────────────────────────

    /// `set_local_root` restricts the visible/pick set to EXACTLY the BFS
    /// depth-k neighborhood of the root; `set_local_root(None, _)` restores
    /// the full node set.
    #[test]
    fn set_local_root_restricts_the_visible_set_to_exactly_the_bfs_depth_k_neighborhood_and_restores_on_clear() {
        let (mut engine, [a, b, c, d]) = chain4_engine_on_a_line();
        engine.refresh_visible();
        assert_eq!(engine.visible_nodes().len(), 4, "sanity: all 4 nodes visible with no restriction");

        engine.set_local_root(Some(b), Some(1));
        engine.refresh_visible();
        let visible: HashSet<NodeIndex> = engine.visible_nodes().iter().copied().collect();
        assert_eq!(visible, [a, b, c].into_iter().collect(), "depth-1 from b in a 4-chain is exactly {{a,b,c}}");
        assert_eq!(engine.local_root(), Some((b, 1)));

        engine.set_local_root(None, None);
        engine.refresh_visible();
        let visible_full: HashSet<NodeIndex> = engine.visible_nodes().iter().copied().collect();
        assert_eq!(visible_full, [a, b, c, d].into_iter().collect(), "clearing local_root restores the full node set");
        assert!(engine.local_root().is_none());
    }

    /// `set_local_root` with a default (omitted) depth uses depth 2.
    #[test]
    fn set_local_root_default_depth_is_two() {
        let (mut engine, [a, b, _c, d]) = chain4_engine_on_a_line();
        engine.set_local_root(Some(a), None);
        assert_eq!(engine.local_root(), Some((a, 2)));
        engine.refresh_visible();
        let visible: HashSet<NodeIndex> = engine.visible_nodes().iter().copied().collect();
        assert!(!visible.contains(&d), "d is 3 hops from a — outside a depth-2 neighborhood");
        assert!(visible.contains(&b));
    }

    /// `a - b - c` chain where `b` is given a DIFFERENT category from `a`/
    /// `c` — the fixture the filter tests exclude `b` with.
    fn three_node_chain_with_a_hideable_middle() -> (Graph<(), ()>, NodeIndex, NodeIndex, NodeIndex) {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let b = graph.push_node((), "b", "hidden", 4.0);
        let c = graph.push_node((), "c", "x", 4.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        (graph, a, b, c)
    }

    /// A filtered-out node is excluded from render/pick (the visible set)
    /// AND from the force topology fed to the layout: dropping its edges
    /// measurably changes where the SURVIVING nodes settle, compared to an
    /// otherwise-identical unfiltered run — the literal "settled positions
    /// differ" gate.
    #[test]
    fn filter_removes_a_node_from_render_and_from_the_force_topology_so_settled_positions_differ() {
        let settle = |engine: &mut TestEngine| {
            for _ in 0..400 {
                engine.tick(1.0 / 60.0);
            }
        };

        let (graph, a, _b, _c) = three_node_chain_with_a_hideable_middle();
        let mut baseline: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        baseline.set_canvas_rect(Rect::new(-400.0, -300.0, 800.0, 600.0));
        baseline.seed_positions(&[(-40.0, 0.0), (0.0, 0.0), (40.0, 0.0)]);
        settle(&mut baseline);
        let baseline_a = baseline.particles[a.index()];

        let (graph2, a2, b2, c2) = three_node_chain_with_a_hideable_middle();
        let mut filtered: TestEngine = GraphEngine::new(graph2, ForceDirectedLayout::default());
        filtered.set_canvas_rect(Rect::new(-400.0, -300.0, 800.0, 600.0));
        filtered.seed_positions(&[(-40.0, 0.0), (0.0, 0.0), (40.0, 0.0)]);
        filtered.set_filter(Some(FilterSpec { categories: Some(vec!["x".to_owned()]), ..Default::default() }));

        filtered.refresh_visible();
        assert!(!filtered.visible_nodes().contains(&b2), "the filtered-out node must be excluded from render/pick");
        assert!(filtered.visible_nodes().contains(&a2), "surviving nodes stay visible");
        assert!(filtered.visible_nodes().contains(&c2));

        settle(&mut filtered);
        let filtered_a = filtered.particles[a2.index()];

        let dist = ((baseline_a.x - filtered_a.x).powi(2) + (baseline_a.y - filtered_a.y).powi(2)).sqrt();
        assert!(
            dist > 1.0,
            "losing the link-force pull toward the filtered-out node must measurably change where `a` settles: \
             baseline ({}, {}) vs filtered ({}, {}), dist {dist}",
            baseline_a.x,
            baseline_a.y,
            filtered_a.x,
            filtered_a.y
        );
    }

    /// `set_filter(None)` clears a previously-set filter and restores the
    /// full visible set.
    #[test]
    fn set_filter_none_clears_a_previous_filter() {
        let (graph, _a, b, _c) = three_node_chain_with_a_hideable_middle();
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(-40.0, 0.0), (0.0, 0.0), (40.0, 0.0)]);

        engine.set_filter(Some(FilterSpec { categories: Some(vec!["x".to_owned()]), ..Default::default() }));
        engine.refresh_visible();
        assert!(!engine.visible_nodes().contains(&b));

        engine.set_filter(None);
        assert!(engine.filter().is_none());
        engine.refresh_visible();
        assert!(engine.visible_nodes().contains(&b), "clearing the filter must restore the excluded node");
    }

    /// Star graph — root + 4 leaves, alternating categories `"keep"`/
    /// `"drop"` — the fixture the filter+local-mode composition test uses.
    fn star_graph_with_categories() -> (Graph<(), ()>, NodeIndex, [NodeIndex; 4]) {
        let mut graph = Graph::new();
        let root = graph.push_node((), "root", "keep", 4.0);
        let mut leaves = Vec::with_capacity(4);
        for i in 0..4 {
            let category = if i % 2 == 0 { "keep" } else { "drop" };
            let leaf = graph.push_node((), format!("leaf{i}"), category, 4.0);
            graph.push_edge(root, leaf, 1.0, ());
            leaves.push(leaf);
        }
        (graph, root, [leaves[0], leaves[1], leaves[2], leaves[3]])
    }

    /// Filter and local-subgraph mode compose as an INTERSECTION: a node
    /// must pass both to stay visible.
    #[test]
    fn filter_and_local_mode_compose_as_an_intersection() {
        let (graph, root, [l0, l1, l2, l3]) = star_graph_with_categories();
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.set_canvas_rect(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.seed_positions(&[(0.0, 0.0), (50.0, 0.0), (0.0, 50.0), (-50.0, 0.0), (0.0, -50.0)]);

        engine.set_local_root(Some(root), Some(1));
        engine.set_filter(Some(FilterSpec { categories: Some(vec!["keep".to_owned()]), ..Default::default() }));
        engine.refresh_visible();

        let visible: HashSet<NodeIndex> = engine.visible_nodes().iter().copied().collect();
        // BFS depth-1 from root = {root, l0, l1, l2, l3}; filter keeps only
        // category "keep" = {root, l0, l2}. Intersection = {root, l0, l2}.
        assert_eq!(visible, [root, l0, l2].into_iter().collect());
        assert!(!visible.contains(&l1), "l1 fails the filter even though it's in the local BFS set");
        assert!(!visible.contains(&l3));
    }

    // ── Graph-strengthening arc G1.4: lost PointerUp ────────────────────
    //
    // 2D quality audit A2: `uzor-window-desktop` has no pointer-capture
    // call anywhere, so a `PointerUp` after the cursor leaves the window
    // can simply never arrive. `PlatformEvent::PointerLeft`/
    // `WindowFocused(false)` must finalize whatever gesture is in progress
    // exactly as a real `PointerUp` at the last known pointer position
    // would.

    #[test]
    fn pointer_left_finalizes_a_background_pan_exactly_like_a_pointer_up_would() {
        let mut engine = empty_engine_with_canvas(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.on_event(&PlatformEvent::PointerDown { x: 300.0, y: 300.0, button: MouseButton::Left });
        engine.on_event(&PlatformEvent::PointerMoved { x: 340.0, y: 300.0 });
        assert!(matches!(engine.mode, PointerMode::PanningCamera { .. }), "fixture sanity: must genuinely be panning");

        assert!(engine.on_event(&PlatformEvent::PointerLeft));

        assert!(matches!(engine.mode, PointerMode::Idle), "PointerLeft must finalize the in-progress pan, same as a real PointerUp");
    }

    #[test]
    fn window_defocus_finalizes_an_in_progress_node_drag_and_leaves_it_sticky_pinned() {
        let (mut engine, a, _b) = two_node_chain_engine();
        engine.on_event(&PlatformEvent::PointerDown { x: 100.0, y: 100.0, button: MouseButton::Left });
        assert!(matches!(engine.mode, PointerMode::DraggingNode), "fixture sanity: must genuinely be dragging node a");
        assert!(engine.layout.alpha_target() > 0.0, "a drag must hold the sustained alpha target while in progress");

        assert!(engine.on_event(&PlatformEvent::WindowFocused(false)));

        assert!(matches!(engine.mode, PointerMode::Idle), "losing window focus must finalize the in-progress node drag");
        assert_eq!(engine.selected, Some(a), "a node drag must select the dragged node on finalize, same as a real PointerUp");
        assert!(engine.is_pinned(a), "the default Sticky drag-end policy must leave the node pinned where the drag left it");
        assert_eq!(engine.layout.alpha_target(), 0.0, "drag-end must clear the sustained alpha target regardless of how the drag ended");
    }

    #[test]
    fn window_refocus_true_is_not_a_gesture_cancel() {
        let mut engine = empty_engine_with_canvas(Rect::new(0.0, 0.0, 800.0, 600.0));
        engine.on_event(&PlatformEvent::PointerDown { x: 300.0, y: 300.0, button: MouseButton::Left });
        assert!(matches!(engine.mode, PointerMode::PanningCamera { .. }));

        assert!(!engine.on_event(&PlatformEvent::WindowFocused(true)), "gaining focus is not a drag-cancel and must not be consumed as one");

        assert!(matches!(engine.mode, PointerMode::PanningCamera { .. }), "gaining focus must not disturb an in-progress pan");
    }

    #[test]
    fn pointer_left_with_nothing_in_progress_is_a_no_op() {
        let mut engine = empty_engine_with_canvas(Rect::new(0.0, 0.0, 800.0, 600.0));
        assert!(matches!(engine.mode, PointerMode::Idle));

        assert!(!engine.on_event(&PlatformEvent::PointerLeft), "PointerLeft with no gesture in progress must not be reported as consumed");
    }

    // ── Graph-strengthening arc G1.5: camera input vs. an in-flight
    // programmatic camera transition ─────────────────────────────────────

    #[test]
    fn a_background_pan_started_mid_camera_transition_clears_the_transition_immediately() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        engine.set_canvas_rect(canvas);
        engine.seed_positions(&[(120.0, 40.0)]);
        engine.pin_node(a);
        assert!(engine.zoom_to_node(a, 500.0, Some(3.0)));
        assert!(engine.camera_transitioning());

        // A background point, far from the seeded node, so this starts a
        // pan rather than a node drag.
        engine.on_event(&PlatformEvent::PointerDown { x: 5.0, y: 5.0, button: MouseButton::Left });

        assert!(!engine.camera_transitioning(), "starting a background pan must clear an in-flight camera transition");
    }

    #[test]
    fn a_scroll_mid_camera_transition_clears_the_transition_immediately() {
        let mut graph = Graph::new();
        let a = graph.push_node((), "a", "x", 4.0);
        let mut engine: TestEngine = GraphEngine::new(graph, ForceDirectedLayout::default());
        let canvas = Rect::new(0.0, 0.0, 800.0, 600.0);
        engine.set_canvas_rect(canvas);
        engine.seed_positions(&[(120.0, 40.0)]);
        engine.pin_node(a);
        engine.on_event(&PlatformEvent::PointerMoved { x: 400.0, y: 300.0 });
        assert!(engine.zoom_to_node(a, 500.0, Some(3.0)));
        assert!(engine.camera_transitioning());

        engine.on_event(&PlatformEvent::Scroll { dx: 0.0, dy: -1.0 });

        assert!(!engine.camera_transitioning(), "a wheel-zoom must clear an in-flight camera transition");
    }
}
