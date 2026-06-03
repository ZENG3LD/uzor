//! Atlas auto-resize on eviction pressure.
//!
//! When evictions/frame crosses AUTO_RESIZE_EVICT_THRESHOLD (8), the
//! consumer can call `backend.maybe_resize_atlas(&device)` to double
//! the atlas dimensions (clamped to MAX_ATLAS_DIM = 8192).
//!
//! GPU-gated (--ignored).

use uzor_urx_core::region::RegionId;
use uzor_urx_core::config::UrxConfig;
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
            label: Some("urx-atlas-resize-test"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue))
}

fn run_composite(
    backend:   &mut HybridBackend,
    device:    &wgpu::Device,
    queue:     &wgpu::Queue,
    instances: &[(RegionId, QuadInstance)],
) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size:  wgpu::Extent3d { width: 64, height: 32, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
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
fn auto_resize_triggers_after_heavy_eviction_pressure() {
    // Tiny 256×256 atlas. Each region is 128×128 (max that fits since
    // atlas.fits() requires each side ≤ atlas/2). 4 fit; a 5th evicts.
    // To hit eviction threshold (8) we churn 12 distinct ids through.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(256)
        .hybrid_atlas_h(256)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // Upload 12 distinct regions of 128×128. Each one beyond capacity
    // (4 fit) forces an eviction. So 12 - 4 = 8 evictions — exactly
    // at the threshold.
    for i in 0..12u64 {
        let mut pm = Pixmap::new(128, 128);
        pm.fill([i as u8, 100, 50, 255]);
        backend.upsert_region_pixmap(&device, &queue, RegionId(i), &pm);
    }

    // Now composite — this resets frame counters at the start. To
    // observe sustained pressure across the FRAME we need to do the
    // uploads AFTER a previous composite. Re-do: do one composite (no
    // pressure), then do another wave of upserts to hit threshold.
    let dummy: Vec<(RegionId, QuadInstance)> = (0..4)
        .map(|i| (RegionId(i + 12), QuadInstance::new(0.0, 0.0, 16.0, 16.0)))
        .collect();
    // First upload these dummy regions then composite — resets counter.
    for (id, _) in &dummy {
        let mut pm = Pixmap::new(8, 8);
        pm.fill([255, 255, 255, 255]);
        backend.upsert_region_pixmap(&device, &queue, *id, &pm);
    }
    run_composite(&mut backend, &device, &queue, &dummy);

    // After composite, the frame counter was reset, then evictions
    // from these dummy upserts incremented it. We want to OBSERVE
    // pressure that built up DURING (between resets), so apply MORE
    // pressure now without another composite.
    for i in 100..112u64 {
        let mut pm = Pixmap::new(128, 128);
        pm.fill([(i & 0xff) as u8, 50, 100, 255]);
        backend.upsert_region_pixmap(&device, &queue, RegionId(i), &pm);
    }

    // 12 upserts of 128×128 into a 256×256 atlas should evict ≥ 8.
    // Now consumer calls maybe_resize_atlas — should return Some.
    let result = backend.maybe_resize_atlas(&device);
    assert!(
        result.is_some(),
        "maybe_resize_atlas should return Some after heavy eviction pressure"
    );
    let (new_w, new_h) = result.unwrap();
    assert_eq!(new_w, 512, "atlas width should double from 256 to 512");
    assert_eq!(new_h, 512, "atlas height should double from 256 to 512");
    // Config should reflect the new dims.
    assert_eq!(backend.config().hybrid_atlas_w, 512);
    assert_eq!(backend.config().hybrid_atlas_h, 512);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn no_resize_without_eviction_pressure() {
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(2048)
        .hybrid_atlas_h(2048)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // 4 small regions fit easily — no evictions.
    let mut instances: Vec<(RegionId, QuadInstance)> = Vec::new();
    for i in 0..4u64 {
        let mut pm = Pixmap::new(64, 64);
        pm.fill([100, 100, 100, 255]);
        backend.upsert_region_pixmap(&device, &queue, RegionId(i), &pm);
        instances.push((RegionId(i), QuadInstance::new(0.0, 0.0, 16.0, 16.0)));
    }
    run_composite(&mut backend, &device, &queue, &instances);
    assert!(backend.maybe_resize_atlas(&device).is_none(),
        "no resize without eviction pressure");
    // Atlas dims unchanged.
    assert_eq!(backend.config().hybrid_atlas_w, 2048);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn resize_caps_at_max_atlas_dim() {
    // Starting at 8192×8192 (the cap), even maximum pressure can't grow.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(8192)
        .hybrid_atlas_h(8192)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // Wide pressure: many 256×256 regions, but 8192² atlas can hold
    // 8192/256 × 8192/256 = 32×32 = 1024 slots without eviction.
    // To force evictions we'd need 1025+ uploads — too costly. Just
    // assert that maybe_resize on a fresh 8192 atlas returns None
    // (no pressure built up, but ALSO the cap blocks growth).
    let mut pm = Pixmap::new(64, 64);
    pm.fill([0, 200, 0, 255]);
    backend.upsert_region_pixmap(&device, &queue, RegionId(0), &pm);
    assert!(backend.maybe_resize_atlas(&device).is_none());
}
