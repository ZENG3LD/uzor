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

pub const ZOOM_MIN: f64 = 0.02;
pub const ZOOM_MAX: f64 = 12.0;

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
}
