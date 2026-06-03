//! Scene3D — collection of nodes with per-node transform + tint.

use crate::mesh::Mesh;
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

#[derive(Clone)]
pub struct Node {
    pub mesh: Arc<Mesh>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub color_tint: [f32; 4],
}

impl Node {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        Self {
            mesh,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            color_tint: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn with_translation(mut self, t: Vec3) -> Self {
        self.translation = t;
        self
    }

    pub fn with_rotation(mut self, q: Quat) -> Self {
        self.rotation = q;
        self
    }

    pub fn with_scale(mut self, s: Vec3) -> Self {
        self.scale = s;
        self
    }

    pub fn with_tint(mut self, rgba: [f32; 4]) -> Self {
        self.color_tint = rgba;
        self
    }

    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

#[derive(Default, Clone)]
pub struct Scene3D {
    pub nodes: Vec<Node>,
    pub clear_color: [f32; 4],
}

impl Scene3D {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), clear_color: [0.04, 0.04, 0.08, 1.0] }
    }

    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }
}
