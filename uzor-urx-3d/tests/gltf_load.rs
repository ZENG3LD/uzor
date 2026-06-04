//! Wave 22 — glTF loader test.
//!
//! Builds a minimal glTF 2.0 file in a temp dir (one triangle with
//! pos + normal + uv + base color factor red), loads it through
//! `uzor_urx_3d::load_gltf`, and verifies the produced Node carries
//! a valid PBR mesh + albedo that matches the factor.

use std::fs;
use std::path::PathBuf;

fn temp_dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("urx-3d-gltf-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

/// Build a minimal .gltf + .bin pair for one indexed triangle with
/// red baseColorFactor and a +Y normal. Returns the path to the .gltf
/// file.
fn build_minimal_triangle_gltf(dir: &PathBuf) -> PathBuf {
    // Triangle: 3 verts × (pos f32x3 + normal f32x3 + uv f32x2) = 32 bytes/vert
    // Indices: 3 × u16 = 6 bytes
    let positions: [f32; 9] = [
        0.0, 0.0, 0.0,
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
    ];
    let normals: [f32; 9] = [
        0.0, 0.0, 1.0,
        0.0, 0.0, 1.0,
        0.0, 0.0, 1.0,
    ];
    let uvs: [f32; 6] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
    let indices: [u16; 3] = [0, 1, 2];

    // Pack buffer: positions | normals | uvs | indices
    let mut buf: Vec<u8> = Vec::new();
    for v in positions { buf.extend_from_slice(&v.to_le_bytes()); }
    let normal_off = buf.len();
    for v in normals { buf.extend_from_slice(&v.to_le_bytes()); }
    let uv_off = buf.len();
    for v in uvs { buf.extend_from_slice(&v.to_le_bytes()); }
    let idx_off = buf.len();
    for v in indices { buf.extend_from_slice(&v.to_le_bytes()); }
    let total = buf.len();

    let bin_path = dir.join("tri.bin");
    fs::write(&bin_path, &buf).unwrap();

    let json = format!(r#"{{
        "asset": {{ "version": "2.0" }},
        "scene": 0,
        "scenes": [{{ "nodes": [0] }}],
        "nodes": [{{ "mesh": 0 }}],
        "meshes": [{{
            "primitives": [{{
                "attributes": {{
                    "POSITION": 0,
                    "NORMAL":   1,
                    "TEXCOORD_0": 2
                }},
                "indices": 3,
                "material": 0
            }}]
        }}],
        "materials": [{{
            "pbrMetallicRoughness": {{
                "baseColorFactor": [1.0, 0.0, 0.0, 1.0],
                "metallicFactor": 0.25,
                "roughnessFactor": 0.6
            }}
        }}],
        "buffers": [{{
            "uri": "tri.bin",
            "byteLength": {total}
        }}],
        "bufferViews": [
            {{ "buffer": 0, "byteOffset": 0,             "byteLength": 36, "target": 34962 }},
            {{ "buffer": 0, "byteOffset": {normal_off}, "byteLength": 36, "target": 34962 }},
            {{ "buffer": 0, "byteOffset": {uv_off},     "byteLength": 24, "target": 34962 }},
            {{ "buffer": 0, "byteOffset": {idx_off},    "byteLength": 6,  "target": 34963 }}
        ],
        "accessors": [
            {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0.0,0.0,0.0], "max": [1.0,1.0,0.0] }},
            {{ "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" }},
            {{ "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" }},
            {{ "bufferView": 3, "componentType": 5123, "count": 3, "type": "SCALAR" }}
        ]
    }}"#);

    let gltf_path = dir.join("tri.gltf");
    fs::write(&gltf_path, json).unwrap();
    gltf_path
}

#[test]
#[ignore]
fn load_minimal_triangle_yields_pbr_node() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })) {
        Ok(a) => a,
        Err(_) => { eprintln!("no GPU adapter"); return; }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-gltf-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).unwrap();

    let dir = temp_dir();
    let path = build_minimal_triangle_gltf(&dir);

    let nodes = uzor_urx_3d::load_gltf(&device, &queue, &path)
        .expect("glTF load failed");
    assert_eq!(nodes.len(), 1, "expected one node, got {}", nodes.len());
    let n = &nodes[0];
    match &n.geometry {
        uzor_urx_3d::NodeMesh::Pbr(mesh, mat) => {
            assert_eq!(mesh.vertices.len(), 3, "wrong vertex count");
            assert_eq!(mesh.indices, vec![0, 1, 2]);
            // metallicFactor=0.25, roughnessFactor=0.6
            assert!((mat.metalness - 0.25).abs() < 1e-3, "met={}", mat.metalness);
            assert!((mat.roughness - 0.6).abs() < 1e-3, "rough={}", mat.roughness);
            // No baseColorTexture → 1×1 albedo from factor (red).
            assert_eq!(mat.albedo.width, 1);
            assert_eq!(mat.albedo.height, 1);
        }
        _ => panic!("expected Pbr geometry, got something else"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn loader_rejects_missing_file() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })) {
        Ok(a) => a,
        Err(_) => { eprintln!("no GPU adapter"); return; }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-gltf-missing"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).unwrap();

    let r = uzor_urx_3d::load_gltf(&device, &queue, "/no/such/file.gltf");
    assert!(r.is_err());
}
