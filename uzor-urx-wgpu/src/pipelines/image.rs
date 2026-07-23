//! Image pipeline — textured quad with vertex-stage rotation, sampling
//! a per-`ImageId` texture from `crate::image_cache::NativeImageCache`
//! (URX Wave 4 Commit 3,
//! `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §4.3). `DrawCommand::Image` only — NOT `peniko::Brush::Image` (§4.1,
//! out of scope, unchanged on both backends).
//!
//! ## Byte-count correction (disclosed deviation from design §4.3's
//! literal "64 bytes" claim)
//!
//! Design §4.3's own field list (`pos`/`size`/`uv_pos`/`uv_size`:
//! 8 bytes each = 32; `rotation`/`tint`: 4 bytes each = 8; `clip_rect`:
//! 16 bytes) sums to **56 bytes**, not the 64 the same section's prose
//! states — the same class of "design sketch's arithmetic doesn't
//! match its own literal field list" finding this whole wave's design
//! doc has needed corrected once or twice already (Wave 3's Cargo.toml
//! placement, `FrameOp::PopLayer`'s dead fields). `ImageInstance`
//! implements EXACTLY the fields §4.3 lists, at their natural packing
//! (56 bytes, matching `QuadInstance`/`LineInstance`/`GlyphInstance`/
//! `TriInstance`'s existing 56-byte convention) — no artificial 8-byte
//! pad added just to hit a number the field list itself doesn't
//! support. Every native instance type in this crate already mixes
//! 56-byte (Quad/Line/Path/Glyph) and 64-byte (Gradient, Wave 3's
//! `BlendCompositeInstance`) layouts side by side, so there is no
//! cross-type uniformity requirement this breaks.
//!
//! ## Rotation (design §4.3/§5.2) — Image gets it THIS wave, independent
//! of Quad SDF's OWN rotation extension (Commit 4)
//!
//! `_pad0`/`QUAD_SHADER_NATIVE`'s rotation reinterpretation (design
//! §5.3) is explicit Commit-4 scope (`quad.rs`/`tessellate.rs` are
//! off-limits this commit) — but `ImagePipeline` is a brand-new type
//! with no such constraint, and design §4.3 explicitly specifies
//! "textured quad + rotation in vertex stage" as PART of this
//! pipeline's own Commit-3 scope. The two rotation extensions are
//! independent GPU-side mechanisms (different instance types,
//! different shaders) that happen to use the identical technique
//! (rotate the LOCAL centered half-extent around the quad's own center
//! before placing vertices) — landing on different commits is a
//! scheduling detail, not a design inconsistency.
//!
//! Wave 4 status at close (Commit 5): the parity fixture
//! `image_axis_aligned_and_rotated` (`uzor-urx-wgpu/tests/{fixtures,
//! parity}.rs`) exercises both the axis-aligned and rotated placement
//! paths together with the shared `uzor_urx_image` registry, gated by
//! the `_IMAGE` tolerance tier (bilinear-vs-CPU-1:1-fast-path
//! divergence, plus the rotated instance's own CPU-bbox-vs-true-shape
//! mismatch, design §0.3/§9).

use bytemuck::{Pod, Zeroable};

/// A textured-quad image instance — 56 bytes packed (see this module's
/// own doc comment for the byte-count correction versus design §4.3's
/// literal "64 bytes" prose).
///
/// Memory layout (56 bytes):
/// - pos:        8 bytes  ([f32; 2] device-space top-left, PRE-rotation local extents — same convention as Quad SDF's own rotation technique, §5.3)
/// - size:       8 bytes  ([f32; 2] device-space quad size, pre-rotation)
/// - uv_pos:     8 bytes  ([f32; 2] src_rect top-left, normalized [0,1] against the full image extent)
/// - uv_size:    8 bytes  ([f32; 2] src_rect size, normalized)
/// - rotation:   4 bytes  (f32, radians — similarity-decomposed angle; 0.0 for axis-aligned)
/// - tint:       4 bytes  (u32, packed STRAIGHT RGBA8 multiply-tint — reserved; `0xFFFFFFFF` opaque white = no-op for `DrawCommand::Image` today, no producer sets a tint yet, same "reserved, no-op default" shape as `QuadInstance.border_color` when unused)
/// - clip_rect: 16 bytes  ([f32; 4])
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct ImageInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_pos: [f32; 2],
    pub uv_size: [f32; 2],
    pub rotation: f32,
    pub tint: u32,
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<ImageInstance>() == 56,
    "ImageInstance must stay 56 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `ImageInstance` — 7 attributes matching the
/// byte offsets in the doc comment above 1:1 (same contiguous-location
/// convention as every sibling instance type).
pub(crate) fn image_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset: 8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x2, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32x2, offset: 24 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32, offset: 32 },
        wgpu::VertexAttribute { shader_location: 5, format: Uint32, offset: 36 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x4, offset: 40 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ImageInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Same initial capacity constant as every sibling pipeline.
const INITIAL_CAPACITY: usize = 1024;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_image_instance_buffer"),
        size: (capacity * std::mem::size_of::<ImageInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the image `wgpu::RenderPipeline` pair (design §2.2's
/// `_off`/`_test` convention) + the shared grow-only instance buffer.
/// Does NOT own any per-image texture/bind group — those live in
/// `crate::image_cache::NativeImageCache`, resolved and bound PER BATCH
/// at replay time (`bind`'s `image_bind_group` parameter — a DIFFERENT
/// bind group on every `BatchKind::Image(id)` batch with a different
/// `id`, unlike `GlyphPipeline`/`GradientPipeline`'s single shared
/// atlas/LUT bind group reused across every batch of their kind).
pub(crate) struct ImagePipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl ImagePipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
        image_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.image_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::IMAGE_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.image_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl), Some(image_bgl)],
            immediate_size: 0,
        });
        let vertex_buffers = [image_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(crate::pipelines::quad::premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.image_pipeline_native"),
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
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[ImageInstance]) {
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

    /// Bind this pipeline + its vertex buffer + `image_bind_group` at
    /// group 1 onto `pass` (design §2.2's `_off`/`_test` selection —
    /// see `QuadPipeline::bind`'s doc comment for the exact semantics
    /// of `stencil_ref`). Called ONCE PER `BatchKind::Image(id)` batch
    /// at replay time with THAT batch's own resolved bind group (unlike
    /// `GlyphPipeline`/`GradientPipeline`, which rebind the SAME shared
    /// atlas/LUT group across every batch of their kind).
    pub(crate) fn bind(&self, pass: &mut wgpu::RenderPass<'_>, stencil_ref: Option<u32>, image_bind_group: &wgpu::BindGroup) {
        match stencil_ref {
            None => pass.set_pipeline(&self.pipeline_off),
            Some(r) => {
                pass.set_pipeline(&self.pipeline_test);
                pass.set_stencil_reference(r);
            }
        }
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.set_bind_group(1, image_bind_group, &[]);
    }

    /// Draw instances `[start, start + count)` — 6 procedurally
    /// generated vertices per instance (2 triangles), same as
    /// Quad/Glyph.
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..6, start..(start + count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<ImageInstance>(), 56);
    }
}
