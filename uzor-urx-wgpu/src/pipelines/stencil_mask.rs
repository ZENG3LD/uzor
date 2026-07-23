//! Stencil mask-write pipeline pair — increment/decrement, the ONLY
//! two operations Wave 3's nested rounded-clip protocol needs (design
//! §2.3/§2.4,
//! `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`).
//!
//! Reuses `crate::pipelines::path::{TriInstance, tri_instance_layout}`
//! VERBATIM (no new instance struct) — mask geometry is the SAME
//! local-space triangle soup Wave 1's `TessCache` already produces for
//! `rect_bez_path(rect, radii)`, re-projected through the clip's own
//! transform exactly like `encode.rs::emit_solid_mesh` does for
//! content. Color fields (`color0/1/2`) carry a fixed opaque dummy and
//! are ignored — `STENCIL_MASK_SHADER_NATIVE`'s fragment stage does
//! only the standard `clip_rect` discard (so a mask write additionally
//! respects whatever PLAIN-RECT clip is active — the "hybrid stacks
//! compose" requirement), then returns a throwaway colour that
//! `ColorWrites::empty()` blocks from ever reaching the color
//! attachment.
//!
//! Has NO `_off` variant, unlike every draw pipeline (design §2.2) — a
//! mask write only ever happens inside an already-armed
//! (`EncodedFrame::has_rounded_clip == true`) pass, which by
//! construction always carries a stencil attachment (design §2.5:
//! arming is a frame-wide decision, made before any pass opens).
//!
//! **Wave 3 Commit 3**: mask-write batches replayed into a freshly-
//! opened blend layer (the Risk-4 replay of every currently-active
//! rounded-clip frame, `encode.rs`'s `ClipStack::active_rounded_frames_for_replay`)
//! are ordinary `FrameOp::Draw(Batch { kind: BatchKind::StencilMask(_),
//! .. })` entries in the op stream — the multi-pass executor
//! (`renderer::replay_ops`) replays them exactly like any other batch,
//! with no special-casing for "this mask write happens to be inside a
//! layer" at all; they simply land in whichever pass is current at
//! that point in the scan (the just-opened layer's own pass, since
//! they're emitted immediately after that layer's `PushLayer` marker).

use crate::pipelines::path::{tri_instance_layout, TriInstance};

/// `DepthStencilState` shared by both mask pipelines — differs ONLY in
/// `pass_op` (`IncrementClamp` for push, `DecrementClamp` for pop).
/// `compare: Equal` is gated per-draw via `pass.set_stencil_reference`
/// (design §2.4: push gates on the PARENT depth, pop gates on THIS
/// scope's own depth). `depth_write_enabled`/`depth_compare` are `None`
/// — `wgpu-types-29.0.3`'s own `DepthStencilState` doc comment prefers
/// `None` over `Some(false)`/`Some(Always)` for a format with no depth
/// aspect (`Stencil8` is pure-stencil) — same finding as
/// `pipelines::stencil_test_state`.
fn mask_stencil_state(op: wgpu::StencilOperation) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Stencil8,
        depth_write_enabled: None,
        depth_compare: None,
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: op,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: op,
            },
            read_mask: 0xFF,
            write_mask: 0xFF,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Clip nesting depth is typically 0-3 in real UI (design §3.4) — a
/// small but generous start, same grow-on-demand strategy as every
/// other native instance buffer in this crate.
const INITIAL_CAPACITY: usize = 64;

fn make_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor_urx_wgpu.native_stencil_mask_buffer"),
        size: (capacity * std::mem::size_of::<TriInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Owns the two mask-write `wgpu::RenderPipeline`s + a shared grow-only
/// `TriInstance` buffer (mask-write geometry and drawn-content geometry
/// never coexist in the same buffer, but share the SAME wire layout).
pub(crate) struct StencilMaskPipeline {
    increment: wgpu::RenderPipeline,
    decrement: wgpu::RenderPipeline,
    buffer: wgpu::Buffer,
    capacity: usize,
}

impl StencilMaskPipeline {
    pub(crate) fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        uniform_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uzor_urx_wgpu.stencil_mask_shader_native"),
            source: wgpu::ShaderSource::Wgsl(crate::shaders::STENCIL_MASK_SHADER_NATIVE.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uzor_urx_wgpu.stencil_mask_pipeline_layout_native"),
            bind_group_layouts: &[Some(uniform_bgl)],
            immediate_size: 0,
        });
        let make = |op: wgpu::StencilOperation| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("uzor_urx_wgpu.stencil_mask_pipeline_native"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[tri_instance_layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        // Never touches the color attachment — this
                        // pipeline's whole job is the paired stencil
                        // attachment in the SAME pass (design §2.3).
                        write_mask: wgpu::ColorWrites::empty(),
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(mask_stencil_state(op)),
                multisample: wgpu::MultisampleState { count: sample_count, ..Default::default() },
                multiview_mask: None,
                cache: None,
            })
        };
        let increment = make(wgpu::StencilOperation::IncrementClamp);
        let decrement = make(wgpu::StencilOperation::DecrementClamp);
        let buffer = make_instance_buffer(device, INITIAL_CAPACITY);

        Self { increment, decrement, buffer, capacity: INITIAL_CAPACITY }
    }

    /// Upload `data`, growing the buffer (doubling capacity) if it no
    /// longer fits — same strategy as every other native pipeline's
    /// `upload`. No-op on an empty slice.
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

    /// Bind the INCREMENT variant (a `PushClipRoundedRect`, design
    /// §2.4) + gate on `gate_ref` (the PARENT scope's depth — `depth -
    /// 1` — so the write only lands where the parent scope was already
    /// active; at the outermost push `gate_ref == 0`, which passes
    /// everywhere on a freshly-`Clear`ed-to-0 buffer).
    pub(crate) fn bind_increment(&self, pass: &mut wgpu::RenderPass<'_>, gate_ref: u32) {
        pass.set_pipeline(&self.increment);
        pass.set_stencil_reference(gate_ref);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
    }

    /// Bind the DECREMENT variant (the matching `PopClip`, design §2.4)
    /// + gate on `gate_ref` (THIS scope's own depth — only decrements
    /// pixels CURRENTLY at that depth, restoring them to `depth - 1`).
    pub(crate) fn bind_decrement(&self, pass: &mut wgpu::RenderPass<'_>, gate_ref: u32) {
        pass.set_pipeline(&self.decrement);
        pass.set_stencil_reference(gate_ref);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
    }

    /// Draw instances `[start, start + count)` — 3 procedurally
    /// generated vertices per instance (one triangle), same shape as
    /// `PathPipeline::draw_range`.
    pub(crate) fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, start: u32, count: u32) {
        pass.draw(0..3, start..(start + count));
    }
}
