//! Glyph pipeline — textured quads sampling `crate::atlas::NativeGlyphAtlas`'s
//! R8 coverage texture.
//!
//! `GlyphInstance` layout is copied verbatim from the legacy crate's
//! `GlyphInstance` (`uzor-render-wgpu-instanced/src/glyph_instance.rs:26-42`):
//! same field order/offsets, same packed-`u32` colour format, `clip_rect`
//! last at offset 40 — matches every other native instance struct's
//! convention (Quad/Line/Tri all end in `clip_rect: [f32; 4]` at the
//! same relative position). URX Wave 2 Commit 2
//! (`docs/uzor-engines/plans/urx-wave2-native-glyph-atlas-design-2026-07-25.md`
//! §4).
//!
//! Unlike Quad/Line/Path (1 bind-group-layout pipelines: the shared
//! uniform group 0 only), `GlyphPipeline` needs 2 —
//! `[uniform_bgl, atlas_bgl]` — since every glyph instance samples the
//! atlas texture at group 1. `NativeUrxRenderer::with_config` therefore
//! must construct `NativeGlyphAtlas` BEFORE `GlyphPipeline::new` (the
//! pipeline borrows the atlas's `BindGroupLayout` at pipeline-creation
//! time, fixed for the pipeline's lifetime — this crate has no
//! atlas-resize path, so no BGL-invalidation concern to handle).

use bytemuck::{Pod, Zeroable};

/// A single glyph quad instance — 56 bytes packed.
///
/// Memory layout (56 bytes):
/// - pos:      8 bytes  ([f32; 2] — top-left of the glyph quad, screen px)
/// - size:     8 bytes  ([f32; 2] — bitmap width/height, screen px)
/// - uv_pos:   8 bytes  ([f32; 2] — atlas UV top-left, 0..1)
/// - uv_size:  8 bytes  ([f32; 2] — atlas UV width/height, 0..1)
/// - color:    4 bytes  (u32, packed STRAIGHT (non-premultiplied) RGBA8 brush colour)
/// - _pad0:    4 bytes  (f32) — REPURPOSED (URX text-gamma design,
///   2026-07-26, §2.5a): the foreground-luma gamma-bin index
///   (`uzor_urx_core::text_gamma::luma_bin`'s output, `0` or `1` for
///   `TEXT_GAMMA_BINS = 2`), written as a plain `f32` (`0.0`/`1.0`) by
///   `encode.rs::encode_glyph_run`. NOT always-0 padding anymore — see
///   that function's own doc comment. Layout size is UNCHANGED (56
///   bytes, verified by the `assert!` below) — this is a value-meaning
///   change only, never a field-layout change.
/// - clip_rect: 16 bytes ([f32; 4])
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct GlyphInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_pos: [f32; 2],
    pub uv_size: [f32; 2],
    pub color: u32,
    pub _pad0: f32,
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<GlyphInstance>() == 56,
    "GlyphInstance must stay 56 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `GlyphInstance` — 7 attributes matching the
/// byte offsets in the doc comment above 1:1 (same contiguous-location
/// convention as Quad/Line/Path, including an explicit attribute for
/// the `_pad0` padding field so the WGSL instance struct's field count
/// matches this array 1:1).
pub(crate) fn glyph_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset: 8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x2, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32x2, offset: 24 },
        wgpu::VertexAttribute { shader_location: 4, format: Uint32, offset: 32 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32, offset: 36 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x4, offset: 40 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Same initial capacity constant as `QuadPipeline`/`LinePipeline`
/// (`uzor-render-wgpu-instanced/src/renderer.rs:45`).
const INITIAL_CAPACITY: usize = 1024;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_glyph_buffer"),
        size: (capacity * std::mem::size_of::<GlyphInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the glyph `wgpu::RenderPipeline` pair (design §2.2 —
/// `_off`/`_test`, same convention as `QuadPipeline`) + the shared
/// grow-only instance buffer. Does NOT own the atlas texture/bind
/// group — those live in `crate::atlas::NativeGlyphAtlas`, borrowed by
/// reference at bind time (`bind`'s `atlas_bind_group` parameter), same
/// split `NativeUrxRenderer::render_into_encoder`'s batch-replay loop
/// uses for every other pipeline (bind group 0 once per pass; group 1
/// here switches to whichever atlas is live).
pub(crate) struct GlyphPipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl GlyphPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
        atlas_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.glyph_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::GLYPH_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.glyph_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl), Some(atlas_bgl)],
            immediate_size: 0,
        });
        // Hoisted OUTSIDE the `make` closure — see `QuadPipeline::new`'s
        // comment for why.
        let vertex_buffers = [glyph_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            // Same premultiplied blend state as every other native
            // pipeline (design §7 / §4) — never the legacy crate's
            // straight-alpha `ALPHA_BLENDING`.
            blend: Some(crate::pipelines::quad::premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.glyph_pipeline_native"),
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
    /// longer fits — same strategy as `QuadPipeline::upload`. No-op on
    /// an empty slice.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[GlyphInstance]) {
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

    /// Bind this pipeline + its vertex buffer + `atlas_bind_group` at
    /// group 1 onto `pass` (design §2.2's `_off`/`_test` selection —
    /// see `QuadPipeline::bind`'s doc comment for the exact semantics
    /// of `stencil_ref`). Group 0 (the shared uniform bind group) is
    /// bound once per pass by `render_into_encoder`, same as
    /// Quad/Line/Path — only group 1 is this pipeline's own concern,
    /// since it's the only native pipeline with a second bind group.
    pub(crate) fn bind(&self, pass: &mut wgpu::RenderPass<'_>, stencil_ref: Option<u32>, atlas_bind_group: &wgpu::BindGroup) {
        match stencil_ref {
            None => pass.set_pipeline(&self.pipeline_off),
            Some(r) => {
                pass.set_pipeline(&self.pipeline_test);
                pass.set_stencil_reference(r);
            }
        }
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.set_bind_group(1, atlas_bind_group, &[]);
    }

    /// Draw instances `[start, start + count)` — 6 procedurally
    /// generated vertices per instance (2 triangles), same as
    /// Quad/Line.
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..6, start..(start + count));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<GlyphInstance>(), 56);
    }
}
