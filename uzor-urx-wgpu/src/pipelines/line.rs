//! Line/capsule pipeline — single-segment strokes with round or butt
//! caps.
//!
//! `LineInstance` layout is copied verbatim from
//! `uzor-render-wgpu-instanced/src/instances.rs:129-140` (same field
//! order/offsets, same packed-`u32` colour wire format, same
//! `cap_flags` bit convention: `0`=round-round, `1`=butt-start,
//! `2`=butt-end, `3`=butt-both). No behavioural deviation from the
//! legacy capsule SDF (design §3 confirms parity) — only the
//! premultiplied-output construction changes, see
//! `crate::shaders::LINE_SHADER_NATIVE`.

use bytemuck::{Pod, Zeroable};

/// A capsule-SDF line segment instance — 56 bytes packed.
///
/// Memory layout (56 bytes):
/// - start:      8 bytes  ([f32; 2])
/// - end:        8 bytes  ([f32; 2])
/// - color:      4 bytes  (u32, packed RGBA8)
/// - width:      4 bytes  (f32)
/// - cap_flags:  4 bytes  (f32 — 0=round-round, 1=butt-start, 2=butt-end, 3=butt-both)
/// - _pad0:      4 bytes
/// - _pad1:      8 bytes
/// - clip_rect: 16 bytes  ([f32; 4])
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct LineInstance {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub color: u32,
    pub width: f32,
    pub cap_flags: f32,
    pub _pad0: f32,
    pub _pad1: [f32; 2],
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<LineInstance>() == 56,
    "LineInstance must stay 56 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `LineInstance` — matches the byte offsets
/// in the doc comment above 1:1 (identical shape to legacy's
/// `line_instance_layout`, `uzor-render-wgpu-instanced/src/renderer.rs:92-120`).
pub(crate) fn line_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset: 8 },
        wgpu::VertexAttribute { shader_location: 2, format: Uint32, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32, offset: 20 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32, offset: 24 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32, offset: 28 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x2, offset: 32 },
        wgpu::VertexAttribute { shader_location: 7, format: Float32x4, offset: 40 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Same initial capacity constant as `QuadPipeline`
/// (`uzor-render-wgpu-instanced/src/renderer.rs:45`).
const INITIAL_CAPACITY: usize = 1024;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_line_buffer"),
        size: (capacity * std::mem::size_of::<LineInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the line/capsule SDF `wgpu::RenderPipeline` pair (design §2.2
/// — `_off`/`_test`, same convention as `QuadPipeline`) + the shared
/// grow-only instance buffer.
pub(crate) struct LinePipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl LinePipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.line_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::LINE_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.line_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl)],
            immediate_size: 0,
        });
        // Hoisted OUTSIDE the `make` closure — see `QuadPipeline::new`'s
        // comment for why (`build_off_test_pair` returns the descriptor
        // value across the closure boundary, so an array literal built
        // INSIDE the closure is a temporary that can't outlive it).
        let vertex_buffers = [line_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            // Same premultiplied blend state as the Quad pipeline
            // (design §7) — scoped to this pipeline only, never the
            // legacy crate's straight-alpha `ALPHA_BLENDING` constant.
            blend: Some(crate::pipelines::quad::premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.line_pipeline_native"),
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

    /// Upload `data`, growing the buffer (doubling capacity) if needed
    /// — identical strategy to `QuadPipeline::upload`. No-op on an
    /// empty slice.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[LineInstance]) {
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
    /// comment for the exact semantics of `stencil_ref`). Called only
    /// when the renderer's batch-replay loop switches INTO a line
    /// batch (matches legacy `renderer.rs:1008-1061`'s
    /// avoid-redundant-`set_pipeline` pattern).
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

    /// Draw instances `[start, start + count)` — 6 procedurally
    /// generated vertices per instance (oriented quad enclosing the
    /// segment), matching the legacy crate's line draw call
    /// (`renderer.rs:1039-1047`).
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..6, start..(start + count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<LineInstance>(), 56);
    }
}
