//! Math re-exports — RH coordinate system (X right, Y up, Z toward viewer).
//!
//! All transforms use right-handed conventions matching learnopengl /
//! glTF / Vulkan/wgpu NDC after the projection's depth-range remap.

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

#[inline]
pub fn perspective_rh(fov_y_rad: f32, aspect: f32, z_near: f32, z_far: f32) -> Mat4 {
    Mat4::perspective_rh(fov_y_rad, aspect, z_near, z_far)
}

#[inline]
pub fn look_at_rh(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_rh(eye, target, up)
}

#[inline]
pub fn model_trs(translation: Vec3, rotation: Quat, scale: Vec3) -> Mat4 {
    Mat4::from_scale_rotation_translation(scale, rotation, translation)
}
