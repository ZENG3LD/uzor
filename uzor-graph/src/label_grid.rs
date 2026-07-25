//! Sigma.js `LabelGrid` port (Wave 2.3 — `docs/uzor-engines/uzor_graph_interaction_wave2_spec.md`
//! §W2.3, `research_graph_interaction_oss_2026.md` §3/§label-lod): bucket
//! label candidates into fixed-size screen-space cells, rank each cell's
//! candidates by importance, and reveal only the top-`quota` per cell —
//! `crate::render::draw_nodes` (the only caller) builds the candidate
//! list from `Graph`/`Camera2D`/`DrawContext` and hands it to
//! [`select_labels`] here. This module itself stays generic-free (no
//! `Graph<N, E>` dependency) — pure grid/quota/alpha math only, fully
//! unit-testable without a graph or a `RenderContext`.
//!
//! **Quota formula, sign-convention note**: sigma's `ratio` is SMALLER
//! when more zoomed in (`getLabelsToDisplay`: `scaledCellArea = cellArea
//! / ratio²`, quota = `ceil(density * scaledCellArea / cellArea)` =
//! `ceil(density / ratio²)`). This engine's `Camera2D::zoom` is the
//! opposite convention — LARGER when more zoomed in (matches every other
//! `Camera2D`/`pick` calculation already in this crate, which all
//! multiply by `zoom`, never divide). The physically-identical
//! relationship ("a fixed screen-space cell covers less graph-space area
//! as you zoom in, so it should reveal quadratically more of the labels
//! living in that patch") is therefore expressed here as
//! `ceil(density * zoom²)` — multiplying, not dividing, by the square.
//! At `zoom == 1.0` (this engine's own zoom-identity default, see
//! `Camera2D::default`) `quota == ceil(density)`, matching sigma's own
//! "`labelDensity` is literally how many labels per cell at 1:1 zoom"
//! framing exactly.
//!
//! **Ranking diverges from sigma deliberately**: sigma sorts each cell's
//! candidates by rendered node SIZE only. This port sorts by node
//! DEGREE first (this engine's own notion of "importance" for a
//! node-link graph — the wave-2 spec's explicit choice), then screen
//! radius, then `NodeIndex` as the final, fully deterministic tie-break
//! (sigma's own comparator source comment explicitly documents "cannot
//! return 0" for the identical reason — some deterministic tie-break is
//! mandatory, ties are not allowed to depend on `HashMap`/sort
//! instability).
//!
//! **Forced labels** (collapsed-cluster representatives passed in by
//! `render::draw_nodes`; hover/selection-neighbor forcing is handled one
//! layer up, in `render::draw_nodes`/`labels_to_draw` itself, since that
//! needs `FocusSet` — see that function's doc comment) are unioned in
//! AFTER every cell's own top-`quota` selection, exactly like sigma's
//! `nodesWithForcedLabels` union.
//!
//! **Screen-space culling** (sigma's own source comment, quoted in the
//! research doc: "we used to rely on the quadtree for this... brute-force
//! screen-space bounds checking is a good performance compromise") is
//! already provided upstream, before a node ever becomes a label
//! candidate here, by [`crate::render::cull_visible`]'s world-space AABB
//! pass (with margin) — no second cull is implemented in this module.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use uzor::types::Rect;

use crate::graph::NodeIndex;

/// Sigma's default (`labelGridCellSize`, `packages/sigma/src/settings.ts`).
pub const LABEL_GRID_CELL_SIZE_PX: f64 = 100.0;

/// Sigma's default (`labelDensity`) — "how many labels per grid cell at
/// 1:1 zoom" (research doc §3, point 4).
pub const DEFAULT_LABEL_DENSITY: f64 = 1.0;

/// Zoom at/above which node labels start fading in for a degree-0 leaf
/// (Obsidian's "Text Fade Threshold" precedent — a continuous dial keyed
/// to zoom, not a binary show/hide). Moved here from `render.rs` (Wave
/// 2.3) so every label-LOD constant lives in one module; `render.rs`
/// re-exports both for its own internal use.
pub const LOD_LABEL_FADE_LOW: f64 = 0.45;
pub const LOD_LABEL_FADE_HIGH: f64 = 0.9;

/// How far (in zoom units) the maximum-degree node in the graph shifts
/// the zoom-fade window down, relative to a degree-0 leaf — see
/// [`label_alpha`]'s doc comment for the full rationale.
const DEGREE_ALPHA_SHIFT: f64 = 0.3;

/// Every label-LOD tuning constant this module owns, bundled into one
/// caller-configurable struct (graph-strengthening arc Wave G2 — the 2D
/// audit's own configurability inventory flagged all four as `✗`, no
/// override anywhere). [`Default`] reproduces the pre-existing module
/// constants byte-identically — see
/// `select_labels_with_the_default_lod_config_matches_the_pre_existing_constants`.
///
/// `crate::engine::GraphEngine` owns one `label_lod: LabelLodConfig` field
/// with a `label_lod()`/`set_label_lod()` accessor pair, the same
/// established shape as [`crate::engine::GraphEngine::label_halo`]/
/// `set_label_halo`. `crate::render::DrawContext::label_lod` threads it
/// into [`select_labels`]/[`label_alpha`] every frame;
/// `crate::engine3d::GraphEngine3D` (out of this 2D-only wave's scope)
/// keeps calling both functions with `LabelLodConfig::default()`, so its
/// own label-LOD behavior is unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelLodConfig {
    /// Grid cell edge length, in screen px — was
    /// [`LABEL_GRID_CELL_SIZE_PX`].
    pub grid_cell_size_px: f64,
    /// Zoom at/above which a degree-0 leaf's label starts fading in — was
    /// [`LOD_LABEL_FADE_LOW`].
    pub fade_low: f64,
    /// Zoom at/above which a degree-0 leaf's label is fully opaque — was
    /// [`LOD_LABEL_FADE_HIGH`].
    pub fade_high: f64,
    /// How far (zoom units) the maximum-degree node's fade window shifts
    /// down relative to a leaf's — was the private `DEGREE_ALPHA_SHIFT`.
    pub degree_alpha_shift: f64,
}

impl Default for LabelLodConfig {
    fn default() -> Self {
        Self {
            grid_cell_size_px: LABEL_GRID_CELL_SIZE_PX,
            fade_low: LOD_LABEL_FADE_LOW,
            fade_high: LOD_LABEL_FADE_HIGH,
            degree_alpha_shift: DEGREE_ALPHA_SHIFT,
        }
    }
}

/// One label candidate for the grid pass. `screen_pos`/`screen_radius`
/// are already camera-projected (the SAME `Camera2D::world_to_screen`/
/// `node_screen_radius` helpers every other draw/pick path in this crate
/// uses — one source of truth, per the engine design doc §4.2/§0.3).
#[derive(Debug, Clone, Copy)]
pub struct LabelCandidate {
    pub node: NodeIndex,
    pub screen_pos: (f64, f64),
    pub degree: u32,
    pub screen_radius: f64,
}

fn cell_index(pos: (f64, f64), viewport: Rect, cols: u32, cell_size_px: f64) -> u32 {
    let local_x = (pos.0 - viewport.x).max(0.0);
    let local_y = (pos.1 - viewport.y).max(0.0);
    let col = (local_x / cell_size_px).floor() as u32;
    let row = (local_y / cell_size_px).floor() as u32;
    row * cols + col
}

/// `ceil(density * zoom²)` — see module doc for the sign-convention note
/// against sigma's own `ceil(density / ratio²)`. `density <= 0` yields an
/// empty per-cell quota (forced labels still show — the union in
/// [`select_labels`] is unconditional).
fn quota_per_cell(zoom: f64, density: f64) -> usize {
    if density <= 0.0 {
        return 0;
    }
    let zoom = zoom.max(1e-9);
    (density * zoom * zoom).ceil().max(0.0) as usize
}

/// Candidate importance ordering — degree descending, then screen radius
/// descending, then `NodeIndex` ascending (the final, always-decisive
/// tie-break; ties never reach it in practice for distinct node indices,
/// but the comparator itself must be total either way).
fn cmp_by_importance(a: &LabelCandidate, b: &LabelCandidate) -> Ordering {
    b.degree
        .cmp(&a.degree)
        .then_with(|| b.screen_radius.partial_cmp(&a.screen_radius).unwrap_or(Ordering::Equal))
        .then_with(|| a.node.0.cmp(&b.node.0))
}

/// The sigma `LabelGrid` selection pass. `viewport` must have positive
/// width/height — a degenerate viewport yields just `forced` (nothing
/// else can be meaningfully gridded). Deterministic: candidate input
/// order never affects the result (every cell re-sorts its own bucket
/// before taking the top-`quota` slice; `HashMap` bucketing order is
/// irrelevant since the final `shown` set only ever grows via `extend`,
/// never depends on cell processing order). `lod.grid_cell_size_px` is
/// floored at `1.0` (a non-positive/degenerate override would otherwise
/// divide by zero or manufacture an absurd cell count).
pub fn select_labels(
    candidates: &[LabelCandidate],
    viewport: Rect,
    zoom: f64,
    density: f64,
    forced: &HashSet<NodeIndex>,
    lod: &LabelLodConfig,
) -> HashSet<NodeIndex> {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return forced.clone();
    }
    let cell_size_px = lod.grid_cell_size_px.max(1.0);
    let cols = ((viewport.width / cell_size_px).ceil() as u32).max(1);

    let mut cells: HashMap<u32, Vec<&LabelCandidate>> = HashMap::new();
    for c in candidates {
        cells.entry(cell_index(c.screen_pos, viewport, cols, cell_size_px)).or_default().push(c);
    }

    let quota = quota_per_cell(zoom, density);
    let mut shown: HashSet<NodeIndex> = HashSet::new();
    for bucket in cells.values_mut() {
        bucket.sort_by(|a, b| cmp_by_importance(a, b));
        shown.extend(bucket.iter().take(quota).map(|c| c.node));
    }

    shown.extend(forced.iter().copied());
    shown
}

/// Label alpha as `f(zoom, normalized_degree)` — the wave-2 differentiator
/// (obsidian research doc §8/finding: Obsidian's own "Text Fade
/// Threshold" is zoom-only; users have open feature requests asking for
/// high-degree/important nodes to keep labels legible further out,
/// confirmed never shipped — built here from day one).
/// `normalized_degree` is `degree / max_degree_in_graph`, clamped to
/// `[0, 1]` by the caller's own computation (this function clamps again
/// defensively).
///
/// Implementation: slide BOTH [`LOD_LABEL_FADE_LOW`]/[`LOD_LABEL_FADE_HIGH`]
/// down by `normalized_degree * DEGREE_ALPHA_SHIFT` zoom units, keeping
/// the window's WIDTH constant. A degree-0 leaf uses the unshifted
/// window exactly (byte-identical to the pre-Wave-2.3 zoom-only fade
/// formula); the maximum-degree node's window starts
/// `DEGREE_ALPHA_SHIFT` zoom units earlier — its label both fades IN at
/// a lower zoom ("appears earlier", per the wave-2 spec's own wording)
/// and only reaches zero opacity at a correspondingly lower zoom on the
/// way back out ("fades later" when zooming away). A pure function,
/// monotonically non-decreasing in both `zoom` and `normalized_degree`.
/// `lod`'s fields replace what used to be the module constants
/// [`LOD_LABEL_FADE_LOW`]/[`LOD_LABEL_FADE_HIGH`]/`DEGREE_ALPHA_SHIFT` —
/// `LabelLodConfig::default()` reproduces them exactly.
pub fn label_alpha(zoom: f64, normalized_degree: f64, lod: &LabelLodConfig) -> f64 {
    let shift = normalized_degree.clamp(0.0, 1.0) * lod.degree_alpha_shift;
    let low = (lod.fade_low - shift).max(0.0);
    let high = (lod.fade_high - shift).max(low + 1e-6);
    ((zoom - low) / (high - low)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    fn candidate(idx: u32, screen_pos: (f64, f64), degree: u32, screen_radius: f64) -> LabelCandidate {
        LabelCandidate { node: NodeIndex(idx), screen_pos, degree, screen_radius }
    }

    /// A dense synthetic cluster — 40 candidates packed into a single
    /// 100px cell — at a LOW zoom (quota == 1): the grid pass must keep
    /// the label count both bounded by `cells * quota` AND strictly
    /// below the total candidate count (the whole point of the LOD
    /// pass — "the unreadable gray label cloud at 534 nodes" this wave
    /// closes).
    #[test]
    fn grid_quota_limits_labels_per_cell_below_total_node_count() {
        let candidates: Vec<LabelCandidate> = (0..40)
            .map(|i| candidate(i, (10.0 + i as f64, 10.0 + i as f64), i % 7, 4.0))
            .collect();
        let zoom = 0.1; // quota = ceil(1.0 * 0.1^2) = ceil(0.01) = 1
        let shown = select_labels(&candidates, viewport(), zoom, DEFAULT_LABEL_DENSITY, &HashSet::new(), &LabelLodConfig::default());

        assert!(shown.len() < candidates.len(), "LOD must cull most of a 40-candidate single-cell cluster: {} shown", shown.len());
        // All 40 candidates land in the same single cell (10..50 px in
        // both axes, well inside one 100px cell) — quota 1 means exactly
        // one label total.
        assert_eq!(shown.len(), 1);
    }

    /// Same crowded single cell, mixed degrees — the top-degree candidate
    /// must win the cell's one slot, never a lower-degree one (even a
    /// larger-radius lower-degree candidate placed first in the input
    /// order).
    #[test]
    fn highest_degree_candidate_wins_a_crowded_cells_single_slot() {
        let winner = candidate(3, (50.0, 50.0), 99, 4.0);
        let candidates = vec![
            candidate(0, (20.0, 20.0), 2, 20.0), // bigger radius, far lower degree
            candidate(1, (30.0, 30.0), 5, 4.0),
            winner,
            candidate(2, (40.0, 40.0), 10, 4.0),
        ];
        let shown = select_labels(&candidates, viewport(), 0.1, DEFAULT_LABEL_DENSITY, &HashSet::new(), &LabelLodConfig::default());
        assert_eq!(shown, HashSet::from([NodeIndex(3)]), "the degree-99 candidate must win the cell's single quota slot");
    }

    /// A node outside the grid pass's quota (e.g. the hover/selection
    /// path one layer up, or a collapsed-cluster representative) must
    /// still show its label — `forced` bypasses the quota unconditionally.
    #[test]
    fn forced_candidates_bypass_the_quota_entirely() {
        let candidates: Vec<LabelCandidate> =
            (0..20).map(|i| candidate(i, (10.0 + i as f64, 10.0), 1, 4.0)).collect();
        let forced_node = NodeIndex(15); // degree 1, would never win the crowded cell's slot on its own
        let forced: HashSet<NodeIndex> = HashSet::from([forced_node]);

        let shown = select_labels(&candidates, viewport(), 0.1, DEFAULT_LABEL_DENSITY, &forced, &LabelLodConfig::default());
        assert!(shown.contains(&forced_node), "a forced candidate must show regardless of the grid quota");
    }

    /// Identical inputs (including the exact same `candidates` slice and
    /// `forced` set) must always produce an identical output set — no
    /// `HashMap`-iteration-order leak into the result (Wave 2.3
    /// determinism gate).
    #[test]
    fn identical_candidate_sets_produce_identical_label_sets() {
        let candidates: Vec<LabelCandidate> = (0..60)
            .map(|i| candidate(i, ((i % 8) as f64 * 90.0 + 5.0, (i / 8) as f64 * 90.0 + 5.0), i % 11, 3.0 + (i % 5) as f64))
            .collect();
        let forced: HashSet<NodeIndex> = HashSet::from([NodeIndex(2), NodeIndex(41)]);

        let first = select_labels(&candidates, viewport(), 0.6, DEFAULT_LABEL_DENSITY, &forced, &LabelLodConfig::default());
        for _ in 0..5 {
            let again = select_labels(&candidates, viewport(), 0.6, DEFAULT_LABEL_DENSITY, &forced, &LabelLodConfig::default());
            assert_eq!(first, again, "identical inputs must yield an identical label set on every repeated call");
        }
    }

    /// Zooming IN must never shrink the shown label count for the exact
    /// same candidate set — the per-cell quota scales quadratically with
    /// zoom (this engine's sign convention, see module doc).
    #[test]
    fn higher_zoom_yields_a_larger_per_cell_quota_and_therefore_more_labels() {
        let candidates: Vec<LabelCandidate> =
            (0..30).map(|i| candidate(i, (10.0 + i as f64 * 2.0, 10.0), i, 4.0)).collect();

        let low_zoom_shown = select_labels(&candidates, viewport(), 0.1, DEFAULT_LABEL_DENSITY, &HashSet::new(), &LabelLodConfig::default());
        let high_zoom_shown = select_labels(&candidates, viewport(), 3.0, DEFAULT_LABEL_DENSITY, &HashSet::new(), &LabelLodConfig::default());

        assert!(
            high_zoom_shown.len() > low_zoom_shown.len(),
            "zooming in must reveal more labels: low-zoom {} vs high-zoom {}",
            low_zoom_shown.len(),
            high_zoom_shown.len()
        );
    }

    #[test]
    fn quota_per_cell_is_ceil_of_density_at_zoom_identity() {
        assert_eq!(quota_per_cell(1.0, 1.0), 1);
        assert_eq!(quota_per_cell(1.0, 2.5), 3);
        assert_eq!(quota_per_cell(1.0, 0.0), 0);
        assert_eq!(quota_per_cell(2.0, 1.0), 4); // ceil(1.0 * 2.0^2) = 4
    }

    /// The differentiator itself: a degree-0 leaf uses the plain
    /// zoom-only fade window; the maximum-degree node's label is both
    /// already partially visible at a zoom where a leaf's alpha is still
    /// exactly 0, AND still above 0 at a zoom where a leaf has already
    /// faded fully out.
    #[test]
    fn label_alpha_is_boosted_by_degree_appearing_earlier_and_fading_later() {
        let leaf_alpha_at_low_edge = label_alpha(LOD_LABEL_FADE_LOW, 0.0, &LabelLodConfig::default());
        let hub_alpha_at_low_edge = label_alpha(LOD_LABEL_FADE_LOW, 1.0, &LabelLodConfig::default());
        assert_eq!(leaf_alpha_at_low_edge, 0.0, "a degree-0 leaf must be exactly invisible at the unshifted low edge");
        assert!(hub_alpha_at_low_edge > 0.0, "a max-degree hub must already be partially visible at the leaf's low edge");

        let below_leaf_window = LOD_LABEL_FADE_LOW - 0.2;
        assert_eq!(label_alpha(below_leaf_window, 0.0, &LabelLodConfig::default()), 0.0, "a leaf below its own window is still exactly invisible");
        assert!(label_alpha(below_leaf_window, 1.0, &LabelLodConfig::default()) > 0.0, "a hub must stay partially visible below a leaf's fade-in zoom");

        // Monotonic in zoom for a fixed degree.
        assert!(label_alpha(LOD_LABEL_FADE_HIGH, 0.5, &LabelLodConfig::default()) >= label_alpha(LOD_LABEL_FADE_LOW, 0.5, &LabelLodConfig::default()));
        // Monotonic in degree for a fixed zoom sitting inside the shifted window.
        let mid_zoom = LOD_LABEL_FADE_LOW - 0.1;
        assert!(label_alpha(mid_zoom, 1.0, &LabelLodConfig::default()) >= label_alpha(mid_zoom, 0.5, &LabelLodConfig::default()));
        assert!(label_alpha(mid_zoom, 0.5, &LabelLodConfig::default()) >= label_alpha(mid_zoom, 0.0, &LabelLodConfig::default()));
    }

    // ── Graph-strengthening arc G2: `LabelLodConfig` configurability ───

    #[test]
    fn label_lod_config_default_matches_the_pre_existing_module_constants() {
        let lod = LabelLodConfig::default();
        assert_eq!(lod.grid_cell_size_px, LABEL_GRID_CELL_SIZE_PX);
        assert_eq!(lod.fade_low, LOD_LABEL_FADE_LOW);
        assert_eq!(lod.fade_high, LOD_LABEL_FADE_HIGH);
    }

    #[test]
    fn select_labels_with_the_default_lod_config_matches_the_pre_existing_constants() {
        // Same fixture as `grid_quota_limits_labels_per_cell_below_total_node_count`
        // — proves the new `lod` parameter's default reproduces the
        // pre-Wave-G2 hardcoded-constant behavior byte-for-byte.
        let candidates: Vec<LabelCandidate> = (0..40)
            .map(|i| candidate(i, (10.0 + i as f64, 10.0 + i as f64), i % 7, 4.0))
            .collect();
        let via_default = select_labels(&candidates, viewport(), 0.1, DEFAULT_LABEL_DENSITY, &HashSet::new(), &LabelLodConfig::default());
        assert_eq!(via_default.len(), 1);
    }

    /// A caller-supplied SMALLER grid cell must never DECREASE the shown
    /// label count for an unchanged candidate set — smaller cells mean
    /// more, smaller buckets, each still keeping its own `quota` (Wave G2:
    /// the grid cell size governs visible output and must be a real,
    /// caller-observable knob, not just a plumbed-through no-op).
    #[test]
    fn a_smaller_grid_cell_size_never_shows_fewer_labels_than_the_default() {
        let candidates: Vec<LabelCandidate> = (0..40)
            .map(|i| candidate(i, (10.0 + i as f64 * 4.0, 10.0), i % 7, 4.0))
            .collect();
        let default_shown = select_labels(&candidates, viewport(), 0.1, DEFAULT_LABEL_DENSITY, &HashSet::new(), &LabelLodConfig::default());
        let small_cell_lod = LabelLodConfig { grid_cell_size_px: 20.0, ..LabelLodConfig::default() };
        let small_cell_shown = select_labels(&candidates, viewport(), 0.1, DEFAULT_LABEL_DENSITY, &HashSet::new(), &small_cell_lod);
        assert!(
            small_cell_shown.len() >= default_shown.len(),
            "smaller grid cells must split the same candidates into more/smaller buckets, never fewer labels: default {} vs small-cell {}",
            default_shown.len(),
            small_cell_shown.len()
        );
        assert!(small_cell_shown.len() > default_shown.len(), "this fixture's candidates are spread wide enough that a 20px cell must actually split them into more buckets than the 100px default");
    }

    /// A caller-supplied fade window shifted entirely above the default's
    /// own must produce a STRICTLY lower alpha at the SAME zoom — proves
    /// `label_alpha` actually reads `lod`, not just accepts and ignores it.
    #[test]
    fn a_caller_supplied_fade_window_actually_changes_label_alpha() {
        let default_alpha = label_alpha(LOD_LABEL_FADE_LOW, 0.0, &LabelLodConfig::default());
        let shifted = LabelLodConfig { fade_low: LOD_LABEL_FADE_LOW + 0.3, fade_high: LOD_LABEL_FADE_HIGH + 0.3, ..LabelLodConfig::default() };
        let shifted_alpha = label_alpha(LOD_LABEL_FADE_LOW, 0.0, &shifted);
        assert_eq!(default_alpha, 0.0, "at the leaf's own unshifted low edge, alpha starts at exactly 0");
        assert_eq!(shifted_alpha, 0.0, "the SAME zoom is now even further below the shifted-up window, still exactly 0");
        // Move further into the shifted window's own low edge instead.
        let alpha_at_shifted_low_edge = label_alpha(shifted.fade_low, 0.0, &shifted);
        let default_alpha_at_same_zoom = label_alpha(shifted.fade_low, 0.0, &LabelLodConfig::default());
        assert_eq!(alpha_at_shifted_low_edge, 0.0, "the shifted window's own low edge is still exactly the fade-in threshold");
        assert!(default_alpha_at_same_zoom > 0.0, "at the SAME zoom, the unshifted default window is already past its own low edge");
    }
}
