//! Quad SDF pipeline — filled/bordered rounded rectangles.
//!
//! `QuadInstance` layout is copied verbatim from
//! `uzor-render-wgpu-instanced/src/instances.rs:75-86` (same field
//! order/offsets, same packed-`u32` colour wire format). `pack_rgba8`
//! is re-implemented locally (not imported from the legacy crate) so
//! this module stays self-contained — Wave 1's whole point is a
//! native, delegation-free pipeline set, and the legacy crate's own
//! blend-state constant must never leak in here (design §9 risk 2);
//! keeping the tiny packing helper local removes even the appearance
//! of that coupling.
//!
//! Fragment-shader border formula is CSS-centered (design §3), a real
//! behavioural deviation from the legacy crate's inset-only border —
//! see `crate::shaders::QUAD_SHADER_NATIVE`.

use bytemuck::{Pod, Zeroable};

/// Pack a 4-byte RGBA into one `u32` in the `unpack4x8unorm`-compatible
/// layout: byte 0 = R, byte 1 = G, byte 2 = B, byte 3 = A.
#[inline]
pub(crate) const fn pack_rgba8(rgba: [u8; 4]) -> u32 {
    (rgba[0] as u32)
        | ((rgba[1] as u32) << 8)
        | ((rgba[2] as u32) << 16)
        | ((rgba[3] as u32) << 24)
}

/// A filled/bordered rectangle instance — 56 bytes packed. Renders as
/// 2 triangles (6 vertices, procedurally generated in the vertex
/// shader) with a rounded-rect SDF in the fragment shader.
///
/// Memory layout (56 bytes):
/// - pos:           8 bytes  ([f32; 2])
/// - size:          8 bytes  ([f32; 2])
/// - color:         4 bytes  (u32, packed RGBA8)
/// - border_color:  4 bytes  (u32, packed RGBA8)
/// - corner_radius: 4 bytes  (f32)
/// - border_width:  4 bytes  (f32)
/// - _pad0:         8 bytes  (`_pad0[0]` = rotation angle in radians,
///   Wave 4 Commit 4, design §5.3 — a similarity transform's rotation
///   component, applied around the rect's own center in the vertex
///   shader; `_pad0[1]` remains reserved)
/// - clip_rect:    16 bytes  ([f32; 4])
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct QuadInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: u32,
    pub border_color: u32,
    pub corner_radius: f32,
    pub border_width: f32,
    pub _pad0: [f32; 2],
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<QuadInstance>() == 56,
    "QuadInstance must stay 56 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `QuadInstance` — 8 attributes matching the
/// byte offsets in the doc comment above 1:1.
pub(crate) fn quad_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset: 8 },
        wgpu::VertexAttribute { shader_location: 2, format: Uint32, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Uint32, offset: 20 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32, offset: 24 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32, offset: 28 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x2, offset: 32 },
        wgpu::VertexAttribute { shader_location: 7, format: Float32x4, offset: 40 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Initial instance-buffer capacity (number of `QuadInstance`s) —
/// same constant as the legacy crate's `INITIAL_CAPACITY`
/// (`uzor-render-wgpu-instanced/src/renderer.rs:45`).
const INITIAL_CAPACITY: usize = 1024;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_quad_buffer"),
        size: (capacity * std::mem::size_of::<QuadInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Premultiplied blend equation (design §7 "the actual fix") — `One /
/// OneMinusSrcAlpha` on both color and alpha, matching `Pixmap`'s own
/// premultiplied definition. Shared by every native pipeline (Quad,
/// Line, ...); never reuse the legacy crate's
/// `wgpu::BlendState::ALPHA_BLENDING` (straight alpha) here.
pub(crate) fn premultiplied_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

/// Owns the quad SDF `wgpu::RenderPipeline` pair (design §2.2 — `_off`
/// for frames with no active rounded clip, byte-identical to Wave 1/2;
/// `_test` for frames where `EncodedFrame::has_rounded_clip` is true)
/// + the shared grow-only instance buffer.
pub(crate) struct QuadPipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl QuadPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.quad_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::QUAD_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.quad_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl)],
            immediate_size: 0,
        });
        // Hoisted OUTSIDE the `make` closure below (not inlined as
        // `&[quad_instance_layout()]`/`&[Some(ColorTargetState{..})]`
        // directly in the descriptor literal) — `build_off_test_pair`
        // RETURNS the descriptor value out of the closure and consumes
        // it after the call, so any array literal built INSIDE the
        // closure body would be a temporary dropped at the closure's
        // return, which the returned descriptor's borrow can't outlive
        // (confirmed by the compiler: "cannot return value referencing
        // temporary value"). These two locals live for the whole
        // `new()` call, so both `make(None)` and `make(Some(..))`
        // borrow the SAME long-lived arrays safely.
        let vertex_buffers = [quad_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.quad_pipeline_native"),
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

    /// Upload `data`, growing the buffer (doubling capacity) if it no
    /// longer fits — same strategy as the legacy crate's
    /// `upload_instances` (`renderer.rs:619-647`). No-op on an empty
    /// slice.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[QuadInstance]) {
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

    /// Bind this pipeline + its vertex buffer onto `pass`. `stencil_ref:
    /// None` selects the `_off` variant (a frame with NO stencil
    /// attachment at all — `has_rounded_clip == false`, byte-identical
    /// to Wave 1/2); `Some(r)` selects `_test` + `set_stencil_reference(r)`
    /// (design §2.2). Called only when the renderer's batch-replay loop
    /// switches INTO a quad batch (matches legacy `renderer.rs:1008-1061`'s
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
    /// generated vertices per instance (2 triangles), matching the
    /// legacy crate's quad draw call (`renderer.rs:1019-1027`).
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..6, start..(start + count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<QuadInstance>(), 56);
    }

    #[test]
    fn pack_rgba8_round_trip_endianness() {
        // Byte 0 = R, byte 1 = G, byte 2 = B, byte 3 = A.
        let p = pack_rgba8([0x11, 0x22, 0x33, 0x44]);
        assert_eq!(p, 0x4433_2211);
    }
}
