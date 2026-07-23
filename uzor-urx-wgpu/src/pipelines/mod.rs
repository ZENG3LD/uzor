//! Native wgpu pipelines — one module per primitive family.
//!
//! Wave 1 Commit 1 shipped the Quad SDF pipeline (`FillRect`/
//! `StrokeRect`, solid brush). Commit 2 added the Line/capsule
//! pipeline. Commit 3 added the Path/triangle pipeline (lyon
//! tessellation). Wave 2 Commit 2 added the Glyph pipeline (textured
//! quads sampling `crate::atlas::NativeGlyphAtlas`). Wave 3 Commit 2
//! adds the stencil-clip machinery: `stencil_mask` (the mask-write
//! pipeline pair) and `stencil_test_state()` below, shared by every
//! draw pipeline's new `_test` variant
//! (`docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`
//! §2.2).

pub(crate) mod glyph;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod quad;
pub(crate) mod stencil_mask;

/// The `DepthStencilState` every draw pipeline's `_test` variant shares
/// (design §2.2) — `Equal(ref)` gate, content NEVER writes stencil
/// (`Keep`/`Keep`/`Keep`), only the mask-write pipelines
/// (`stencil_mask.rs`) actually mutate the buffer.
///
/// **API finding (design §2.2's sketch used bare `bool`/`CompareFunction`
/// for `depth_write_enabled`/`depth_compare` — stale for the pinned
/// wgpu 29.0.3): both fields are `Option<_>` in this version.** Per
/// `wgpu-types-29.0.3/src/render.rs`'s own doc comment on
/// `DepthStencilState`: "If `format` is a depth or depth/stencil
/// format, then this must be `Some`. Otherwise, specifying `None` is
/// preferred" — `Stencil8` has no depth aspect at all, so `None` is the
/// format's OWN documented preference, used here rather than the
/// also-accepted `Some(false)`/`Some(Always)`.
pub(crate) fn stencil_test_state() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Stencil8,
        depth_write_enabled: None,
        depth_compare: None,
        stencil: wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Keep,
            },
            back: wgpu::StencilFaceState {
                compare: wgpu::CompareFunction::Equal,
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: wgpu::StencilOperation::Keep,
            },
            read_mask: 0xFF,
            write_mask: 0xFF,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Build the `_off`/`_test` `wgpu::RenderPipeline` pair every draw
/// pipeline (Quad/Line/Path/Glyph) needs from ONE shared descriptor
/// recipe (design §2.2 "a shared builder") — called once per draw-
/// pipeline kind at construction, avoiding 4x copy-pasted
/// `depth_stencil` boilerplate. `make` is invoked TWICE (`None`, then
/// `Some(stencil_test_state())`) and must build a FRESH
/// `RenderPipelineDescriptor` each call (the shader module + bind
/// group layouts it borrows are `Copy` references, so this is cheap —
/// no cloning of GPU resources, just re-assembling the descriptor
/// struct with a different `depth_stencil` field).
pub(crate) fn build_off_test_pair<'a>(
    device: &wgpu::Device,
    make: impl Fn(Option<wgpu::DepthStencilState>) -> wgpu::RenderPipelineDescriptor<'a>,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let off = device.create_render_pipeline(&make(None));
    let test = device.create_render_pipeline(&make(Some(stencil_test_state())));
    (off, test)
}
