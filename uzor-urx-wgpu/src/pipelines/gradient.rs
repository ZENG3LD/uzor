//! Gradient (Linear + Radial + Sweep) per-fragment-eval pipeline —
//! real per-fragment LUT sampling for all three kinds (URX Wave 4
//! Commit 3,
//! `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §2.1/§2.3; Linear joined 2026-07-25). Radial (`distance(p,
//! center)/radius`) and Sweep (`atan2(...)`) are NOT affine functions
//! of position — interpolating their VALUES across a triangle visibly
//! facets/bands, so per-fragment eval was always required for them.
//! Linear's `t` IS an affine function of position, so the design
//! originally kept it on a per-VERTEX `TriInstance`-based colour +
//! barycentric-interpolation path, reasoning that was "mathematically
//! exact." That reasoning only holds when `color(t)` is itself affine
//! — a straight 2-stop lerp. For 3+ stops `color(t)` is
//! PIECEWISE-affine, and an ordinary shape's own tessellation (often
//! just a rect's 4 corners) carries no vertex at any interior stop
//! boundary, so the old path silently faded straight between whichever
//! two colours happened to land at the mesh's corners — every
//! intermediate stop vanished. Linear now shares this SAME pipeline;
//! see `encode.rs::transform_gradient_params`'s doc comment for the
//! measured before/after and the exact device-param packing (`p0` =
//! start, `(p1, p2)` = the axis vector `end - start`).
//!
//! `GradientPipeline` needs 2 bind-group-layouts, same shape as
//! `GlyphPipeline`/`BlendCompositePipeline`: group 0 the shared uniform
//! bind group, group 1 `crate::gradient_lut::GradientLutAtlas`'s
//! texture-only bind group layout (`textureLoad`, no sampler — design
//! §2.2/§2.3: CPU's own LUT read is an exact rounded-clamped index with
//! zero interpolation between entries, and this pipeline reproduces
//! that exactly rather than letting bilinear/anisotropic filtering
//! blend between two adjacent, semantically-unrelated LUT rows).
//!
//! Wave 4 status at close (Commits 4-6): mesh vertices reproject
//! through the full affine (Commit 4, see `v0`'s own doc below);
//! parity fixtures `radial_gradient_rect`/`sweep_gradient_rect`/
//! `gradient_on_stroke_path_star_gpu_only_correctness` (Commit 5,
//! `uzor-urx-wgpu/tests/{fixtures,parity}.rs`) exercise this pipeline
//! end-to-end — the last one GPU-only, having surfaced a genuinely new
//! finding that CPU never renders a real gradient on anything but
//! `FillRect` (see that fixture's own doc comment; CLOSED separately,
//! coordinator 2026-07-25 — `uzor-urx-cpu::path::fill_path_aa` now
//! evaluates gradients too, see that crate's own `gradient.rs`).

use bytemuck::{Pod, Zeroable};

/// A Linear/Radial/Sweep gradient-mesh triangle instance — 64 bytes
/// packed (design §2.3's exact layout; Linear joined 2026-07-25).
///
/// Memory layout (64 bytes):
/// - v0:          8 bytes  ([f32; 2] device-space triangle vertex, post
///   FULL-affine `project_local` — translate+scale-only through Wave 4
///   Commit 3, upgraded to the full 6-coefficient affine in Commit 4,
///   design §5.4)
/// - v1:          8 bytes  ([f32; 2])
/// - v2:          8 bytes  ([f32; 2])
/// - p0:          8 bytes  ([f32; 2] Linear: device-space `start`; Radial: end_center device-space; Sweep: center device-space)
/// - p1:          4 bytes  (f32 — Linear: device-space axis `end.x - start.x`; Radial: end_radius, device-scaled; Sweep: start_angle + rotation offset)
/// - p2:          4 bytes  (f32 — Linear: device-space axis `end.y - start.y`; Sweep: end_angle + rotation offset; Radial: unused, 0.0)
/// - kind_extend: 4 bytes  (u32 — bits[0:1] kind (0=Radial,1=Sweep,2=Linear); bits[2:3] extend (0=Pad,1=Repeat,2=Reflect))
/// - lut_row:     4 bytes  (u32 — row index into `GradientLutAtlas`)
/// - clip_rect:  16 bytes  ([f32; 4])
///
/// `p0`/`p1`/`p2`/`kind_extend`/`lut_row` are IDENTICAL across all 3
/// vertices of one triangle (they describe the whole gradient, not a
/// per-vertex quantity) — only `v0`/`v1`/`v2` genuinely vary; the
/// fragment shader reads the gradient params + samples the LUT once
/// per FRAGMENT (design §2.1), using `@builtin(position)` as the query
/// point since `p0`/etc. are already in the SAME device space that
/// varying resolves to (§2.4: gradient params are transformed at
/// encode time, not recomputed per vertex/fragment).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct GradientInstance {
    pub v0: [f32; 2],
    pub v1: [f32; 2],
    pub v2: [f32; 2],
    pub p0: [f32; 2],
    pub p1: f32,
    pub p2: f32,
    pub kind_extend: u32,
    pub lut_row: u32,
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<GradientInstance>() == 64,
    "GradientInstance must stay 64 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `GradientInstance` — 9 attributes matching
/// the byte offsets in the doc comment above 1:1.
pub(crate) fn gradient_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset: 8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x2, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32x2, offset: 24 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32, offset: 32 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32, offset: 36 },
        wgpu::VertexAttribute { shader_location: 6, format: Uint32, offset: 40 },
        wgpu::VertexAttribute { shader_location: 7, format: Uint32, offset: 44 },
        wgpu::VertexAttribute { shader_location: 8, format: Float32x4, offset: 48 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GradientInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Same initial capacity constant as every sibling pipeline.
const INITIAL_CAPACITY: usize = 1024;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_gradient_buffer"),
        size: (capacity * std::mem::size_of::<GradientInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the Radial/Sweep gradient `wgpu::RenderPipeline` pair (design
/// §2.2's `_off`/`_test` convention) + the shared grow-only instance
/// buffer. Does NOT own the LUT texture/bind group — that lives in
/// `crate::gradient_lut::GradientLutAtlas`, borrowed at bind time
/// (`bind`'s `lut_bind_group` parameter), same split
/// `GlyphPipeline`/`BlendCompositePipeline` already use for their own
/// group-1 resource.
pub(crate) struct GradientPipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl GradientPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
        lut_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.gradient_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::GRADIENT_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.gradient_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl), Some(lut_bgl)],
            immediate_size: 0,
        });
        // Hoisted OUTSIDE the `make` closure — see `QuadPipeline::new`'s
        // comment for why (temporary-lifetime fix, Wave 1 finding).
        let vertex_buffers = [gradient_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(crate::pipelines::quad::premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.gradient_pipeline_native"),
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
    /// longer fits. No-op on an empty slice.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[GradientInstance]) {
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

    /// Bind this pipeline + its vertex buffer + `lut_bind_group` at
    /// group 1 onto `pass` (design §2.2's `_off`/`_test` selection —
    /// see `QuadPipeline::bind`'s doc comment for the exact semantics
    /// of `stencil_ref`).
    pub(crate) fn bind(&self, pass: &mut wgpu::RenderPass<'_>, stencil_ref: Option<u32>, lut_bind_group: &wgpu::BindGroup) {
        match stencil_ref {
            None => pass.set_pipeline(&self.pipeline_off),
            Some(r) => {
                pass.set_pipeline(&self.pipeline_test);
                pass.set_stencil_reference(r);
            }
        }
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.set_bind_group(1, lut_bind_group, &[]);
    }

    /// Draw instances `[start, start + count)` — 3 procedurally
    /// generated vertices per instance (one triangle), same as
    /// `PathPipeline::draw_range`.
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..3, start..(start + count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_instance_is_64_bytes() {
        assert_eq!(std::mem::size_of::<GradientInstance>(), 64);
    }
}
