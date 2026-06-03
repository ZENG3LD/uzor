//! PerspectiveCamera — view + projection matrices.

use glam::{Mat4, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct PerspectiveCamera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub fov_y: f32,   // radians
    pub aspect: f32,  // w / h
    pub z_near: f32,
    pub z_far: f32,
}

impl PerspectiveCamera {
    pub fn new(eye: Vec3, target: Vec3, aspect: f32) -> Self {
        Self {
            eye,
            target,
            up: Vec3::Y,
            fov_y: 60_f32.to_radians(),
            aspect,
            z_near: 0.1,
            z_far: 100.0,
        }
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye, self.target, self.up)
    }

    pub fn proj(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, self.z_near, self.z_far)
    }

    pub fn view_proj(&self) -> Mat4 {
        self.proj() * self.view()
    }
}
