//! Mesh — vertex/index buffers + a couple of built-in primitives.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Vertex format for Wave 1: position + vertex color. Wave 3+ will add
/// normals + UV in a separate VertexLit struct; keeping this one
/// minimal so the unlit pipeline can stay simple.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub _pad0: f32, // align to 16B for tidy std140-ish layout
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(pos: Vec3, color: [f32; 4]) -> Self {
        Self { pos: pos.to_array(), _pad0: 0.0, color }
    }

    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 1,
                },
            ],
        }
    }
}

/// Lit vertex format for Wave 4 — pos + normal + color.
///
/// 48 bytes / vertex with one f32 pad after pos (so normal stays
/// 16-aligned for std140-friendly buffer layout).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexLit {
    pub pos: [f32; 3],
    pub _pad0: f32,
    pub normal: [f32; 3],
    pub _pad1: f32,
    pub color: [f32; 4],
}

impl VertexLit {
    pub fn new(pos: Vec3, normal: Vec3, color: [f32; 4]) -> Self {
        Self {
            pos: pos.to_array(),
            _pad0: 0.0,
            normal: normal.normalize_or_zero().to_array(),
            _pad1: 0.0,
            color,
        }
    }

    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexLit>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 16,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 2,
                },
            ],
        }
    }
}

/// Textured-lit vertex format for Wave 5 — pos + normal + uv.
///
/// 48 bytes / vertex with f32 pads for std140 alignment. UV is 2D so
/// fits naturally in the trailing slot (no color — that's per-instance
/// tint multiplied into the sampled texel).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexUv {
    pub pos: [f32; 3],
    pub _pad0: f32,
    pub normal: [f32; 3],
    pub _pad1: f32,
    pub uv: [f32; 2],
    pub _pad2: [f32; 2],
}

impl VertexUv {
    pub fn new(pos: Vec3, normal: Vec3, uv: [f32; 2]) -> Self {
        Self {
            pos: pos.to_array(),
            _pad0: 0.0,
            normal: normal.normalize_or_zero().to_array(),
            _pad1: 0.0,
            uv,
            _pad2: [0.0; 2],
        }
    }

    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexUv>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 16,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 32,
                    shader_location: 2,
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshUv {
    pub vertices: Vec<VertexUv>,
    pub indices: Vec<u32>,
}

/// PBR vertex format for Wave 6 — pos + normal + tangent + uv.
///
/// 64 bytes / vertex. Tangent comes with an implicit bitangent via
/// `cross(normal, tangent.xyz) * tangent.w` (handedness in tangent.w).
/// Pad of 1 float keeps the struct 16-aligned for std140-style layout.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexPbr {
    pub pos: [f32; 3],
    pub _pad0: f32,
    pub normal: [f32; 3],
    pub _pad1: f32,
    pub tangent: [f32; 4], // xyz = tangent, w = handedness ±1
    pub uv: [f32; 2],
    pub _pad2: [f32; 2],
}

impl VertexPbr {
    pub fn new(pos: Vec3, normal: Vec3, tangent: [f32; 4], uv: [f32; 2]) -> Self {
        Self {
            pos: pos.to_array(),
            _pad0: 0.0,
            normal: normal.normalize_or_zero().to_array(),
            _pad1: 0.0,
            tangent,
            uv,
            _pad2: [0.0; 2],
        }
    }

    pub fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexPbr>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0,  shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 16, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 48, shader_location: 3 },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshPbr {
    pub vertices: Vec<VertexPbr>,
    pub indices: Vec<u32>,
}

impl MeshPbr {
    pub fn new(vertices: Vec<VertexPbr>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Unit cube with per-face normals + tangents + UVs. Same UV winding
    /// as `MeshLit::cube_uv` (so PBR cubes can reuse textures built for
    /// the Wave 5 cube). Tangent points in +U direction; handedness=+1.
    pub fn cube_pbr() -> Self {
        // (corners CCW from outside, normal, tangent in +U direction)
        let faces: [([[f32; 3]; 4], [f32; 3], [f32; 3]); 6] = [
            // +X face: normal=+X, tangent runs along +Z (right of camera)
            (
                [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            // -X
            (
                [[-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, -1.0]],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, -1.0],
            ),
            // +Y
            (
                [[-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0]],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
            ),
            // -Y
            (
                [[-1.0, -1.0, 1.0], [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0]],
                [0.0, -1.0, 0.0],
                [1.0, 0.0, 0.0],
            ),
            // +Z
            (
                [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
            ),
            // -Z
            (
                [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]],
                [0.0, 0.0, -1.0],
                [-1.0, 0.0, 0.0],
            ),
        ];

        let uv_corners = [[0.0_f32, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (i, (corners, normal, tangent)) in faces.iter().enumerate() {
            let base = (i * 4) as u32;
            let n = Vec3::from_array(*normal);
            let t = [tangent[0], tangent[1], tangent[2], 1.0_f32]; // handedness +1
            for (k, c) in corners.iter().enumerate() {
                vertices.push(VertexPbr::new(
                    Vec3::from_array(*c),
                    n,
                    t,
                    uv_corners[k],
                ));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self { vertices, indices }
    }
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct MeshLit {
    pub vertices: Vec<VertexLit>,
    pub indices: Vec<u32>,
}

impl MeshLit {
    pub fn new(vertices: Vec<VertexLit>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Unit cube with per-face flat normals (no shared verts across
    /// faces, so each face's 4 corners get the same normal vector).
    ///
    /// Default face colors match `Mesh::cube_rgb_faces` so existing
    /// demos / tests look the same when migrated to the lit pipeline:
    ///   +X red, -X cyan, +Y green, -Y magenta, +Z blue, -Z yellow.
    pub fn cube_lit() -> Self {
        let faces: [([[f32; 3]; 4], [f32; 3], [f32; 4]); 6] = [
            // +X
            (
                [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 1.0],
            ),
            // -X
            (
                [[-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, -1.0]],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 1.0, 1.0],
            ),
            // +Y
            (
                [[-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0]],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0, 1.0],
            ),
            // -Y
            (
                [[-1.0, -1.0, 1.0], [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0]],
                [0.0, -1.0, 0.0],
                [1.0, 0.0, 1.0, 1.0],
            ),
            // +Z
            (
                [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
            ),
            // -Z
            (
                [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]],
                [0.0, 0.0, -1.0],
                [1.0, 1.0, 0.0, 1.0],
            ),
        ];

        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (i, (corners, normal, color)) in faces.iter().enumerate() {
            let base = (i * 4) as u32;
            let n = Vec3::from_array(*normal);
            for c in corners {
                vertices.push(VertexLit::new(Vec3::from_array(*c), n, *color));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self { vertices, indices }
    }

    /// Cube with UV mapping — each face has its own quad in
    /// `[0,1]×[0,1]` UV space, all six faces share the SAME atlas
    /// region. Wave 5+ atlas manager can substitute different UVs
    /// per face by post-processing this Mesh's vertices.
    pub fn cube_uv() -> MeshUv {
        let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
            (
                [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]],
                [1.0, 0.0, 0.0],
            ),
            (
                [[-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, -1.0]],
                [-1.0, 0.0, 0.0],
            ),
            (
                [[-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0]],
                [0.0, 1.0, 0.0],
            ),
            (
                [[-1.0, -1.0, 1.0], [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0]],
                [0.0, -1.0, 0.0],
            ),
            (
                [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]],
                [0.0, 0.0, 1.0],
            ),
            (
                [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]],
                [0.0, 0.0, -1.0],
            ),
        ];

        let uv_corners = [[0.0_f32, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (i, (corners, normal)) in faces.iter().enumerate() {
            let base = (i * 4) as u32;
            let n = Vec3::from_array(*normal);
            for (k, c) in corners.iter().enumerate() {
                vertices.push(VertexUv::new(
                    Vec3::from_array(*c),
                    n,
                    uv_corners[k],
                ));
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        MeshUv { vertices, indices }
    }

    /// Flat plane on the XZ plane (Y=0) facing +Y.
    /// `extent` half-side, color uniform.
    pub fn plane_lit(extent: f32, color: [f32; 4]) -> Self {
        let n = Vec3::Y;
        let verts = vec![
            VertexLit::new(Vec3::new(-extent, 0.0, -extent), n, color),
            VertexLit::new(Vec3::new(-extent, 0.0, extent), n, color),
            VertexLit::new(Vec3::new(extent, 0.0, extent), n, color),
            VertexLit::new(Vec3::new(extent, 0.0, -extent), n, color),
        ];
        let idx = vec![0, 1, 2, 0, 2, 3];
        Self::new(verts, idx)
    }
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        Self { vertices, indices }
    }

    /// Unit cube centered at origin, side length 2 (extent ±1 on every
    /// axis). Each face gets its own 4 vertices so face colors are
    /// not shared across faces.
    ///
    /// Color convention (matches handoff §3 acceptance — 6 distinct face
    /// colors so the depth test can be verified visually + by readback):
    ///   +X red, -X cyan, +Y green, -Y magenta, +Z blue, -Z yellow.
    pub fn cube_rgb_faces() -> Self {
        let faces: [([[f32; 3]; 4], [f32; 4]); 6] = [
            // +X (right)
            (
                [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]],
                [1.0, 0.0, 0.0, 1.0],
            ),
            // -X (left)
            (
                [[-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, -1.0]],
                [0.0, 1.0, 1.0, 1.0],
            ),
            // +Y (top)
            (
                [[-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0]],
                [0.0, 1.0, 0.0, 1.0],
            ),
            // -Y (bottom)
            (
                [[-1.0, -1.0, 1.0], [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0]],
                [1.0, 0.0, 1.0, 1.0],
            ),
            // +Z (front, facing viewer)
            (
                [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]],
                [0.0, 0.0, 1.0, 1.0],
            ),
            // -Z (back)
            (
                [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]],
                [1.0, 1.0, 0.0, 1.0],
            ),
        ];

        let mut vertices = Vec::with_capacity(24);
        let mut indices = Vec::with_capacity(36);
        for (i, (corners, color)) in faces.iter().enumerate() {
            let base = (i * 4) as u32;
            for c in corners {
                vertices.push(Vertex::new(Vec3::from_array(*c), *color));
            }
            // CCW winding seen from outside (cull mode = back-face).
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        Self { vertices, indices }
    }
}
