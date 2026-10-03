//! GPU framebuffer readback + PNG encode (moved/trimmed from `uzor-desktop`).
//!
//! Keeps `capture_screenshot`, `capture_screenshot_texture`, `encode_png`.
//! File-system save paths (`screenshot_save_dir`, timestamps) are deleted
//! (L-D2): the host answers `WindowCommand::Screenshot` with PNG bytes only.

use vello::util::RenderSurface;
use vello::wgpu;

/// Recreate `surface.target_texture` with `COPY_SRC` and `RENDER_ATTACHMENT`.
pub fn add_copy_src_to_target_texture(surface: &mut RenderSurface<'_>, device: &wgpu::Device) {
    let old = &surface.target_texture;
    let size = old.size();

    let new_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target_texture_with_copy_src"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
}

/// Synchronous GPU readback of a vello render surface.
///
/// Returns raw RGBA pixels (after optional crop) and `(width, height)`, or
/// `None` on failure. `crop` is `Some((x, y, w, h))` in texture pixels.
pub fn capture_screenshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface: &RenderSurface<'_>,
    crop: Option<(u32, u32, u32, u32)>,
) -> Option<(Vec<u8>, u32, u32)> {
    capture_screenshot_texture(device, queue, &surface.target_texture, crop)
}

/// Synchronous GPU readback of an arbitrary texture.
pub fn capture_screenshot_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    crop: Option<(u32, u32, u32, u32)>,
) -> Option<(Vec<u8>, u32, u32)> {
    let size = texture.size();
    let full_width = size.width;
    let full_height = size.height;

    if full_width == 0 || full_height == 0 {
        log::warn!("screenshot: texture has zero dimension ({full_width}x{full_height})");
        return None;
    }

    let bytes_per_pixel = 4u32;
    let unpadded_bytes_per_row = full_width * bytes_per_pixel;
    const ALIGNMENT: u32 = 256;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(ALIGNMENT) * ALIGNMENT;
    let buffer_size = u64::from(padded_bytes_per_row * full_height);

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot_staging_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("screenshot_copy_encoder"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(full_height),
            },
        },
        wgpu::Extent3d {
            width: full_width,
            height: full_height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), wgpu::BufferAsyncError>>();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });

    loop {
        if let Err(e) = device.poll(wgpu::PollType::Poll) {
            log::warn!("screenshot: device poll error: {e:?}");
            return None;
        }
        match rx.try_recv() {
            Ok(Ok(())) => break,
            Ok(Err(e)) => {
                log::warn!("screenshot: buffer map error: {e}");
                return None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => std::hint::spin_loop(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                log::warn!("screenshot: map channel disconnected");
                return None;
            }
        }
    }

    let data = buffer_slice.get_mapped_range();
    let mut full_pixels: Vec<u8> =
        Vec::with_capacity((full_width * full_height * bytes_per_pixel) as usize);

    for row in 0..full_height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        full_pixels.extend_from_slice(&data[start..end]);
    }

    drop(data);
    staging_buffer.unmap();

    let (pixels, out_width, out_height) = if let Some((cx, cy, cw, ch)) = crop {
        let cx = cx.min(full_width);
        let cy = cy.min(full_height);
        let cw = cw.min(full_width - cx);
        let ch = ch.min(full_height - cy);
        if cw == 0 || ch == 0 {
            (full_pixels, full_width, full_height)
        } else {
            let mut cropped = Vec::with_capacity((cw * ch * bytes_per_pixel) as usize);
            for row in cy..(cy + ch) {
                let start = ((row * full_width + cx) * bytes_per_pixel) as usize;
                let end = start + (cw * bytes_per_pixel) as usize;
                cropped.extend_from_slice(&full_pixels[start..end]);
            }
            (cropped, cw, ch)
        }
    } else {
        (full_pixels, full_width, full_height)
    };

    Some((pixels, out_width, out_height))
}

/// Encode raw RGBA8 pixels to PNG bytes.
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = match encoder.write_header() {
            Ok(w) => w,
            Err(e) => {
                log::warn!("screenshot: PNG header error: {e}");
                return None;
            }
        };
        if let Err(e) = writer.write_image_data(pixels) {
            log::warn!("screenshot: PNG write error: {e}");
            return None;
        }
    }
    Some(png_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_png_roundtrips_tiny() {
        let pixels = [255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 255];
        let png = encode_png(&pixels, 2, 2).expect("encode");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
