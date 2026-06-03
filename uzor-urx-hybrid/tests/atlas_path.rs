//! Atlas-packed region path (Hybrid-P1). When
//! `UrxConfig::hybrid_atlas_enabled = true`, small regions live in a
//! shared atlas texture; standalone-texture path remains for large
//! regions and as fallback.
//!
//! GPU-gated (--ignored). Also includes a couple of non-GPU logic
//! tests for the atlas allocator.

use uzor_urx_core::region::RegionId;
use uzor_urx_core::config::UrxConfig;
use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::{HybridBackend, QuadInstance, RegionAtlas, AtlasUpsertResult};

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-atlas-test"),
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
    backend:  &mut HybridBackend,
    device:   &wgpu::Device,
    queue:    &wgpu::Queue,
    instances: &[(RegionId, QuadInstance)],
) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size:  wgpu::Extent3d { width: 256, height: 128, depth_or_array_layers: 1 },
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
        wgpu::TextureFormat::Rgba8Unorm, 256, 128, instances);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn small_region_lands_in_atlas_when_enabled() {
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);
    let id = RegionId(1);

    let mut pm = Pixmap::new(64, 64);
    pm.fill([200, 100, 50, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);

    // Verify: region is atlas-packed (not standalone).
    assert!(backend.is_atlas_packed(id),
        "64×64 region should be atlas-packed with atlas enabled");
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn oversize_region_falls_back_to_standalone() {
    let Some((device, queue)) = init_device() else { return; };
    // Default atlas is 2048×2048; half-size cap = 1024. Region of
    // 1500×1500 should not fit (each side > 1024).
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);
    let id = RegionId(2);

    let mut pm = Pixmap::new(1500, 100);
    pm.fill([50, 100, 200, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);

    // Each-side ≤ half: 1500 > 1024 → reject → standalone.
    assert!(!backend.is_atlas_packed(id), "wide region must NOT fit atlas");
    assert_eq!(backend.region_count(), 1, "must be cached standalone");
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn atlas_disabled_default_uses_standalone() {
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new(); // default config — atlas off
    let id = RegionId(3);

    let mut pm = Pixmap::new(32, 32);
    pm.fill([10, 20, 30, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);

    assert!(!backend.is_atlas_packed(id),
        "default config: atlas off, region should be standalone");
    assert_eq!(backend.region_count(), 1);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn atlas_dirty_skip_matches_hash() {
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);
    let id = RegionId(4);

    let mut pm = Pixmap::new(48, 48);
    pm.fill([99, 99, 99, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);
    let slot_after_first = *backend.atlas_slot(id).expect("must be atlas-packed");

    // Same content → hash match → skip.
    backend.upsert_region_pixmap(&device, &queue, id, &pm);
    let slot_after_second = *backend.atlas_slot(id).unwrap();
    assert_eq!(slot_after_first.content_hash, slot_after_second.content_hash);
    assert_eq!(slot_after_first.px_rect, slot_after_second.px_rect,
        "no re-allocation on identical content");
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn atlas_generation_skip_works() {
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);
    let id = RegionId(5);

    let mut pm = Pixmap::new(32, 32);
    pm.fill([1, 2, 3, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    assert!(backend.is_atlas_packed(id));
    assert_eq!(backend.last_uploaded_generation(id), Some(1));

    // Same gen → ZERO work.
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    assert_eq!(backend.last_uploaded_generation(id), Some(1));

    // Bump gen → write happens.
    pm.fill([9, 8, 7, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 2);
    assert_eq!(backend.last_uploaded_generation(id), Some(2));
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn composite_works_with_mixed_atlas_and_standalone() {
    // Mix: one atlas-resident region + one standalone (oversize).
    // composite must dispatch both via the same call, picking the
    // correct view + UV remap for each.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    let small_id = RegionId(10);
    let large_id = RegionId(11);

    let mut small_pm = Pixmap::new(64, 64);
    small_pm.fill([255, 0, 0, 255]);
    let mut large_pm = Pixmap::new(1500, 64);
    large_pm.fill([0, 255, 0, 255]);

    backend.upsert_region_pixmap(&device, &queue, small_id, &small_pm);
    backend.upsert_region_pixmap(&device, &queue, large_id, &large_pm);
    assert!(backend.is_atlas_packed(small_id));
    assert!(!backend.is_atlas_packed(large_id));

    // composite both — must not panic.
    let instances = vec![
        (small_id, QuadInstance::new(0.0, 0.0, 64.0, 64.0)),
        (large_id, QuadInstance::new(64.0, 0.0, 192.0, 64.0)),
    ];
    run_composite(&mut backend, &device, &queue, &instances);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn instanced_composite_coalesces_atlas_run() {
    // With hybrid_instanced_composite enabled + 5 atlas-resident
    // regions, composite() should emit ONE draw call (range 0..5).
    // Verifies the run-coalescing branch — atlas regions share a
    // single bind group.
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_instanced_composite(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    // Upload 5 small atlas-eligible regions.
    let mut pm = Pixmap::new(32, 32);
    pm.fill([10, 20, 30, 255]);
    let mut instances = Vec::with_capacity(5);
    for i in 0..5u64 {
        let id = RegionId(100 + i);
        backend.upsert_region_pixmap(&device, &queue, id, &pm);
        instances.push((id, QuadInstance::new((i as f32) * 32.0, 0.0, 32.0, 32.0)));
    }
    // All 5 must be atlas-packed.
    for (id, _) in &instances {
        assert!(backend.is_atlas_packed(*id));
    }

    // Composite — must not panic.
    run_composite(&mut backend, &device, &queue, &instances);
    // No structural assertion possible without a metrics recorder
    // hooked up; the counter increment is verified by the
    // KEY_HYBRID_COMPOSITE_PASS_DRAWS in metrics_keys (read via
    // UrxRecorder in apps).
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn coalesce_disabled_per_region_draw() {
    // hybrid_atlas_enabled = true but hybrid_instanced_composite = false:
    // atlas-resident regions still get per-region bind groups (so the
    // coalesce branch can be A/B'd against the non-coalesced path).
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_instanced_composite(false) // explicit
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    let mut pm = Pixmap::new(16, 16);
    pm.fill([200, 50, 50, 255]);
    let mut instances = Vec::with_capacity(3);
    for i in 0..3u64 {
        let id = RegionId(200 + i);
        backend.upsert_region_pixmap(&device, &queue, id, &pm);
        instances.push((id, QuadInstance::new((i as f32) * 16.0, 0.0, 16.0, 16.0)));
    }
    run_composite(&mut backend, &device, &queue, &instances);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn coalesce_breaks_at_standalone_in_middle() {
    // Mix: [atlas, atlas, standalone, atlas, atlas]
    // With coalesce enabled, runs should be:
    //   [0..2 atlas] [2..3 standalone] [3..5 atlas]
    // = 3 draws (vs 5 without coalesce).
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .hybrid_instanced_composite(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);

    let mut small = Pixmap::new(32, 32);
    small.fill([10, 10, 10, 255]);
    let mut large = Pixmap::new(1500, 32);
    large.fill([200, 200, 200, 255]);

    let mut instances = Vec::with_capacity(5);
    // Two atlas regions first.
    for i in 0..2u64 {
        let id = RegionId(300 + i);
        backend.upsert_region_pixmap(&device, &queue, id, &small);
        instances.push((id, QuadInstance::new((i as f32) * 32.0, 0.0, 32.0, 32.0)));
        assert!(backend.is_atlas_packed(id));
    }
    // One standalone (oversize).
    let standalone_id = RegionId(400);
    backend.upsert_region_pixmap(&device, &queue, standalone_id, &large);
    instances.push((standalone_id, QuadInstance::new(64.0, 0.0, 96.0, 32.0)));
    assert!(!backend.is_atlas_packed(standalone_id));
    // Two more atlas regions.
    for i in 0..2u64 {
        let id = RegionId(500 + i);
        backend.upsert_region_pixmap(&device, &queue, id, &small);
        instances.push((id, QuadInstance::new(160.0 + (i as f32) * 32.0, 0.0, 32.0, 32.0)));
        assert!(backend.is_atlas_packed(id));
    }

    // composite — must paint everything in painter's order.
    run_composite(&mut backend, &device, &queue, &instances);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn remove_region_clears_atlas_slot() {
    let Some((device, queue)) = init_device() else { return; };
    let cfg = UrxConfig::builder()
        .hybrid_atlas_enabled(true)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);
    let id = RegionId(7);

    let mut pm = Pixmap::new(32, 32);
    pm.fill([100, 100, 100, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);
    assert!(backend.is_atlas_packed(id));

    backend.remove_region(id);
    assert!(!backend.is_atlas_packed(id),
        "remove_region must free the atlas slot");
}

// ── Non-GPU logic tests ───────────────────────────────────────────────

#[test]
fn atlas_upsert_result_variants_are_pattern_matchable() {
    // Just a structural test that AtlasUpsertResult exists and has
    // the expected variants — guards against accidental breaking
    // changes to the public API.
    fn classify(r: AtlasUpsertResult) -> &'static str {
        match r {
            AtlasUpsertResult::Uploaded { .. } => "uploaded",
            AtlasUpsertResult::Skipped { .. } => "skipped",
            AtlasUpsertResult::Reject => "reject",
        }
    }
    assert_eq!(classify(AtlasUpsertResult::Reject), "reject");
}

// Silence unused-import lint when not running --ignored GPU tests.
#[allow(dead_code)]
fn _ensure_imports() {
    let _ = std::mem::size_of::<RegionAtlas>();
}
