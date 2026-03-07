//! GPU-backed glyph atlas for text rendering.
//!
//! `GlyphAtlas` maintains a single `R8Unorm` wgpu texture that stores
//! rasterized glyph bitmaps.  Rectangle packing is done by `etagere`'s
//! `BucketedAtlasAllocator`.  Glyph rasterization is delegated to
//! `cosmic_text::SwashCache`.

use std::collections::HashMap;

use cosmic_text::{CacheKey, FontSystem, SwashCache, SwashContent};
use etagere::{size2, BucketedAtlasAllocator};

/// UV coordinates and placement metadata for a single glyph in the atlas.
#[derive(Clone, Copy, Debug)]
pub struct GlyphEntry {
    /// UV x of the glyph's top-left corner in the atlas (0.0–1.0).
    pub uv_x: f32,
    /// UV y of the glyph's top-left corner in the atlas (0.0–1.0).
    pub uv_y: f32,
    /// UV width of the glyph in the atlas (0.0–1.0).
    pub uv_w: f32,
    /// UV height of the glyph in the atlas (0.0–1.0).
    pub uv_h: f32,
    /// Pixel width of the glyph bitmap.
    pub width: u32,
    /// Pixel height of the glyph bitmap.
    pub height: u32,
    /// Horizontal bearing (distance from origin to left edge) in pixels.
    pub placement_left: i32,
    /// Vertical bearing (distance from baseline to top edge) in pixels.
    pub placement_top: i32,
}

/// A GPU-backed glyph atlas using a single `R8Unorm` texture.
///
/// Glyphs are rasterized on demand by `SwashCache` and packed into the
/// texture by `BucketedAtlasAllocator`.  Each glyph entry stores UV
/// coordinates and placement metrics for use by the glyph instance shader.
pub struct GlyphAtlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    allocator: BucketedAtlasAllocator,
    cache: HashMap<CacheKey, GlyphEntry>,
    width: u32,
    height: u32,
}

impl GlyphAtlas {
    /// Create a new atlas backed by a `width × height` `R8Unorm` texture.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let allocator = BucketedAtlasAllocator::new(size2(width as i32, height as i32));

        Self {
            texture,
            view,
            sampler,
            allocator,
            cache: HashMap::new(),
            width,
            height,
        }
    }

    /// Return the `TextureView` for use in bind groups.
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Look up or rasterize a glyph into the atlas.
    ///
    /// Returns `None` for whitespace glyphs, zero-size bitmaps, or allocation
    /// failures (atlas full).
    pub fn get_or_insert(
        &mut self,
        cache_key: CacheKey,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<GlyphEntry> {
        // Fast path: glyph already in cache.
        if let Some(entry) = self.cache.get(&cache_key) {
            return Some(*entry);
        }

        // Rasterize the glyph via SwashCache (uncached — we maintain our own
        // atlas-level cache keyed by CacheKey).
        let image = swash_cache.get_image_uncached(font_system, cache_key)?;

        let glyph_w = image.placement.width;
        let glyph_h = image.placement.height;
        if glyph_w == 0 || glyph_h == 0 {
            return None;
        }

        // Convert whatever content type to a single-channel alpha bitmap.
        let alpha_data: Vec<u8> = match image.content {
            SwashContent::Mask => image.data.clone(),
            SwashContent::Color => {
                // RGBA — take the alpha channel.
                image
                    .data
                    .chunks(4)
                    .map(|c| c.get(3).copied().unwrap_or(255))
                    .collect()
            }
            SwashContent::SubpixelMask => {
                // RGB subpixel — average to a single luminance value.
                image
                    .data
                    .chunks(3)
                    .map(|c| {
                        let r = c.get(0).copied().unwrap_or(0) as u16;
                        let g = c.get(1).copied().unwrap_or(0) as u16;
                        let b = c.get(2).copied().unwrap_or(0) as u16;
                        ((r + g + b) / 3) as u8
                    })
                    .collect()
            }
        };

        // Allocate a slot in the atlas (1-pixel padding on each side to prevent
        // bilinear bleed between neighbouring glyphs).
        let padded_w = glyph_w + 2;
        let padded_h = glyph_h + 2;
        let alloc = self
            .allocator
            .allocate(size2(padded_w as i32, padded_h as i32))?;

        // The actual glyph starts 1 px inside the padded allocation.
        let atlas_x = alloc.rectangle.min.x as u32 + 1;
        let atlas_y = alloc.rectangle.min.y as u32 + 1;

        // Upload the single-channel bitmap to the atlas texture.
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: atlas_x,
                    y: atlas_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &alpha_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(glyph_w),
                rows_per_image: Some(glyph_h),
            },
            wgpu::Extent3d {
                width: glyph_w,
                height: glyph_h,
                depth_or_array_layers: 1,
            },
        );

        let entry = GlyphEntry {
            uv_x: atlas_x as f32 / self.width as f32,
            uv_y: atlas_y as f32 / self.height as f32,
            uv_w: glyph_w as f32 / self.width as f32,
            uv_h: glyph_h as f32 / self.height as f32,
            width: glyph_w,
            height: glyph_h,
            placement_left: image.placement.left,
            placement_top: image.placement.top,
        };

        self.cache.insert(cache_key, entry);
        Some(entry)
    }

    /// Drop the in-memory glyph cache.
    ///
    /// The texture contents are unchanged; call this when the atlas is recreated.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.allocator = BucketedAtlasAllocator::new(size2(self.width as i32, self.height as i32));
    }
}
