//! LRU eviction in RegionAtlas — verifies that filling the atlas
//! beyond capacity triggers eviction of the least-recently-used slot
//! instead of silently rejecting + falling back to standalone path.
//!
//! GPU-gated (--ignored). Atlas needs a real wgpu::Device for the
//! texture allocation.

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
            label: Some("urx-atlas-lru-test"),
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
fn lru_evicts_oldest_when_atlas_fills() {
    // Tiny atlas (256×256) + many 128×128 regions = ~4 fit. The 5th
    // upsert must evict the LRU region instead of returning Reject.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(256)
        .hybrid_atlas_h(256)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // 128×128 regions, atlas is 256×256 → fits up to 4 in 2×2 grid.
    let mut pm = Pixmap::new(128, 128);
    pm.fill([100, 100, 100, 255]);

    // Upload 4 — all should land in the atlas.
    for i in 0..4u64 {
        backend.upsert_region_pixmap(&device, &queue, RegionId(i), &pm);
        assert!(backend.is_atlas_packed(RegionId(i)),
            "region {} should be atlas-packed", i);
    }

    // Upload a 5th. Atlas full → LRU eviction → 5th gets in, oldest gets out.
    // Shelf allocator may need to evict more than one slot if the freed
    // shelf doesn't fit the new alloc — verify the LRU ordering (id=0 goes
    // first), but allow multiple evictions if the allocator needed them.
    backend.upsert_region_pixmap(&device, &queue, RegionId(99), &pm);
    assert!(backend.is_atlas_packed(RegionId(99)),
        "new region must land in atlas after LRU eviction");

    // The OLDEST (id=0) must have been evicted first (LRU contract).
    assert!(!backend.is_atlas_packed(RegionId(0)),
        "oldest region (id=0) must have been evicted first under LRU");

    // The newest pre-fill (id=3) must still be in (it's the MRU before the
    // 99 upsert, so LRU never gets to it).
    assert!(backend.is_atlas_packed(RegionId(3)),
        "id=3 (MRU before final upsert) must survive");
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn composite_touch_keeps_static_panel_alive() {
    // Static panel composited every frame must survive 10 fresh
    // dynamic uploads of other regions filling the atlas.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(256)
        .hybrid_atlas_h(256)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // 128×128 fits 4 in a 256×256 atlas.
    let mut pm = Pixmap::new(128, 128);
    pm.fill([255, 0, 0, 255]);

    // Upload the static panel (id=1000) first.
    backend.upsert_region_pixmap(&device, &queue, RegionId(1000), &pm);
    assert!(backend.is_atlas_packed(RegionId(1000)));

    // Upload 3 more so atlas is full.
    for i in 0..3u64 {
        backend.upsert_region_pixmap(&device, &queue, RegionId(i), &pm);
    }

    // Now composite the static panel every "frame". Touch promotes it
    // to MRU, so subsequent dynamic uploads evict id=0/1/2, not 1000.
    for tick in 0..10u64 {
        // Composite the static panel.
        let instances = vec![(RegionId(1000), QuadInstance::new(0.0, 0.0, 32.0, 32.0))];
        run_composite(&mut backend, &device, &queue, &instances);

        // Upload a fresh dynamic region — forces eviction.
        backend.upsert_region_pixmap(&device, &queue, RegionId(2000 + tick), &pm);
    }

    // The static panel must STILL be atlas-packed after 10 evictions.
    assert!(backend.is_atlas_packed(RegionId(1000)),
        "static panel must survive: LRU+touch promoted it on each composite");
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn no_eviction_if_upsert_replaces_existing_slot() {
    // Re-upserting an existing region (same dims, different content)
    // must NOT count as an eviction — it's an in-place re-write.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(256)
        .hybrid_atlas_h(256)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    let mut pm = Pixmap::new(64, 64);
    pm.fill([100, 100, 100, 255]);

    backend.upsert_region_pixmap(&device, &queue, RegionId(1), &pm);
    let slot0 = *backend.atlas_slot(RegionId(1)).unwrap();

    // Different content, same dims.
    pm.fill([200, 200, 200, 255]);
    backend.upsert_region_pixmap(&device, &queue, RegionId(1), &pm);
    let slot1 = *backend.atlas_slot(RegionId(1)).unwrap();

    // The allocation in the atlas should be reused — same px_rect.
    assert_eq!(slot0.px_rect, slot1.px_rect,
        "same-dim re-upsert must reuse the existing allocation");
    // content_hash must have changed.
    assert_ne!(slot0.content_hash, slot1.content_hash);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn oversize_after_eviction_still_rejects() {
    // A region too large for the empty atlas must still get Reject
    // even after LRU evicts everything.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_atlas_w(256)
        .hybrid_atlas_h(256)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // 200×200 region with 256×256 atlas → fits predicate fails
    // (each-side ≤ atlas/2 = 128, so 200 fails). Falls back to
    // standalone immediately.
    let mut pm = Pixmap::new(200, 200);
    pm.fill([50, 50, 50, 255]);
    backend.upsert_region_pixmap(&device, &queue, RegionId(1), &pm);

    assert!(!backend.is_atlas_packed(RegionId(1)),
        "oversize must go standalone, not atlas, regardless of LRU");
}
