//! Texture struct + simple atlas/registry for Wave 5.
//!
//! Texture3D wraps a `wgpu::Texture` + view + sampler. Apps construct
//! it via `Texture3D::from_rgba8(device, queue, width, height, pixels)`
//! and pass an `Arc<Texture3D>` to a `Node` through
//! `NodeMesh::Textured`. The renderer keeps a per-frame cache so the
//! same `Arc<Texture3D>` reuses one bind group across draws.

use std::collections::HashMap;
use std::sync::Arc;

const EVICT_AFTER_FRAMES: u32 = 240;

pub struct Texture3D {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

impl Texture3D {
    /// Create an RGBA8 texture from CPU pixels (4 bytes/pixel, row-major,
    /// length = width*height*4).
    pub fn from_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Self {
        assert_eq!(
            pixels.len(),
            (width * height * 4) as usize,
            "rgba8 pixel buffer size mismatch"
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx3d.tex.rgba8"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("urx3d.tex.sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Self { texture, view, sampler, width, height }
    }

    /// 64×64 procedural checkerboard atlas (RGB rainbow on bright cells).
    /// Useful default for demos and tests without shipping an image.
    pub fn checkerboard(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let w = 64;
        let h = 64;
        let mut px = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let bright = ((x / 8) ^ (y / 8)) & 1 == 0;
                if bright {
                    // Rainbow stripe along the diagonal
                    let t = ((x + y) % 64) as f32 / 64.0;
                    let r = (t * 255.0) as u8;
                    let g = ((1.0 - (t - 0.5).abs() * 2.0) * 255.0) as u8;
                    let b = ((1.0 - t) * 255.0) as u8;
                    px[i] = r;
                    px[i + 1] = g;
                    px[i + 2] = b;
                    px[i + 3] = 255;
                } else {
                    px[i + 3] = 255;
                }
            }
        }
        Self::from_rgba8(device, queue, w, h, &px)
    }
}

#[derive(Eq, PartialEq, Hash, Copy, Clone)]
struct TexKey(*const Texture3D);
unsafe impl Send for TexKey {}
unsafe impl Sync for TexKey {}

pub struct TextureCacheEntry {
    pub bind_group: wgpu::BindGroup,
    last_touched: u32,
}

pub struct TextureCache {
    entries: HashMap<TexKey, TextureCacheEntry>,
    frame: u32,
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), frame: 0 }
    }

    pub fn begin_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.entries.retain(|_, e| {
            let age = self.frame.wrapping_sub(e.last_touched);
            age < EVICT_AFTER_FRAMES
        });
    }

    pub fn get_or_create<F>(
        &mut self,
        tex: &Arc<Texture3D>,
        build: F,
    ) -> &wgpu::BindGroup
    where
        F: FnOnce(&Texture3D) -> wgpu::BindGroup,
    {
        let key = TexKey(Arc::as_ptr(tex));
        if !self.entries.contains_key(&key) {
            let bg = build(tex);
            self.entries.insert(
                key,
                TextureCacheEntry { bind_group: bg, last_touched: self.frame },
            );
        }
        let entry = self.entries.get_mut(&key).unwrap();
        entry.last_touched = self.frame;
        &entry.bind_group
    }
}
