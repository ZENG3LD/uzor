//! Scene3D — collection of nodes with per-node transform + tint.

use crate::light::Light;
use crate::mesh::{Mesh, MeshLit, MeshPbr, MeshUv};
use crate::texture::Texture3D;
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

/// Wave 4-6 material model.
///
/// `Unlit`     → Wave 3 `unlit_instanced`     (Arc<Mesh>, vertex color)
/// `Lit`       → Wave 4 `phong_instanced`     (Arc<MeshLit> + PhongMaterial)
/// `Textured`  → Wave 5 `textured_instanced`  (Arc<MeshUv> + Texture3D)
/// `Pbr`       → Wave 6 `pbr_instanced`       (Arc<MeshPbr> + PbrMaterial:
///                albedo, metalness, roughness, ao, optional normal map)
/// `Line`      → Wave C (owner-ordered edge-quality overhaul)
///                `Renderer3D`'s dedicated always-alpha-blended
///                `LineList` pipeline (`Arc<Mesh>`, same vertex format
///                Unlit reuses — see [`crate::mesh::Mesh::unit_line`]).
///                **Bypasses [`Node::is_transparent`] entirely** — a
///                `Line` node's tint alpha almost always sits inside
///                the ~0.35-0.6 translucent range the industry-standard
///                "graph reads as a cloud of nodes, edges recede"
///                aesthetic wants, but that alpha is baked into the
///                dedicated line pipeline's own fixed blend state, not
///                routed through the generic per-node transparent slow
///                path (`Renderer3D::render_inner`'s own doc comment
///                explains why: that path draws ONE instance per node,
///                which would tank perf at thousands of edges — the
///                line pipeline stays batched into ONE instanced draw
///                call regardless of edge count, exactly like the
///                opaque Unlit/Lit paths).
#[derive(Clone)]
pub enum NodeMesh {
    Unlit(Arc<Mesh>),
    Lit(Arc<MeshLit>),
    Textured(Arc<MeshUv>, Arc<Texture3D>),
    Pbr(Arc<MeshPbr>, PbrMaterial),
    Line(Arc<Mesh>),
}

#[derive(Clone)]
pub struct PbrMaterial {
    pub albedo: Arc<Texture3D>,
    pub normal_map: Option<Arc<Texture3D>>,
    pub metalness: f32,
    pub roughness: f32,
    pub ao: f32,
}

impl PbrMaterial {
    pub fn new(albedo: Arc<Texture3D>) -> Self {
        Self {
            albedo,
            normal_map: None,
            metalness: 0.0,
            roughness: 0.5,
            ao: 1.0,
        }
    }

    pub fn with_metalness(mut self, m: f32) -> Self {
        self.metalness = m.clamp(0.0, 1.0);
        self
    }
    pub fn with_roughness(mut self, r: f32) -> Self {
        self.roughness = r.clamp(0.04, 1.0);
        self
    }
    pub fn with_ao(mut self, ao: f32) -> Self {
        self.ao = ao.clamp(0.0, 1.0);
        self
    }
    pub fn with_normal_map(mut self, nm: Arc<Texture3D>) -> Self {
        self.normal_map = Some(nm);
        self
    }
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

    pub fn new_pbr(mesh: Arc<MeshPbr>, pbr: PbrMaterial) -> Self {
        Self {
            geometry: NodeMesh::Pbr(mesh, pbr),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            material: PhongMaterial::default(), // unused for PBR; keeps API uniform
        }
    }

    /// Wave C — a `LineList`-topology node (see [`NodeMesh::Line`]'s own
    /// doc comment for why this bypasses [`Node::is_transparent`]).
    pub fn new_line(mesh: Arc<Mesh>) -> Self {
        Self {
            geometry: NodeMesh::Line(mesh),
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            material: PhongMaterial::default(), // unused for Line; keeps API uniform
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

    /// Wave 18 — node is treated as transparent if its tint alpha drops
    /// below 1.0. Renderer3D sorts these back-to-front and draws them
    /// AFTER all opaque nodes for correct alpha blending.
    ///
    /// **`NodeMesh::Line` nodes never reach this check in
    /// `Renderer3D::render_inner`** — they're pulled into their own
    /// dedicated instanced group BEFORE this per-node test runs, even
    /// though a typical line tint's alpha (~0.35-0.6) would otherwise
    /// read `true` here. Calling this method directly on a `Line` node
    /// still returns whatever its tint alpha implies — it's just not
    /// CONSULTED by the render loop for that variant. See
    /// [`NodeMesh::Line`]'s own doc comment.
    pub fn is_transparent(&self) -> bool {
        self.color_tint[3] < 0.999
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
