//! Headless wgpu device + offscreen texture + pixel readback.
//!
//! Confirms HybridBackend.composite actually produces pixels when fed
//! real GPU. Marked `#[ignore]` so CI without a GPU adapter doesn't
//! fail; run with `cargo test --test headless -- --ignored --nocapture`.

use uzor_urx_core::region::RegionId;
use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::{HybridBackend, QuadInstance};

const W: u32 = 80;
const H: u32 = 40;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-hybrid-test-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue))
}

fn read_offscreen(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32, height: u32,
) -> Vec<u8> {
    // 256-aligned stride.
    let bytes_per_pixel = 4;
    let unpadded_bpr = width * bytes_per_pixel;
    let padded_bpr = (unpadded_bpr + 255) & !255;
    let buf_size = (padded_bpr * height) as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx-readback"),
        size: buf_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture, mip_level: 0, origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bpr),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let mut out = vec![0u8; (unpadded_bpr * height) as usize];
    for row in 0..height as usize {
        let src = &data[row * padded_bpr as usize .. row * padded_bpr as usize + unpadded_bpr as usize];
        let dst = &mut out[row * unpadded_bpr as usize .. (row + 1) * unpadded_bpr as usize];
        dst.copy_from_slice(src);
    }
    drop(data);
    buffer.unmap();
    out
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn hybrid_composite_paints_a_red_quad() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => {
            eprintln!("no wgpu adapter — skipping");
            return;
        }
    };

    // 1) CPU rasterise a red 20×10 pixmap.
    let mut region_pixmap = Pixmap::new(20, 10);
    region_pixmap.fill([255, 0, 0, 255]);

    // 2) Upload into HybridBackend.
    let mut backend = HybridBackend::new();
    let id = RegionId(42);
    backend.upsert_region_pixmap(&device, &queue, id, &region_pixmap);

    // 3) Create an offscreen target.
    let target_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx-test-target"),
        size:  wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    // Clear target first (LoadOp::Clear) so composite has known bg.
    {
        let _rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }

    // Composite the red region at (10, 5)..(30, 15).
    let instances = vec![(id, QuadInstance::new(10.0, 5.0, 20.0, 10.0))];
    backend.composite(
        &device, &queue, &mut enc, &target_view,
        wgpu::TextureFormat::Rgba8Unorm, W, H, &instances,
    );

    queue.submit(Some(enc.finish()));

    let pixels = read_offscreen(&device, &queue, &target_tex, W, H);

    // Sample dead-centre of the composited region: pixel (20, 10).
    let i = ((10 * W + 20) * 4) as usize;
    let c = &pixels[i..i+4];
    assert!(c[0] > 200, "centre of red quad must be red, got {:?}", c);
    assert!(c[3] > 200, "centre must be opaque, got {:?}", c);
    // Pixel outside the quad — must be cleared-black.
    let i_bg = ((1 * W + 1) * 4) as usize;
    let bg = &pixels[i_bg..i_bg+4];
    assert!(bg[0] < 30, "outside should be black, got {:?}", bg);
}
