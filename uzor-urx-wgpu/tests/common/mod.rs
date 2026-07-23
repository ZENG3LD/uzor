//! Shared headless-GPU test plumbing for the pixel-parity harness.
//!
//! `init_device`/`readback_rgba` are copied verbatim (same wgpu-29 API
//! shape) from `uzor-urx-3d/tests/render_to_texture.rs:19-104`.
//! `dump_png` is new — it un-premultiplies for human eyeballing only;
//! the numeric comparator in `tests/parity.rs` never touches
//! un-premultiplied bytes.

use std::path::Path;

/// Headless wgpu device — `compatible_surface: None`, no window. Returns
/// `None` (never panics) when no adapter is available so callers can
/// skip gracefully on GPU-less CI/dev boxes.
pub fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("uzor-urx-wgpu-parity-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()
}

/// Read back a `RENDER_ATTACHMENT | COPY_SRC` texture as a tight
/// (no row padding) `Vec<u8>` of RGBA8 bytes — premultiplied, exactly
/// as written by the render pass (no format conversion).
pub fn readback_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let aligned_stride = (width * 4 + 255) & !255;
    let buf_size = (aligned_stride * height) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uzor-urx-wgpu-parity-readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned_stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv()
        .expect("map_async callback channel closed before firing")
        .expect("staging buffer map failed");
    let raw = slice.get_mapped_range();
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height as usize {
        let row_start = row * aligned_stride as usize;
        let row_end = row_start + (width * 4) as usize;
        out.extend_from_slice(&raw[row_start..row_end]);
    }
    drop(raw);
    staging.unmap();
    out
}

/// Write `premul_rgba` (premultiplied RGBA8, tightly packed) to a PNG
/// at `path`, un-premultiplying first — display-only convenience so a
/// human can eyeball the dumped frame without it looking artificially
/// dark. The numeric parity comparator always compares the raw
/// premultiplied bytes directly, never this unpremultiplied copy.
pub fn dump_png(path: &Path, width: u32, height: u32, premul_rgba: &[u8]) {
    let mut straight = vec![0u8; premul_rgba.len()];
    for (src, dst) in premul_rgba.chunks_exact(4).zip(straight.chunks_exact_mut(4)) {
        let a = src[3];
        if a == 0 {
            dst.copy_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let unmul = |c: u8| -> u8 {
            ((c as u32 * 255 + (a as u32) / 2) / (a as u32)).min(255) as u8
        };
        dst[0] = unmul(src[0]);
        dst[1] = unmul(src[1]);
        dst[2] = unmul(src[2]);
        dst[3] = a;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = image::save_buffer(path, &straight, width, height, image::ColorType::Rgba8) {
        eprintln!("dump_png: failed to write {}: {e}", path.display());
    }
}
