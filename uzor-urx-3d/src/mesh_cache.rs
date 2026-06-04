//! Mesh GPU buffer cache.
//!
//! Wave 1 uploaded vertex/index buffers per Node per frame (~30µs of
//! GPU alloc + queue.write per node — at 33 nodes that's 1ms straight
//! into the per-frame budget, pushing us off vsync). Wave 2 keeps a
//! registry keyed on `Arc<Mesh>` pointer identity: identical Arc → one
//! pair of GPU buffers reused every frame.
//!
//! Two reasons we key on Arc identity (not content hash):
//!   - O(1) lookup, no allocator round-trips per draw call
//!   - Authoritative — if the consumer wants a different mesh, they
//!     clone+rebuild and we automatically get a new entry
//!
//! Eviction: a generation counter is bumped each frame; entries not
//! touched for `EVICT_AFTER` frames get released. Keeps the cache
//! bounded for streaming/procedural workloads (Wave 6+ might add LRU
//! or a memory-budget gate; not needed for Wave 2 acceptance).

use crate::mesh::{Mesh, MeshLit};
use bytemuck::cast_slice;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

const EVICT_AFTER_FRAMES: u32 = 240; // ~4s at 60fps

pub struct MeshGpu {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
    last_touched: u32,
}

/// Pointer-identity key over `Arc<Mesh>`. Wave 2 only matches
/// identical Arc instances; if a Node clones the Mesh into a new
/// allocation it gets its own slot.
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct MeshKey(*const Mesh);

// `*const Mesh` isn't Send/Sync by default, but we never deref the
// pointer for anything but identity; the actual Mesh lives behind
// Arc<Mesh> in the Node so it stays alive for the cache lookup.
unsafe impl Send for MeshKey {}
unsafe impl Sync for MeshKey {}

pub struct MeshCache {
    entries: HashMap<MeshKey, MeshGpu>,
    frame: u32,
}

impl Default for MeshCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), frame: 0 }
    }

    /// Mark a new frame — touches counter bumps and any entry not
    /// touched for `EVICT_AFTER_FRAMES` frames is released.
    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let cutoff = self.frame.wrapping_sub(EVICT_AFTER_FRAMES);
        self.entries.retain(|_, e| {
            // Saturating-style age compare with wrapping arithmetic
            let age = self.frame.wrapping_sub(e.last_touched);
            age < EVICT_AFTER_FRAMES || cutoff == 0
        });
    }

    /// Get or upload buffers for a mesh.
    pub fn get_or_upload(&mut self, device: &wgpu::Device, mesh: &Arc<Mesh>) -> &MeshGpu {
        let key = MeshKey(Arc::as_ptr(mesh));
        // Two-step to keep the borrow checker happy
        if !self.entries.contains_key(&key) {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("urx3d.mesh.vb"),
                contents: cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("urx3d.mesh.ib"),
                contents: cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.entries.insert(
                key,
                MeshGpu {
                    vb,
                    ib,
                    index_count: mesh.indices.len() as u32,
                    last_touched: self.frame,
                },
            );
        }
        let e = self.entries.get_mut(&key).unwrap();
        e.last_touched = self.frame;
        e
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─── MeshLit cache (Wave 4) ──────────────────────────────────────────

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct MeshLitKey(*const MeshLit);
unsafe impl Send for MeshLitKey {}
unsafe impl Sync for MeshLitKey {}

pub struct MeshLitGpu {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub index_count: u32,
    last_touched: u32,
}

pub struct MeshLitCache {
    entries: HashMap<MeshLitKey, MeshLitGpu>,
    frame: u32,
}

impl Default for MeshLitCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshLitCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), frame: 0 }
    }

    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        let cutoff = self.frame.wrapping_sub(EVICT_AFTER_FRAMES);
        self.entries.retain(|_, e| {
            let age = self.frame.wrapping_sub(e.last_touched);
            age < EVICT_AFTER_FRAMES || cutoff == 0
        });
    }

    pub fn get_or_upload(&mut self, device: &wgpu::Device, mesh: &Arc<MeshLit>) -> &MeshLitGpu {
        let key = MeshLitKey(Arc::as_ptr(mesh));
        if !self.entries.contains_key(&key) {
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("urx3d.mesh_lit.vb"),
                contents: cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("urx3d.mesh_lit.ib"),
                contents: cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            self.entries.insert(
                key,
                MeshLitGpu {
                    vb,
                    ib,
                    index_count: mesh.indices.len() as u32,
                    last_touched: self.frame,
                },
            );
        }
        let e = self.entries.get_mut(&key).unwrap();
        e.last_touched = self.frame;
        e
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
