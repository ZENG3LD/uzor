//! Hybrid dirty-skip: identical content via `upsert_region_pixmap`
//! must NOT trigger a re-upload; content_hash check is the cheapest
//! mechanism. With `upsert_region_with_generation`, the caller can
//! skip even the hash pass.
//!
//! Marked `#[ignore]` like other GPU-touching tests in this crate;
//! run with `cargo test --test dirty_skip -- --ignored --nocapture`.

use std::sync::atomic::{AtomicU32, Ordering};
use uzor_urx_core::region::RegionId;
use uzor_urx_core::config::{UrxConfig, DirtyStrategy};
use uzor_urx_cpu::Pixmap;
use uzor_urx_hybrid::HybridBackend;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-dirty-skip-test"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    Some((device, queue))
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn identical_pixmap_skips_upload_via_hash() {
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(7);

    // First upload — counted.
    let mut pm = Pixmap::new(32, 32);
    pm.fill([100, 200, 50, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);
    let bytes_after_first = backend.region_bytes();

    // Same content → no new texture allocation, no re-upload.
    backend.upsert_region_pixmap(&device, &queue, id, &pm);
    assert_eq!(backend.region_bytes(), bytes_after_first,
        "no new region should have been created");

    // Modify one byte → hash diverges → upload happens (no panic).
    pm.set_pixel(5, 5, [0, 0, 255, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn generation_path_skips_even_hash() {
    let Some((device, queue)) = init_device() else { return; };
    // Switch strategy off so we don't ALSO go through hash by accident.
    let cfg = UrxConfig::builder()
        .hybrid_dirty_strategy(DirtyStrategy::GenerationOnly)
        .build()
        .unwrap();
    let mut backend = HybridBackend::with_config(cfg);
    let id = RegionId(13);

    let mut pm = Pixmap::new(16, 16);
    pm.fill([10, 20, 30, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);

    // Re-call with SAME generation. Even though pixmap pointer is the
    // same, the gen-only path should bail before the hash.
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);

    // Bump generation → goes through upload.
    pm.fill([50, 60, 70, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 2);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn dirty_skip_doesnt_corrupt_subsequent_resizes() {
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(99);

    // 32×32 region.
    let mut pm32 = Pixmap::new(32, 32);
    pm32.fill([200, 200, 200, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm32);

    // Resize to 16×16 (different dims) — replace_contents would fail
    // with ResizeNeeded; backend must create a new texture.
    let mut pm16 = Pixmap::new(16, 16);
    pm16.fill([50, 50, 50, 255]);
    backend.upsert_region_pixmap(&device, &queue, id, &pm16);

    assert_eq!(backend.region_count(), 1);
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn last_uploaded_generation_reports_correctly() {
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(101);

    // No upload yet → None.
    assert_eq!(backend.last_uploaded_generation(id), None);
    assert!(!backend.is_region_clean_at(id, 0));

    // First upload at gen=1.
    let mut pm = Pixmap::new(8, 8);
    pm.fill([100, 100, 100, 255]);
    backend.upsert_region_with_generation(&device, &queue, id, &pm, 1);
    assert_eq!(backend.last_uploaded_generation(id), Some(1));
    assert!(backend.is_region_clean_at(id, 1));
    assert!(!backend.is_region_clean_at(id, 2));

    // Re-tag clean at gen=5 without uploading — no Pixmap touched.
    assert!(backend.mark_clean_with_generation(id, 5));
    assert_eq!(backend.last_uploaded_generation(id), Some(5));
    assert!(backend.is_region_clean_at(id, 5));

    // Marking clean on a missing region → false.
    assert!(!backend.mark_clean_with_generation(RegionId(999), 1));
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn upload_if_dirty_skips_raster_fn_on_clean_path() {
    // The critical test: when gen is unchanged, raster_fn must NEVER
    // be invoked. This saves the consumer's CPU rasterisation cost.
    let Some((device, queue)) = init_device() else { return; };
    let mut backend = HybridBackend::new();
    let id = RegionId(202);

    let raster_calls = AtomicU32::new(0);
    let make_pixmap = |seed: u8| -> Pixmap {
        raster_calls.fetch_add(1, Ordering::SeqCst);
        let mut pm = Pixmap::new(16, 16);
        pm.fill([seed, seed, seed, 255]);
        pm
    };

    // First call at gen=1 — raster_fn MUST be invoked.
    let did_upload = backend.upload_if_dirty(&device, &queue, id, 1, || make_pixmap(50));
    assert!(did_upload);
    assert_eq!(raster_calls.load(Ordering::SeqCst), 1);

    // Second call at gen=1 — raster_fn MUST NOT be invoked.
    let did_upload = backend.upload_if_dirty(&device, &queue, id, 1, || make_pixmap(200));
    assert!(!did_upload, "upload_if_dirty must skip on matching generation");
    assert_eq!(raster_calls.load(Ordering::SeqCst), 1,
        "raster_fn must NOT have been called on the clean path");

    // gen=2 — raster_fn fires.
    let did_upload = backend.upload_if_dirty(&device, &queue, id, 2, || make_pixmap(99));
    assert!(did_upload);
    assert_eq!(raster_calls.load(Ordering::SeqCst), 2);

    // gen=2 again — raster_fn skipped again.
    let did_upload = backend.upload_if_dirty(&device, &queue, id, 2, || make_pixmap(99));
    assert!(!did_upload);
    assert_eq!(raster_calls.load(Ordering::SeqCst), 2);
}
