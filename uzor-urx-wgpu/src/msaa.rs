//! Grow-only MSAA offscreen color target.
//!
//! Shape matches `uzor-urx-3d`'s `create_msaa_targets`/`resize` (see
//! `uzor-urx-3d/src/pipeline.rs:1866-1939`): one `RENDER_ATTACHMENT`
//! color texture at a fixed `sample_count`, freed/regrown only when the
//! requested size grows past the current allocation — never shrinks,
//! so window-resize jitter doesn't churn allocations. No depth/stencil
//! target: the native 2D pipelines never z-test.

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

    /// Ensure a color target exists that covers at least `width x
    /// height`. No-op when MSAA is disabled (`sample_count <= 1`) or
    /// the current allocation already covers the request.
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.sample_count <= 1 {
            return;
        }
        if self.view.is_some() && width <= self.width && height <= self.height {
            return;
        }
        let w = width.max(self.width).max(1);
        let h = height.max(self.height).max(1);
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
