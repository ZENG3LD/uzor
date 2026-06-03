//! Verify the transform-only frame proxy: composite() runs without
//! ANY prior upload → backend marks it as a transform-only frame +
//! emits KEY_HYBRID_COMPOSITE_TRANSFORM_ONLY_FRAMES.
//!
//! This is the "smooth animation = zero CPU raster + zero GPU upload"
//! signal that lets diagnostics show whether the consumer is using
//! the hybrid path correctly.
//!
//! GPU-gated (--ignored).

use uzor_urx_core::region::RegionId;
use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::{HybridBackend, QuadInstance};

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-transform-only-test"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue))
}

fn make_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("urx-target"),
        size:  wgpu::Extent3d { width: 64, height: 32, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn run_composite(
    backend:  &mut HybridBackend,
    device:   &wgpu::Device,
    queue:    &wgpu::Queue,
    instances: &[(RegionId, QuadInstance)],
) {
    let (_, view) = make_target(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
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
    backend.composite(device, queue, &mut enc, &view,
        wgpu::TextureFormat::Rgba8Unorm, 64, 32, instances);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn uploads_counter_tracks_actual_writes() {
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(1);

    assert_eq!(backend.uploads_since_last_composite(), 0);

    // First upsert at gen=1 → counter = 1.
    let mut pm = Pixmap::new(16, 16);
    pm.fill([100, 50, 25, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    assert_eq!(backend.uploads_since_last_composite(), 1);

    // Same gen → skip → counter unchanged.
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    assert_eq!(backend.uploads_since_last_composite(), 1);

    // New content gen → counter = 2.
    pm.fill([200, 200, 200, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 2);
    assert_eq!(backend.uploads_since_last_composite(), 2);

    // composite() → counter resets.
    let instances = vec![(id, QuadInstance::new(0.0, 0.0, 16.0, 16.0))];
    run_composite(&mut backend, &device, &queue, &instances);
    assert_eq!(backend.uploads_since_last_composite(), 0);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn transform_only_frame_after_initial_upload() {
    // The canonical scenario: a panel animates (slides across screen)
    // with no content change. Frame 1: upload. Frame 2..N: only
    // composite with new dst rect. Each of those frames is
    // "transform-only" — they consume zero CPU raster + zero GPU upload.
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(7);

    let mut pm = Pixmap::new(32, 32);
    pm.fill([100, 100, 100, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);

    // Frame 1: composite. NOT transform-only — we uploaded above.
    run_composite(&mut backend, &device, &queue,
        &[(id, QuadInstance::new(0.0, 0.0, 32.0, 32.0))]);
    assert_eq!(backend.uploads_since_last_composite(), 0,
        "composite must have reset the counter");

    // Frame 2: composite at NEW dst rect, no upload between frames.
    // This IS a transform-only frame.
    run_composite(&mut backend, &device, &queue,
        &[(id, QuadInstance::new(10.0, 5.0, 32.0, 32.0))]);
    assert_eq!(backend.uploads_since_last_composite(), 0);

    // Frame 3: same. Still transform-only.
    run_composite(&mut backend, &device, &queue,
        &[(id, QuadInstance::new(20.0, 10.0, 32.0, 32.0).with_tint(0.5, 0.5, 0.5, 1.0))]);
    assert_eq!(backend.uploads_since_last_composite(), 0);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn skipped_upsert_doesnt_count_as_upload() {
    // upsert_region_with_generation with matching gen short-circuits
    // BEFORE the upload happens — must NOT bump the counter.
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(13);

    let mut pm = Pixmap::new(8, 8);
    pm.fill([50, 50, 50, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    // Reset via composite.
    run_composite(&mut backend, &device, &queue,
        &[(id, QuadInstance::new(0.0, 0.0, 8.0, 8.0))]);
    assert_eq!(backend.uploads_since_last_composite(), 0);

    // 10× attempts at same gen — all should be skipped.
    for _ in 0..10 {
        backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    }
    assert_eq!(backend.uploads_since_last_composite(), 0,
        "10 skipped upserts must not increment the counter");

    // composite without uploads → transform-only frame.
    run_composite(&mut backend, &device, &queue,
        &[(id, QuadInstance::new(5.0, 5.0, 8.0, 8.0))]);
    assert_eq!(backend.uploads_since_last_composite(), 0);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn quad_instance_builders_compose() {
    // QuadInstance::new + chain with_dst/with_uv/with_tint produces
    // expected struct without modifying other fields.
    let q = QuadInstance::new(1.0, 2.0, 3.0, 4.0);
    assert_eq!(q.dst, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(q.uv, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(q.tint, [1.0, 1.0, 1.0, 1.0]);

    let q2 = q.with_dst(10.0, 20.0, 30.0, 40.0);
    assert_eq!(q2.dst, [10.0, 20.0, 30.0, 40.0]);
    assert_eq!(q2.uv, [0.0, 0.0, 1.0, 1.0]); // unchanged

    let q3 = q2.with_uv(0.25, 0.5, 0.75, 1.0).with_tint(0.5, 0.5, 0.5, 0.8);
    assert_eq!(q3.uv, [0.25, 0.5, 0.75, 1.0]);
    assert_eq!(q3.tint, [0.5, 0.5, 0.5, 0.8]);
    assert_eq!(q3.dst, [10.0, 20.0, 30.0, 40.0]); // chained from q2
}
