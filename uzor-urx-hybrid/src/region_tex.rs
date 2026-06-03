//! Per-region uploaded texture. Lazily created from a CPU pixmap,
//! uploaded once, blitted by the compositor every frame at the
//! region's current transform.

use uzor_urx_cpu::Pixmap;
use uzor_urx_core::region::RegionId;

/// A region's pre-rasterised pixmap uploaded as a `wgpu::Texture`.
/// Lives in the hybrid backend's region-texture map keyed by
/// `RegionId`. Recreated when the source pixmap dimensions change.
pub struct RegionTexture {
    pub region_id:   RegionId,
    pub width:       u32,
    pub height:      u32,
    pub texture:     wgpu::Texture,
    pub view:        wgpu::TextureView,
    pub bytes:       u64,
}

impl RegionTexture {
    /// Create + upload a new region texture from a CPU pixmap.
    /// The texture format is `Rgba8UnormSrgb` (or `Rgba8Unorm` — see
    /// the format choice below) and usage includes `TEXTURE_BINDING`
    /// + `COPY_DST`.
    pub fn new(
        device:    &wgpu::Device,
        queue:     &wgpu::Queue,
        region_id: RegionId,
        pixmap:    &Pixmap,
    ) -> Self {
        let width  = pixmap.width();
        let height = pixmap.height();
        let bytes  = (width as u64) * (height as u64) * 4;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx-hybrid-region"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Unorm (not Srgb) — the pixmap is already in non-linear
            // 8-bit space; sRGB conversion in the sampler would
            // double-convert. Caller's compose shader can output sRGB
            // if the surface format is Srgb.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage:  wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload immediately.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixmap.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width:  width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { region_id, width, height, texture, view, bytes }
    }

    /// Replace the texture's contents from a new pixmap. If dimensions
    /// match, reuses the existing texture (cheap `queue.write_texture`).
    /// If dimensions differ, returns `Err` — caller should drop this
    /// entry and create a new one via `new`.
    pub fn replace_contents(
        &mut self,
        queue:  &wgpu::Queue,
        pixmap: &Pixmap,
    ) -> Result<(), ResizeNeeded> {
        if pixmap.width() != self.width || pixmap.height() != self.height {
            return Err(ResizeNeeded);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixmap.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width:  self.width.max(1),
                height: self.height.max(1),
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }
}

/// Returned by `replace_contents` when the new pixmap's dimensions
/// don't match the existing texture; caller must drop + recreate.
#[derive(Debug, Clone, Copy)]
pub struct ResizeNeeded;
