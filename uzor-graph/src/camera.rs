//! Screen<->world camera transform for the graph canvas — pan + zoom.
//!
//! Ported and generalized from the case-specific MVP
//! (`dig2chain/dig2chain-graph-ui`, rejected — see the engine design
//! doc's root-cause section 0). The world<->screen round-trip and
//! zoom-at-cursor invariant were already correct there; this port adds
//! [`Camera2D::node_screen_radius`] — the single shared helper render
//! AND hit-test both call, so they can never independently drift. That
//! drift (render clamped effective zoom at a 0.25 floor, hit-test at a
//! *different* 0.05 floor) was the actual root cause of "clicking a node
//! opened no sidebar" in the rejected MVP, not a wiring bug.

use uzor::types::Rect;

/// Wave 2.5 (oss doc §2.6/obsidian doc §5.5) — de-facto community
/// default `scaleExtent([0.01, 1000])` (vasturiano `force-graph`), 5
/// orders of magnitude of zoom range. Was `[0.02, 12.0]` before Wave 2.5;
/// widening a clamp range only relaxes existing behavior (every prior
/// zoom value was already inside the new, wider bounds) so this is a
/// safe, additive change — see `uzor-graph/CLAUDE.md`'s divergence log.
pub const ZOOM_MIN: f64 = 0.01;
pub const ZOOM_MAX: f64 = 1000.0;

/// Final clamp range for a node's on-screen paint radius — applied once,
/// from the single shared helper, never from two independent constants.
pub const NODE_SCREEN_RADIUS_MIN: f64 = 1.5;
pub const NODE_SCREEN_RADIUS_MAX: f64 = 48.0;

#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    /// Screen-space translation of the world origin, in logical pixels.
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 }
    }
}

/// Axis-aligned bounding box in world space.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Aabb {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    /// Bounding box of a point cloud. `None` for an empty slice.
    pub fn from_points(points: &[(f64, f64)]) -> Option<Self> {
        let mut it = points.iter();
        let &(fx, fy) = it.next()?;
        let mut aabb = Aabb { min_x: fx, min_y: fy, max_x: fx, max_y: fy };
        for &(x, y) in it {
            aabb.min_x = aabb.min_x.min(x);
            aabb.min_y = aabb.min_y.min(y);
            aabb.max_x = aabb.max_x.max(x);
            aabb.max_y = aabb.max_y.max(y);
        }
        Some(aabb)
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

impl Camera2D {
    pub fn world_to_screen(&self, world: (f64, f64), viewport: Rect) -> (f64, f64) {
        (
            viewport.x + self.pan_x + world.0 * self.zoom,
            viewport.y + self.pan_y + world.1 * self.zoom,
        )
    }

    pub fn screen_to_world(&self, screen: (f64, f64), viewport: Rect) -> (f64, f64) {
        (
            (screen.0 - viewport.x - self.pan_x) / self.zoom,
            (screen.1 - viewport.y - self.pan_y) / self.zoom,
        )
    }

    /// World-space AABB currently covered by `viewport` — used to cull
    /// nodes/edges before issuing draw calls.
    pub fn visible_world_aabb(&self, viewport: Rect) -> Aabb {
        let top_left = self.screen_to_world((viewport.min_x(), viewport.min_y()), viewport);
        let bottom_right = self.screen_to_world((viewport.max_x(), viewport.max_y()), viewport);
        Aabb {
            min_x: top_left.0,
            min_y: top_left.1,
            max_x: bottom_right.0,
            max_y: bottom_right.1,
        }
    }

    /// Zoom around a fixed screen-space point (`cursor`) so the world
    /// point currently under the cursor stays under it after the zoom.
    pub fn zoom_at(&mut self, cursor: (f64, f64), viewport: Rect, factor: f64) {
        let world_under_cursor = self.screen_to_world(cursor, viewport);
        let new_zoom = (self.zoom * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        self.zoom = new_zoom;
        self.pan_x = cursor.0 - viewport.x - world_under_cursor.0 * new_zoom;
        self.pan_y = cursor.1 - viewport.y - world_under_cursor.1 * new_zoom;
    }

    /// Frame `aabb` (with a small margin) inside `viewport`. Backs the
    /// `fit_view` agent action.
    pub fn fit_view(&mut self, aabb: Aabb, viewport: Rect) {
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return;
        }
        let world_w = aabb.width().max(1.0);
        let world_h = aabb.height().max(1.0);
        const MARGIN: f64 = 0.85;
        let zoom = ((viewport.width / world_w).min(viewport.height / world_h) * MARGIN)
            .clamp(ZOOM_MIN, ZOOM_MAX);
        self.zoom = zoom;
        let center_x = (aabb.min_x + aabb.max_x) / 2.0;
        let center_y = (aabb.min_y + aabb.max_y) / 2.0;
        self.pan_x = viewport.width / 2.0 - center_x * zoom;
        self.pan_y = viewport.height / 2.0 - center_y * zoom;
    }

    /// Single source of truth for a node's on-screen paint radius.
    /// [`crate::render::draw_nodes`] and [`crate::interaction::pick::nearest_node`]
    /// both call this — never two independently-clamped floors.
    pub fn node_screen_radius(&self, base_radius: f32) -> f64 {
        (base_radius as f64 * self.zoom).clamp(NODE_SCREEN_RADIUS_MIN, NODE_SCREEN_RADIUS_MAX)
    }
}

/// Target `(pan, zoom)` that fits `aabb` inside `viewport` with
/// `padding_px` screen-space padding on every side (Wave 2.5 — oss doc
/// §2.6, vasturiano `zoomToFit(ms, px, filterFn)` convention: a literal
/// pixel margin, distinct from [`Camera2D::fit_view`]'s existing
/// ratio-based `MARGIN`, which stays untouched — this is a NEW sibling,
/// not a replacement, so the existing instant `fit_view` keeps its exact
/// prior behavior). Pure — doesn't touch any `Camera2D` instance; a
/// caller (an animated transition, or an instant snap) applies the
/// result itself.
pub fn fit_target(aabb: Aabb, viewport: Rect, padding_px: f64) -> ((f64, f64), f64) {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return ((0.0, 0.0), 1.0);
    }
    let pad = padding_px.max(0.0);
    let avail_w = (viewport.width - 2.0 * pad).max(1.0);
    let avail_h = (viewport.height - 2.0 * pad).max(1.0);
    let world_w = aabb.width().max(1.0);
    let world_h = aabb.height().max(1.0);
    let zoom = (avail_w / world_w).min(avail_h / world_h).clamp(ZOOM_MIN, ZOOM_MAX);
    let center_x = (aabb.min_x + aabb.max_x) / 2.0;
    let center_y = (aabb.min_y + aabb.max_y) / 2.0;
    let pan = (viewport.width / 2.0 - center_x * zoom, viewport.height / 2.0 - center_y * zoom);
    (pan, zoom)
}

/// Auto initial-zoom convention (Wave 2.5 — oss doc §2.6/obsidian doc
/// §5.5, vasturiano `force-graph`'s `ZOOM2NODES_FACTOR = 4`): `4 /
/// cbrt(node_count)` — bigger graphs start more zoomed out. Pure utility,
/// clamped to [`ZOOM_MIN`]/[`ZOOM_MAX`].
///
/// Deliberately NOT wired automatically into `GraphEngine::new`/
/// `seed_positions`/`fit_view` (see `uzor-graph/CLAUDE.md`'s divergence
/// log): virtually every existing engine test constructs an engine, seeds
/// positions, and then clicks/hovers at literal world coordinates
/// assuming the untouched `Camera2D::default()` zoom of `1.0` (screen ==
/// world) — auto-applying a node-count-dependent zoom at construction
/// would silently regress that entire baseline. Exposed as a standalone,
/// tested, pure function instead, for a caller that wants a size-aware
/// starting zoom before its own first `fit_view`/`zoom_to_fit` call.
pub fn auto_initial_zoom(node_count: usize) -> f64 {
    let n = node_count.max(1) as f64;
    (4.0 / n.cbrt()).clamp(ZOOM_MIN, ZOOM_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_screen_round_trips_through_screen_to_world() {
        let camera = Camera2D { pan_x: 12.0, pan_y: -8.0, zoom: 1.5 };
        let viewport = Rect::new(100.0, 50.0, 800.0, 600.0);
        let world = (37.5, -12.25);
        let screen = camera.world_to_screen(world, viewport);
        let back = camera.screen_to_world(screen, viewport);
        assert!((back.0 - world.0).abs() < 1e-9);
        assert!((back.1 - world.1).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_keeps_cursor_world_point_fixed() {
        let mut camera = Camera2D { pan_x: 5.0, pan_y: 5.0, zoom: 1.0 };
        let viewport = Rect::new(0.0, 0.0, 640.0, 480.0);
        let cursor = (300.0, 200.0);
        let world_before = camera.screen_to_world(cursor, viewport);
        camera.zoom_at(cursor, viewport, 1.2);
        let world_after = camera.screen_to_world(cursor, viewport);
        assert!((world_before.0 - world_after.0).abs() < 1e-9);
        assert!((world_before.1 - world_after.1).abs() < 1e-9);
    }

    #[test]
    fn fit_view_frames_the_aabb_centered() {
        let mut camera = Camera2D::default();
        let viewport = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let aabb = Aabb { min_x: -50.0, min_y: -50.0, max_x: 50.0, max_y: 50.0 };
        camera.fit_view(aabb, viewport);
        let center_screen = camera.world_to_screen((0.0, 0.0), viewport);
        assert!((center_screen.0 - viewport.width / 2.0).abs() < 1e-6);
        assert!((center_screen.1 - viewport.height / 2.0).abs() < 1e-6);
        assert!(camera.zoom > 0.0);
    }

    #[test]
    fn node_screen_radius_is_shared_and_monotonic_in_zoom() {
        let mut camera = Camera2D { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        let r1 = camera.node_screen_radius(5.0);
        camera.zoom = 2.0;
        let r2 = camera.node_screen_radius(5.0);
        assert!(r2 > r1);
        // Clamp still bounds absurd zoom.
        camera.zoom = 1000.0;
        assert!(camera.node_screen_radius(5.0) <= NODE_SCREEN_RADIUS_MAX);
        camera.zoom = 0.00001;
        assert!(camera.node_screen_radius(5.0) >= NODE_SCREEN_RADIUS_MIN);
    }

    // ── W2.5 navigation (zoom bounds, fit_target, auto_initial_zoom) ───────

    #[test]
    fn zoom_at_clamps_within_the_wide_0_01_to_1000_bounds() {
        let mut camera = Camera2D { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        let viewport = Rect::new(0.0, 0.0, 640.0, 480.0);
        camera.zoom_at((300.0, 200.0), viewport, 1e9);
        assert!((camera.zoom - ZOOM_MAX).abs() < 1e-6, "an absurd zoom-in factor must clamp at ZOOM_MAX: {}", camera.zoom);

        camera.zoom = 1.0;
        camera.zoom_at((300.0, 200.0), viewport, 1e-9);
        assert!((camera.zoom - ZOOM_MIN).abs() < 1e-6, "an absurd zoom-out factor must clamp at ZOOM_MIN: {}", camera.zoom);
    }

    #[test]
    fn fit_target_centers_the_aabb_and_respects_pixel_padding() {
        let viewport = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let aabb = Aabb { min_x: -50.0, min_y: -50.0, max_x: 50.0, max_y: 50.0 };
        let (pan, zoom) = fit_target(aabb, viewport, 40.0);
        let camera = Camera2D { pan_x: pan.0, pan_y: pan.1, zoom };

        let center_screen = camera.world_to_screen((0.0, 0.0), viewport);
        assert!((center_screen.0 - viewport.width / 2.0).abs() < 1e-6);
        assert!((center_screen.1 - viewport.height / 2.0).abs() < 1e-6);

        // 100x100 world AABB with 40px padding on every side fits inside
        // (1000-80)x(800-80) = 920x720 -> width-bound zoom 9.2, height-bound
        // zoom 7.2 -> the tighter (height) bound wins.
        assert!((zoom - 7.2).abs() < 1e-6, "zoom must be the tighter of the two padded-fit ratios: {zoom}");
    }

    #[test]
    fn fit_target_degenerate_viewport_returns_a_safe_default_instead_of_dividing_by_zero() {
        let aabb = Aabb { min_x: 0.0, min_y: 0.0, max_x: 10.0, max_y: 10.0 };
        let (pan, zoom) = fit_target(aabb, Rect::new(0.0, 0.0, 0.0, 0.0), 40.0);
        assert_eq!(pan, (0.0, 0.0));
        assert_eq!(zoom, 1.0);
    }

    #[test]
    fn auto_initial_zoom_shrinks_as_node_count_grows_and_never_panics_at_zero() {
        let small = auto_initial_zoom(1);
        let big = auto_initial_zoom(1000);
        assert!(small > big, "bigger graphs must start more zoomed out: {small} (n=1) vs {big} (n=1000)");
        assert!(auto_initial_zoom(0) > 0.0, "zero nodes must not divide by zero or panic");
        assert!(auto_initial_zoom(0) <= ZOOM_MAX);
    }
}
