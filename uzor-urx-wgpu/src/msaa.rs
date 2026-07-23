//! MSAA offscreen color target.
//!
//! ## Commit 3 correction (resolve-target size-mismatch bug)
//!
//! Commit 1/2 described this target as "grow-only" (never shrinks),
//! matching the Wave 1 design brief's prose description of
//! `uzor-urx-3d`'s MSAA lifecycle. Commit 3's resize-sanity test
//! (render 64x64 -> 512x512 -> 64x64 on one renderer instance) exposed
//! that this was wrong: `wgpu`/WebGPU require a multisampled color
//! attachment and its `resolve_target` to have IDENTICAL width/height
//! — resolve is a per-pixel operation, not a scaling blit. A grow-only
//! MSAA buffer that stays at 512x512 after the caller shrinks back to
//! a 64x64 destination view is a validation error (or undefined
//! behaviour), not just wasted memory.
//!
//! Re-reading `uzor-urx-3d::Renderer3D::resize` directly (not just the
//! design brief's prose) confirms it actually recreates its MSAA
//! targets whenever `size != self.depth_size` — an EXACT-match
//! recreate, not grow-only. This module now matches that real
//! behaviour: `ensure` reallocates whenever the requested size
//! CHANGES (grows OR shrinks), never leaving a stale larger-than-
//! requested buffer around. Still gated on `sample_count > 1` and
//! still a no-op when the requested size repeats (the common per-frame
//! case), so ordinary rendering pays no extra cost — only an actual
//! resize reallocates, exactly like the cited precedent.

/// Owns the (optional) multisampled color target `NativeUrxRenderer`
/// resolves into the caller's view every frame. `sample_count <= 1`
/// means MSAA is disabled — `ensure` becomes a no-op and the renderer
/// draws straight into the caller's view instead.
pub(crate) struct MsaaTarget {
    view: Option<wgpu::TextureView>,
    width: u32,
    height: u32,
    sample_count: u32,
    format: wgpu::TextureFormat,
}

impl MsaaTarget {
    pub(crate) fn new(sample_count: u32, format: wgpu::TextureFormat) -> Self {
        Self { view: None, width: 0, height: 0, sample_count, format }
    }

    /// Ensure a color target exists that is EXACTLY `width x height`
    /// (reallocates on any size change, not just growth — see this
    /// module's doc comment). No-op when MSAA is disabled
    /// (`sample_count <= 1`) or the requested size repeats the current
    /// allocation.
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.sample_count <= 1 {
            return;
        }
        let w = width.max(1);
        let h = height.max(1);
        if self.view.is_some() && w == self.width && h == self.height {
            return;
        }
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_urx_wgpu.native_msaa_color"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.view = Some(tex.create_view(&wgpu::TextureViewDescriptor::default()));
        self.width = w;
        self.height = h;
    }

    /// Borrow the MSAA color view for this frame's render pass.
    ///
    /// Only ever called from `render_into_encoder` at `sample_count >
    /// 1`, immediately after `ensure` — the `expect` documents a
    /// renderer-internal invariant, not a caller-reachable condition.
    pub(crate) fn color_view(&self) -> &wgpu::TextureView {
        self.view
            .as_ref()
            .expect("MsaaTarget::ensure must run before color_view at sample_count > 1")
    }
}
