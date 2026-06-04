//! Scene3D — collection of nodes with per-node transform + tint.

use crate::light::Light;
use crate::mesh::{Mesh, MeshLit, MeshUv};
use crate::texture::Texture3D;
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

/// Wave 4+5 material model.
///
/// `Unlit`     → Wave 3 `unlit_instanced` (Arc<Mesh>, vertex color only)
/// `Lit`       → Wave 4 `phong_instanced` (Arc<MeshLit> + PhongMaterial)
/// `Textured`  → Wave 5 `textured_instanced` (Arc<MeshUv> + Arc<Texture3D>
///               + PhongMaterial); texel × tint × Phong
#[derive(Clone)]
pub enum NodeMesh {
    Unlit(Arc<Mesh>),
    Lit(Arc<MeshLit>),
    Textured(Arc<MeshUv>, Arc<Texture3D>),
}

#[derive(Copy, Clone, Debug)]
pub struct PhongMaterial {
    pub ambient_strength: f32,
    pub diffuse_strength: f32,
    pub specular_strength: f32,
    pub shininess: f32,
}

impl Default for PhongMaterial {
    fn default() -> Self {
        Self {
            ambient_strength: 0.1,
            diffuse_strength: 0.85,
            specular_strength: 0.4,
            shininess: 32.0,
        }
    }
}

#[derive(Clone)]
pub struct Node {
    pub geometry: NodeMesh,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub color_tint: [f32; 4],
    pub material: PhongMaterial,
}

impl Node {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        Self {
            geometry: NodeMesh::Unlit(mesh),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            material: PhongMaterial::default(),
        }
    }

    pub fn new_lit(mesh: Arc<MeshLit>) -> Self {
        Self {
            geometry: NodeMesh::Lit(mesh),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            material: PhongMaterial::default(),
        }
    }

    pub fn new_textured(mesh: Arc<MeshUv>, texture: Arc<Texture3D>) -> Self {
        Self {
            geometry: NodeMesh::Textured(mesh, texture),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            material: PhongMaterial::default(),
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

    pub fn with_material(mut self, m: PhongMaterial) -> Self {
        self.material = m;
        self
    }

    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn is_lit(&self) -> bool {
        matches!(self.geometry, NodeMesh::Lit(_))
    }
}

#[derive(Clone)]
pub struct Scene3D {
    pub nodes: Vec<Node>,
    pub clear_color: [f32; 4],
    pub lights: Vec<Light>,
    pub ambient: [f32; 3],
}

impl Default for Scene3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene3D {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            clear_color: [0.04, 0.04, 0.08, 1.0],
            lights: Vec::new(),
            ambient: [0.08, 0.08, 0.10],
        }
    }

    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }

    pub fn push_light(&mut self, light: Light) {
        self.lights.push(light);
    }
}
