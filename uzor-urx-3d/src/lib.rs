//! # uzor-urx-3d
//!
//! URX 3D rendering backend — native XYZ coordinate system,
//! vertex/index buffers, perspective camera, depth-tested raster
//! pipeline. Sibling to URX 2D (`uzor-urx-wgpu-full`); they coexist
//! through render-to-texture bridges (a 3D scene can become a
//! `SceneCmd::image` inside a URX 2D scene, and a URX 2D scene can
//! be rendered to a texture used as a quad inside a 3D scene).
//!
//! Wave 1 scope: unlit pipeline, mesh + Scene3D + perspective camera,
//! spinning cube. Waves 2-7 add instancing, Phong lighting, textures,
//! PBR, shadows, 2D↔3D bridges (see handoff 25 §3).
//!
//! Coordinate system: right-handed (X right, Y up, Z toward viewer),
//! consistent with `glam::Mat4::look_at_rh` / `perspective_rh` and
//! glTF.

pub mod camera;
pub mod gltf_loader;
pub mod light;
pub mod math;
pub mod mesh;
pub mod mesh_cache;
pub mod particles;
pub mod pipeline;
pub mod scene3d;
pub mod texture;

pub use gltf_loader::load_gltf;

pub use camera::PerspectiveCamera;
pub use light::{Light, LightArrayRaw, LightRaw, MAX_LIGHTS};
pub use math::{look_at_rh, model_trs, perspective_rh, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
pub use mesh::{Mesh, MeshLit, MeshPbr, MeshUv, Vertex, VertexLit, VertexPbr, VertexUv};
pub use mesh_cache::{
    MeshCache, MeshGpu, MeshLitCache, MeshLitGpu, MeshPbrCache, MeshPbrGpu, MeshUvCache, MeshUvGpu,
};
pub use particles::{EmitterConfig, Particle, ParticleRenderer, ParticleSystem};
pub use pipeline::{Renderer3D, DEPTH_FORMAT, HDR_FORMAT};
pub use scene3d::{Node, NodeMesh, PbrMaterial, PhongMaterial, Scene3D};
pub use texture::{Texture3D, TextureCache, TextureCube};
