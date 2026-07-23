//! Blend-layer composite pipeline — samples a just-closed layer's
//! resolved RGBA texture and alpha-blends it onto whatever is now the
//! current (parent) target (URX Wave 3 Commit 3 design §3.6,
//! `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`).
//!
//! Per design §0.3's decision, the ONLY fully-implemented `BlendMode`
//! combination is `{Mix::Normal, Compose::SrcOver}` scaled by
//! `alpha` — which is EXACTLY "alpha-blend a texture onto the render
//! target," expressible with the SAME fixed-function premultiplied
//! blend state every other native pipeline shares
//! (`pipelines::quad::premultiplied_blend_state`); non-default `mode`
//! values are silently (but countedly, design §0.3) treated as if they
//! were `Normal`/`SrcOver` by this SAME shader.
//!
//! ## §3.6's premultiply formula — corrected (design deviation,
//! disclosed)
//!
//! The design sketch's fragment formula multiplies the SAMPLED texel's
//! rgb by the SAMPLED texel's own alpha again (`sample.rgb * sample.a`)
//! before scaling by the instance `alpha`. That is WRONG for this
//! pipeline's actual data flow: the layer's own content was rendered
//! through the SAME premultiplied blend state every other native
//! pipeline uses, onto a target cleared to fully transparent, then
//! hardware-resolved (MSAA average) into `resolve_texture` — the
//! resolved texel is therefore ALREADY premultiplied
//! (`texel.rgb == straight_rgb * texel.a`). Re-multiplying `texel.rgb`
//! by `texel.a` a SECOND time would double-premultiply (quadratic in
//! alpha), silently darkening/thinning every partially-transparent
//! layer edge. The correct operation for scaling an ALREADY-premultiplied
//! value by a scalar `alpha` is a uniform per-channel multiply across
//! ALL 4 channels, INCLUDING alpha itself — the exact same reasoning
//! `uzor-urx-cpu::blend::composite_layer_srcover`'s `scaled[i] = src[i]
//! * alpha` already uses for the identical "already-premultiplied bytes,
//! scale by a scalar layer alpha" operation (Wave 3 Commit 1). This
//! shader does `out = texel * alpha` — no separate rgb/alpha unpack, no
//! re-premultiply.

use bytemuck::{Pod, Zeroable};

/// 32 bytes: `alpha: f32, _pad: [f32; 3], clip_rect: [f32; 4]`. No
/// position/size fields — the vertex shader generates a full-viewport
/// procedural quad (design §0.2: layers cover the full viewport, not a
/// bounds-derived sub-rect) directly from `Uniforms::screen_size`, same
/// as every other native pipeline's `quad_vert_pos`-shaped helper.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct BlendCompositeInstance {
    pub alpha: f32,
    pub _pad: [f32; 3],
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<BlendCompositeInstance>() == 32,
    "BlendCompositeInstance must stay 32 bytes — WGSL struct in shaders.rs mirrors this layout"
);

/// Vertex buffer layout for `BlendCompositeInstance` — 3 attributes
/// matching the byte offsets in the doc comment above 1:1.
pub(crate) fn blend_composite_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32, offset: 0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x3, offset: 4 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x4, offset: 16 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<BlendCompositeInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// One composite draw per `PopLayer` — never coalesced with another
/// (each binds a DIFFERENT layer's own resolve texture at group 1), so
/// this buffer's initial capacity is deliberately small; the same
/// grow-on-demand strategy as every other native instance buffer
/// applies if a scene ever nests deeper.
const INITIAL_CAPACITY: usize = 16;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_blend_composite_buffer"),
        size: (capacity * std::mem::size_of::<BlendCompositeInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the composite `wgpu::RenderPipeline` pair (design §2.2 —
/// `_off`/`_test`, same convention as every draw pipeline: a composite
/// drawn while a rounded clip is active must respect it too) + the
/// shared grow-only instance buffer. Does NOT own any layer's resolve
/// texture/bind group — those live in `renderer::BlendLayerTarget`,
/// bound at group 1 by whichever layer is being composited THIS draw
/// (`bind`'s `layer_resolve_bind_group` parameter) — same split
/// `GlyphPipeline` uses for the atlas.
pub(crate) struct BlendCompositePipeline {
    pipeline_off: wgpu::RenderPipeline,
    pipeline_test: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl BlendCompositePipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
        layer_resolve_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.blend_composite_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::BLEND_COMPOSITE_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.blend_composite_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl), Some(layer_resolve_bgl)],
            immediate_size: 0,
        });
        // Hoisted OUTSIDE the `make` closure — `build_off_test_pair`
        // returns the descriptor value across the closure boundary, so
        // an array literal built INSIDE the closure is a temporary that
        // can't outlive it (same finding as every other native
        // pipeline's `new`).
        let vertex_buffers = [blend_composite_instance_layout()];
        let color_targets = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(crate::pipelines::quad::premultiplied_blend_state()),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let (pipeline_off, pipeline_test) =
            crate::pipelines::build_off_test_pair(device, |depth_stencil| wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.blend_composite_pipeline_native"),
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
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[BlendCompositeInstance]) {
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

    /// Bind this pipeline + its vertex buffer + `layer_resolve_bind_group`
    /// at group 1 onto `pass` (design §2.2's `_off`/`_test` selection —
    /// see `pipelines::quad::QuadPipeline::bind`'s doc comment for the
    /// exact semantics of `stencil_ref`).
    pub(crate) fn bind(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        stencil_ref: Option<u32>,
        layer_resolve_bind_group: &wgpu::BindGroup,
    ) {
        match stencil_ref {
            None => pass.set_pipeline(&self.pipeline_off),
            Some(r) => {
                pass.set_pipeline(&self.pipeline_test);
                pass.set_stencil_reference(r);
            }
        }
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.set_bind_group(1, layer_resolve_bind_group, &[]);
    }

    /// Draw instance `index` — 6 procedurally generated vertices (2
    /// triangles), same as every other full-viewport-quad native
    /// pipeline. Never coalesced (design: each composite binds a
    /// DIFFERENT layer's resolve texture), so this always draws exactly
    /// one instance.
    pub(crate) fn draw_one(&self, pass: &mut wgpu::RenderPass<'_>, index: u32) {
        pass.draw(0..6, index..(index + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_composite_instance_is_32_bytes() {
        assert_eq!(std::mem::size_of::<BlendCompositeInstance>(), 32);
    }
}
