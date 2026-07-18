//! 3D screen-space projection (W3D arc plan §1.3/§1.5/§2). Wave 2 scope
//! is [`project_world_to_screen`] only — it feeds the label overlay
//! (Wave 3: project a node's world position, hand the screen point to
//! the existing `label_grid.rs` LOD pass, draw through the ordinary 2D
//! `RenderContext` path). `screen_to_ray`/`nearest_node_3d` (CPU
//! ray-vs-sphere picking) are Wave 3, not implemented here.

use glam::Vec3;

use uzor::types::Rect;
use uzor_urx_3d::PerspectiveCamera;

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
}
