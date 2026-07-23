//! Path/triangle pipeline — flat-shaded, per-vertex-colour triangles
//! from lyon tessellation (`FillPath`/`StrokePath`, and the gradient-
//! routed `FillRect{Brush::Gradient(Linear), radii}` case).
//!
//! `TriInstance` is a genuinely NEW 56-byte layout, NOT a copy of the
//! legacy crate's `TriangleInstance` (design §3, risk §9 item 7):
//! legacy's `TriangleInstance` carries one FLAT `color: u32` per
//! triangle (its gradient support is a per-triangle-centroid
//! approximation); Wave 1 needs `FillRect` to support a real linear
//! gradient without inventing a 4th pipeline, so this struct carries
//! **per-vertex** colour instead and lets the rasteriser interpolate
//! it for free via the `vec4<f32>` vertex→fragment varying.
//!
//! `v0`/`v1`/`v2` are re-projected from LOCAL (pre-transform) mesh
//! space through the frame's FULL 6-coefficient affine at replay time
//! (`encode.rs::project_local`, Wave 4 Commit 4, design §5.4) — was a
//! translate+scale-only decomposition through Wave 1-3. This module's
//! own `TriInstance`/`PathPipeline` are unaffected: they just carry
//! whatever device-space vertices `encode.rs` computes.
//!
//! `PATH_SHADER_NATIVE` deliberately has NO barycentric edge AA —
//! that per-triangle-independent edge fade is the literal mechanism of
//! the legacy crate's internal-tessellation-seam bug (design §4 "AA
//! decision"). MSAA (already armed from Commit 1) supplies all the
//! antialiasing this pipeline needs, both at the shape's outer
//! boundary and at internal tessellation seams (which the legacy
//! per-triangle scheme visibly gapped/double-faded).

use bytemuck::{Pod, Zeroable};

/// A flat/gradient-vertex-coloured triangle instance — 56 bytes
/// packed.
///
/// Memory layout (56 bytes):
/// - v0:      8 bytes  ([f32; 2])
/// - v1:      8 bytes  ([f32; 2])
/// - v2:      8 bytes  ([f32; 2])
/// - color0:  4 bytes  (u32, packed RGBA8 at v0)
/// - color1:  4 bytes  (u32, packed RGBA8 at v1)
/// - color2:  4 bytes  (u32, packed RGBA8 at v2)
/// - _pad0:   4 bytes
/// - clip_rect: 16 bytes ([f32; 4])
///
/// Solid fills/strokes set `color0 == color1 == color2` — degenerates
/// to flat shading, byte-identical to a single-colour triangle.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct TriInstance {
    pub v0: [f32; 2],
    pub v1: [f32; 2],
    pub v2: [f32; 2],
    pub color0: u32,
    pub color1: u32,
    pub color2: u32,
    pub _pad0: f32,
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<TriInstance>() == 56,
    "TriInstance must stay 56 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `TriInstance` — matches the byte offsets
/// in the doc comment above 1:1.
pub(crate) fn tri_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset: 8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x2, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Uint32, offset: 24 },
        wgpu::VertexAttribute { shader_location: 4, format: Uint32, offset: 28 },
        wgpu::VertexAttribute { shader_location: 5, format: Uint32, offset: 32 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32, offset: 36 },
        wgpu::VertexAttribute { shader_location: 7, format: Float32x4, offset: 40 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<TriInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Same initial capacity constant as `QuadPipeline`/`LinePipeline`.
const INITIAL_CAPACITY: usize = 1024;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_path_buffer"),
        size: (capacity * std::mem::size_of::<TriInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the path/triangle `wgpu::RenderPipeline` pair (design §2.2 —
/// `_off`/`_test`, same convention as `QuadPipeline`) + the shared
/// grow-only instance buffer.
pub(crate) struct PathPipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl PathPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.path_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::PATH_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.path_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl)],
            immediate_size: 0,
        });
        // Hoisted OUTSIDE the `make` closure — see `QuadPipeline::new`'s
        // comment for why.
        let vertex_buffers = [tri_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            // Same premultiplied blend state as Quad/Line (design §7).
            blend: Some(crate::pipelines::quad::premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.path_pipeline_native"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &vertex_buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &color_targets,
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil,
                multisample: wgpu::MultisampleState { count: sample_count, ..Default::default() },
                multiview_mask: None,
                cache: None,
            });
        let buffer = make_instance_buffer(device, INITIAL_CAPACITY);

        Self { pipeline_off, pipeline_test, buffer, capacity: INITIAL_CAPACITY }
    }

    /// Upload `data`, growing the buffer (doubling capacity) if
    /// needed. No-op on an empty slice.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[TriInstance]) {
        if data.is_empty() {
            return;
        }
        let needed = data.len();
        if needed > self.capacity {
            while self.capacity < needed {
                self.capacity *= 2;
            }
            self.buffer = make_instance_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    }

    /// Bind this pipeline + its vertex buffer onto `pass` (design §2.2's
    /// `_off`/`_test` selection — see `QuadPipeline::bind`'s doc
    /// comment for the exact semantics of `stencil_ref`).
    pub(crate) fn bind(&self, pass: &mut wgpu::RenderPass<'_>, stencil_ref: Option<u32>) {
        match stencil_ref {
            None => pass.set_pipeline(&self.pipeline_off),
            Some(r) => {
                pass.set_pipeline(&self.pipeline_test);
                pass.set_stencil_reference(r);
            }
        }
        pass.set_vertex_buffer(0, self.buffer.slice(..));
    }

    /// Draw instances `[start, start + count)` — 3 procedurally
    /// generated vertices per instance (one triangle), matching the
    /// legacy crate's triangle draw call (`renderer.rs:1029-1037`).
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..3, start..(start + count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<TriInstance>(), 56);
    }
}
