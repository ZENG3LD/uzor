//! Wave 22 — glTF 2.0 mesh + material loader.
//!
//! Maps a glTF file's primitives → `MeshPbr` + `PbrMaterial` + `Node`.
//! Each glTF mesh primitive becomes one `Node`. World transforms are
//! pre-baked from the node hierarchy so consumers don't need to walk
//! the scene graph.
//!
//! Supported:
//!   - POSITION / NORMAL / TANGENT / TEXCOORD_0
//!   - PBR metallic-roughness (baseColorFactor, baseColorTexture,
//!     metallicFactor, roughnessFactor, normalTexture)
//!   - Node TRS + matrix transforms, flattened to world space
//!   - Multiple scenes (loads the default scene)
//!   - Indexed + non-indexed primitives
//!
//! Skipped (later waves):
//!   - Animation, skinning (Wave 13)
//!   - Morph targets
//!   - KHR_* extensions
//!   - Cameras, lights from the glTF scene
//!
//! Errors are simple `String` messages — apps don't typically retry
//! a failed glTF load, they surface the message.

use crate::mesh::{MeshPbr, VertexPbr};
use crate::scene3d::{Node, PbrMaterial};
use crate::texture::Texture3D;
use glam::{Mat4, Quat, Vec3};
use std::path::Path;
use std::sync::Arc;

/// Load a glTF (or .glb) file and return a flat list of `Node`s with
/// world-space transforms already applied to each primitive's mesh.
///
/// Textures referenced by materials are uploaded to GPU on the fly.
/// If a material lacks a baseColor texture, a 1×1 albedo from the
/// baseColorFactor is generated.
pub fn load_gltf(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: impl AsRef<Path>,
) -> Result<Vec<Node>, String> {
    let path = path.as_ref();
    let (doc, buffers, images) = gltf::import(path)
        .map_err(|e| format!("glTF import failed for {:?}: {e}", path))?;

    // Cache textures by gltf::image::Data index so multiple materials
    // sharing the same image hit the same Arc<Texture3D>.
    let mut tex_cache: Vec<Option<Arc<Texture3D>>> = vec![None; images.len()];

    let scene = doc.default_scene()
        .or_else(|| doc.scenes().next())
        .ok_or_else(|| "glTF has no scenes".to_string())?;

    let mut out: Vec<Node> = Vec::new();
    for root in scene.nodes() {
        walk_node(
            &root, Mat4::IDENTITY, &doc, &buffers, &images,
            &mut tex_cache, device, queue, &mut out,
        )?;
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn walk_node(
    node: &gltf::Node,
    parent: Mat4,
    doc: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    tex_cache: &mut [Option<Arc<Texture3D>>],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    out: &mut Vec<Node>,
) -> Result<(), String> {
    let local = match node.transform() {
        gltf::scene::Transform::Matrix { matrix } => Mat4::from_cols_array_2d(&matrix),
        gltf::scene::Transform::Decomposed { translation, rotation, scale } => {
            Mat4::from_scale_rotation_translation(
                Vec3::from(scale),
                Quat::from_array(rotation),
                Vec3::from(translation),
            )
        }
    };
    let world = parent * local;

    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            let (mp, mat) = build_primitive(
                &prim, doc, buffers, images, tex_cache, device, queue,
            )?;
            // Pre-bake world transform into the node's own TRS by
            // decomposing the matrix. We use scale/rotation/translation
            // so `Node::model_matrix()` produces the same matrix.
            let (scale, rotation, translation) = decompose_trs(world);
            out.push(
                Node::new_pbr(Arc::new(mp), mat)
                    .with_translation(translation)
                    .with_rotation(rotation)
                    .with_scale(scale),
            );
        }
    }
    for child in node.children() {
        walk_node(&child, world, doc, buffers, images, tex_cache, device, queue, out)?;
    }
    Ok(())
}

fn decompose_trs(m: Mat4) -> (Vec3, Quat, Vec3) {
    // Mat4::to_scale_rotation_translation isn't guaranteed for
    // non-positive-determinant matrices; for typical glTF this works.
    let (s, r, t) = m.to_scale_rotation_translation();
    (s, r, t)
}

fn build_primitive(
    prim: &gltf::Primitive,
    _doc: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    tex_cache: &mut [Option<Arc<Texture3D>>],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(MeshPbr, PbrMaterial), String> {
    let reader = prim.reader(|b| Some(&buffers[b.index()]));

    let positions = reader.read_positions()
        .ok_or_else(|| "primitive missing POSITION".to_string())?
        .collect::<Vec<_>>();
    let normals: Vec<[f32; 3]> = reader.read_normals()
        .map(|i| i.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
    let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
        .map(|t| t.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
    let tangents: Vec<[f32; 4]> = reader.read_tangents()
        .map(|t| t.collect())
        .unwrap_or_else(|| {
            // No explicit tangents — synthesize a plausible one in
            // each vertex's tangent plane. Not MikkTSpace-correct, but
            // good enough for unlit / non-normal-mapped surfaces.
            normals.iter().map(|n| {
                let n = Vec3::from(*n);
                let arb = if n.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
                let t = arb.cross(n).normalize_or_zero();
                [t.x, t.y, t.z, 1.0]
            }).collect()
        });

    let mut vertices = Vec::with_capacity(positions.len());
    for i in 0..positions.len() {
        vertices.push(VertexPbr::new(
            Vec3::from(positions[i]),
            Vec3::from(normals[i]),
            tangents[i],
            uvs[i],
        ));
    }

    let indices: Vec<u32> = match reader.read_indices() {
        Some(idx) => idx.into_u32().collect(),
        None => {
            // Sequential triangle list.
            (0..vertices.len() as u32).collect()
        }
    };

    let mesh = MeshPbr::new(vertices, indices);

    // Material
    let pbr = prim.material();
    let mr = pbr.pbr_metallic_roughness();
    let base_color = mr.base_color_factor(); // [f32; 4]
    let metalness = mr.metallic_factor();
    let roughness = mr.roughness_factor();

    let albedo = match mr.base_color_texture() {
        Some(info) => {
            let image_idx = info.texture().source().index();
            tex_from_image(image_idx, images, tex_cache, device, queue, base_color)?
        }
        None => {
            // 1×1 from baseColorFactor.
            let px = [
                (base_color[0] * 255.0).clamp(0.0, 255.0) as u8,
                (base_color[1] * 255.0).clamp(0.0, 255.0) as u8,
                (base_color[2] * 255.0).clamp(0.0, 255.0) as u8,
                (base_color[3] * 255.0).clamp(0.0, 255.0) as u8,
            ];
            Arc::new(Texture3D::from_rgba8(device, queue, 1, 1, &px))
        }
    };

    let mut mat = PbrMaterial::new(albedo)
        .with_metalness(metalness)
        .with_roughness(roughness);

    if let Some(nm) = pbr.normal_texture() {
        let image_idx = nm.texture().source().index();
        // Normal maps don't get the base-color tint; pass white.
        let nm_tex = tex_from_image(image_idx, images, tex_cache, device, queue, [1.0; 4])?;
        mat = mat.with_normal_map(nm_tex);
    }

    Ok((mesh, mat))
}

fn tex_from_image(
    idx: usize,
    images: &[gltf::image::Data],
    cache: &mut [Option<Arc<Texture3D>>],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    _factor: [f32; 4],
) -> Result<Arc<Texture3D>, String> {
    if let Some(t) = &cache[idx] { return Ok(t.clone()); }
    let img = &images[idx];
    let (w, h) = (img.width, img.height);
    // glTF images come in many formats; convert to RGBA8.
    let rgba = match img.format {
        gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
        gltf::image::Format::R8G8B8 => {
            let mut v = Vec::with_capacity((w * h * 4) as usize);
            for ch in img.pixels.chunks(3) {
                v.extend_from_slice(ch);
                v.push(255);
            }
            v
        }
        gltf::image::Format::R8 => {
            let mut v = Vec::with_capacity((w * h * 4) as usize);
            for p in &img.pixels {
                v.extend_from_slice(&[*p, *p, *p, 255]);
            }
            v
        }
        gltf::image::Format::R8G8 => {
            let mut v = Vec::with_capacity((w * h * 4) as usize);
            for ch in img.pixels.chunks(2) {
                v.extend_from_slice(&[ch[0], ch[1], 0, 255]);
            }
            v
        }
        other => return Err(format!("unsupported glTF image format: {:?}", other)),
    };
    let tex = Arc::new(Texture3D::from_rgba8(device, queue, w, h, &rgba));
    cache[idx] = Some(tex.clone());
    Ok(tex)
}
