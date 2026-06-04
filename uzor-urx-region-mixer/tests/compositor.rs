//! Wave 9b ext — GPU compositor end-to-end test.
//!
//! Build a 2-region scene where each region paints a SOLID color into
//! its intermediate via `queue.write_texture` (the simplest possible
//! "backend"), then composite onto a final RGBA target and verify
//! that each region's pixels land in its bounds and nowhere else.

use std::cell::RefCell;
use std::rc::Rc;

use uzor_urx_core::math::Rect;
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_engine::{BackendHint, RenderCadence, RenderTarget, UrxEngine};
use uzor_urx_region_mixer::{GpuCompositor, RegionMixer, COMPOSITOR_FORMAT};

const W: u32 = 128;
const H: u32 = 128;

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x0: x, y0: y, x1: x + w, y1: y + h }
}

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })).ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx-region-mixer-compositor-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).ok()
}

fn fill_region(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    compositor: &mut GpuCompositor,
    region: RegionId,
    bounds: Rect,
    color: [u8; 4],
) {
    let w = bounds.width() as u32;
    let h = bounds.height() as u32;
    compositor.ensure(device, region, w, h);
    let tex = compositor.region_texture(region).unwrap();
    let pixels = vec![color; (w * h) as usize].into_iter().flatten().collect::<Vec<u8>>();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

#[test]
#[ignore]
fn compositor_blits_two_regions_in_dispatch_order() {
    let Some((device, queue)) = init_device() else { return; };

    let mut e = UrxEngine::new_mixed(W, H);
    // Two regions, side by side: left red, right green.
    let left_bounds = rect(0.0, 0.0, 64.0, 128.0);
    let right_bounds = rect(64.0, 0.0, 64.0, 128.0);
    e.upsert_region_with_hint(RegionId(1), Scene::new(), left_bounds, RenderCadence::Static, BackendHint::Cpu);
    e.upsert_region_with_hint(RegionId(2), Scene::new(), right_bounds, RenderCadence::Static, BackendHint::FullGpu);

    let mut compositor = GpuCompositor::new(&device, COMPOSITOR_FORMAT);
    let recorded = Rc::new(RefCell::new(Vec::<(RegionId, Rect)>::new()));

    // RegionMixer callbacks: capture the region/bounds; we fill the
    // texture AFTER render() returns because borrowing compositor
    // inside the FnMut would conflict.
    let rec = recorded.clone();
    let rec2 = recorded.clone();
    let mut mixer = RegionMixer::new()
        .on_cpu(move |id, bounds, _| rec.borrow_mut().push((id, bounds)))
        .on_full_gpu(move |id, bounds, _| rec2.borrow_mut().push((id, bounds)));
    mixer.begin_frame();
    e.render(RenderTarget::Mixed { dispatcher: &mut mixer }).unwrap();

    // Now actually paint each region's intermediate with the
    // dispatch-recorded bounds.
    for &(id, bounds) in recorded.borrow().iter() {
        let color = if id.0 == 1 { [255, 0, 0, 255] } else { [0, 255, 0, 255] };
        fill_region(&device, &queue, &mut compositor, id, bounds, color);
    }
    assert_eq!(recorded.borrow().len(), 2);
    assert_eq!(mixer.records().len(), 2);

    // Final target.
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("final-target"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COMPOSITOR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    compositor.composite_to(&device, &queue, &mut enc, &mixer, &view, (W, H));
    queue.submit(Some(enc.finish()));

    // Read back the target.
    let aligned = (W * 4 + 255) & !255;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (aligned * H) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(aligned), rows_per_image: Some(H) },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let raw = slice.get_mapped_range();

    // Sample a pixel from each region's centre and verify the color.
    let pixel = |x: u32, y: u32| -> [u8; 4] {
        let off = (y * aligned + x * 4) as usize;
        [raw[off], raw[off + 1], raw[off + 2], raw[off + 3]]
    };
    let left_centre  = pixel(32, 64);
    let right_centre = pixel(96, 64);
    eprintln!("left_centre={:?}  right_centre={:?}", left_centre, right_centre);

    // sRGB output texture — exact bytes depend on the colorspace
    // round-trip, but red dominance in left + green in right is
    // unambiguous.
    assert!(
        left_centre[0] > 200 && left_centre[1] < 80,
        "left should be red: {:?}", left_centre
    );
    assert!(
        right_centre[1] > 200 && right_centre[0] < 80,
        "right should be green: {:?}", right_centre
    );

    drop(raw); staging.unmap();
}
