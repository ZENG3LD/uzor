//! GPU framebuffer screenshot pipeline: COPY_SRC patching, GPU readback, PNG encode, OS-conventional save directory.

use vello::util::RenderSurface;
use vello::wgpu;

/// Get current time as milliseconds since Unix epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Build a timestamp string suitable for a filename (YYYYMMDD_HHMMSS).
///
/// Uses `SystemTime` to avoid pulling in the `chrono` crate.
pub fn timestamp_for_filename() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Simple decomposition: seconds since epoch -> date/time components.
    // Accurate enough for filenames; does not handle leap seconds.
    let seconds_per_minute = 60u64;
    let seconds_per_hour = 3600u64;
    let seconds_per_day = 86400u64;

    let s = secs % seconds_per_minute;
    let m = (secs / seconds_per_minute) % 60;
    let h = (secs / seconds_per_hour) % 24;

    // Days since epoch (1970-01-01)
    let mut days = secs / seconds_per_day;

    // Convert days to year/month/day (Gregorian calendar)
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let months = [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    let mut day = days + 1;
    for (i, &days_in_month) in months.iter().enumerate() {
        let dim = if i == 1 && is_leap_year(year) {
            29
        } else {
            days_in_month
        };
        if day <= dim {
            break;
        }
        day -= dim;
        month += 1;
    }

    format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        year, month, day, h, m, s
    )
}

/// Return whether `year` is a Gregorian leap year.
pub fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

/// Recreate `surface.target_texture` with `COPY_SRC` and `RENDER_ATTACHMENT` added to the usage flags.
///
/// Vello's `create_targets` only sets `STORAGE_BINDING | TEXTURE_BINDING`.
/// Without `COPY_SRC`, `copy_texture_to_buffer` fails; without `RENDER_ATTACHMENT`, the instanced wgpu backend cannot use the texture as a render target.
/// Since all fields on `RenderSurface` are `pub`, we can drop and replace the
/// texture in-place. The view must be recreated from the new texture.
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

/// Perform a synchronous GPU readback of the render texture.
///
/// Returns raw RGBA pixels (after optional crop) and the final `(width, height)`,
/// or `None` on failure. The caller is responsible for PNG encoding and
/// clipboard operations so that both can share the same pixel buffer.
///
/// `crop` is `Some((x, y, w, h))` in texture-pixel coordinates. Coordinates
/// are clamped to the texture boundary to avoid panics on out-of-bounds rects.
pub fn capture_screenshot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface: &RenderSurface<'_>,
    crop: Option<(u32, u32, u32, u32)>,
) -> Option<(Vec<u8>, u32, u32)> {
    let texture = &surface.target_texture;
    let size = texture.size();
    let full_width = size.width;
    let full_height = size.height;

    if full_width == 0 || full_height == 0 {
        eprintln!("[Screenshot] Texture has zero dimension ({full_width}x{full_height})");
        return None;
    }

    let bytes_per_pixel = 4u32; // Rgba8Unorm
    let unpadded_bytes_per_row = full_width * bytes_per_pixel;

    // wgpu requires rows aligned to 256 bytes (COPY_BYTES_PER_ROW_ALIGNMENT)
    const ALIGNMENT: u32 = 256;
    let padded_bytes_per_row =
        unpadded_bytes_per_row.div_ceil(ALIGNMENT) * ALIGNMENT;

    let buffer_size = (padded_bytes_per_row * full_height) as u64;

    // Create a staging buffer for GPU -> CPU transfer
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("screenshot_staging_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Encode the copy command
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

    // Map the staging buffer and wait for completion via a channel
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), wgpu::BufferAsyncError>>();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });

    // Poll until the mapping callback fires
    loop {
        match device.poll(wgpu::PollType::Poll) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[Screenshot] Device poll error: {e:?}");
                return None;
            }
        }
        match rx.try_recv() {
            Ok(Ok(())) => break,
            Ok(Err(e)) => {
                eprintln!("[Screenshot] Buffer map error: {e:?}");
                return None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // Mapping not yet complete; spin and try again
                std::hint::spin_loop();
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                eprintln!("[Screenshot] Map channel disconnected unexpectedly");
                return None;
            }
        }
    }

    // Read the full pixel data (strip row padding)
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

    // Apply optional crop
    let (pixels, out_width, out_height) = if let Some((cx, cy, cw, ch)) = crop {
        // Clamp to texture bounds to avoid panics
        let cx = cx.min(full_width);
        let cy = cy.min(full_height);
        let cw = cw.min(full_width - cx);
        let ch = ch.min(full_height - cy);

        if cw == 0 || ch == 0 {
            eprintln!("[Screenshot] Crop rect is empty after clamping — using full frame");
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

/// Return the directory where screenshots should be saved.
///
/// Prefers `%USERPROFILE%\Pictures\Screenshots` on Windows; falls back to the
/// current working directory so the function always returns a usable path.
pub fn screenshot_save_dir() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let dir = std::path::PathBuf::from(home)
            .join("Pictures")
            .join("Screenshots");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }
    std::env::current_dir().unwrap_or_default()
}

/// Encode raw RGBA pixels to PNG bytes.
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Option<Vec<u8>> {
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = match encoder.write_header() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[Screenshot] PNG header error: {e}");
                return None;
            }
        };
        if let Err(e) = writer.write_image_data(pixels) {
            eprintln!("[Screenshot] PNG write error: {e}");
            return None;
        }
        // writer (and encoder) drop here, releasing the borrow on png_bytes
    }
    Some(png_bytes)
}
