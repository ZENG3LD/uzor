//! Stencil offscreen target for the rounded-clip mechanism (URX Wave 3
//! Commit 2 design §2.1,
//! `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`).
//!
//! Mirrors `msaa::MsaaTarget`'s EXACT-match-on-any-size-change lifecycle
//! from day one — Wave 1 Commit 3 discovered a grow-only variant of
//! this shape is a resolve-size-mismatch bug waiting to happen (see
//! `msaa.rs`'s own module doc); this type starts from that fix, not
//! before it, since it's new code with no "grow-only" history to
//! correct.
//!
//! One genuine asymmetry with `MsaaTarget`: there is no `resolve_target`
//! concept for a stencil attachment. Stencil values are consumed
//! IN-PASS by the stencil test on every draw call, never sampled as a
//! texture afterward, so there is nothing to resolve — a multisampled
//! `Stencil8` attachment is perfectly valid on its own, no companion
//! single-sample buffer needed (unlike color, which must resolve to be
//! presentable/sampleable elsewhere).
//!
//! **Wave 3 Commit 3**: this type is no longer root-only — each
//! `renderer::BlendLayerTarget` owns its OWN `StencilTarget` sibling
//! (design §3.4 Risk 4), `ensure`'d independently at that layer's own
//! viewport-matched size whenever a frame is armed. `Load`/`Store`
//! (not `Clear`/`Discard`) apply across a PAUSE of a layer's pass (a
//! nested `PushLayer` opening deeper), so a layer's accumulated
//! rounded-clip nesting depth survives being paused and resumed —
//! `Clear(0)` only ever happens on that specific target's FIRST open
//! (`renderer::OpenKind::First`), matching the "fresh all-zero buffer
//! is the base case" invariant this type's `ensure` doc comment
//! already establishes for the root.
pub(crate) struct StencilTarget {
    view: Option<wgpu::TextureView>,
    width: u32,
    height: u32,
    sample_count: u32,
}

impl StencilTarget {
    /// `sample_count` MUST equal the color MSAA target's own sample
    /// count (design §2.1) — both attachments of the same pass share
    /// exactly one sample count; there is no per-attachment override in
    /// `wgpu`'s pass model.
    pub(crate) fn new(sample_count: u32) -> Self {
        Self { view: None, width: 0, height: 0, sample_count }
    }

    /// Ensure a `Stencil8` target exists that is EXACTLY `width x
    /// height` (reallocates on any size change, not just growth — same
    /// discipline as `MsaaTarget::ensure`). Unlike `MsaaTarget`, this
    /// has NO `sample_count <= 1` early-out — a stencil buffer is
    /// needed whenever a frame is armed (`EncodedFrame::has_rounded_clip`),
    /// regardless of whether color MSAA is active. Only called when a
    /// frame actually needs one (design §2.5's frame-wide arming
    /// decision) — a frame with no rounded clip never calls this at
    /// all, so no texture is ever allocated for the common case.
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if self.view.is_some() && w == self.width && h == self.height {
            return;
        }
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_urx_wgpu.native_stencil"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Stencil8,
            // Never sampled (see this module's doc comment) — pure
            // render-attachment usage.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.view = Some(tex.create_view(&wgpu::TextureViewDescriptor::default()));
        self.width = w;
        self.height = h;
    }

    /// Borrow the stencil view for this frame's render pass. Only ever
    /// called immediately after `ensure`, when a frame is armed — the
    /// `expect` documents a renderer-internal invariant (mirrors
    /// `MsaaTarget::color_view`'s own doc-comment-as-invariant style),
    /// not a caller-reachable condition.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        self.view
            .as_ref()
            .expect("StencilTarget::ensure must run before view() — only call when a frame is armed")
    }
}
