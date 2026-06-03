//! Tests: UrxConfig wired into UrxEngine at construction.

use uzor_urx_core::config::UrxConfig;
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_core::math::Rect;
use uzor_urx_engine::{Backend, RenderCadence, UrxEngine};

#[test]
fn new_with_config_sets_cache_budget() {
    let config = UrxConfig::builder()
        .region_cache_budget_bytes(4096)
        .build()
        .unwrap();
    let engine = UrxEngine::new_with_config(Backend::Cpu, 800, 600, config);
    assert_eq!(engine.config().region_cache_budget_bytes, 4096);
}

#[test]
fn default_constructor_uses_default_config() {
    let engine = UrxEngine::new(Backend::Cpu, 800, 600);
    assert_eq!(engine.config().region_cache_budget_bytes, 64 << 20);
}

#[test]
fn cache_evicts_under_tight_budget() {
    // 4096-byte budget: one 32×32 region = 4096 bytes exactly at the
    // limit, so inserting a second will evict the first.
    let config = UrxConfig::builder()
        .region_cache_budget_bytes(4096)
        .build()
        .unwrap();
    let mut engine = UrxEngine::new_cpu_with_config(800, 600, config);

    // Insert two 32×32 Static regions. Each pixmap = 32*32*4 = 4096 bytes.
    // With a 4096-byte budget the second insert must evict the first.
    for i in 0u32..2 {
        let id    = RegionId(i as u64);
        let x     = (i * 40) as f64;
        engine.upsert_region(
            id,
            Scene::default(),
            Rect::new(x, 0.0, x + 32.0, 32.0),
            RenderCadence::Static,
        );
    }

    // Render so the engine rasterises + caches each region.
    use uzor_urx_cpu::Pixmap;
    use uzor_urx_engine::RenderTarget;
    let mut pixmap = Pixmap::new(800, 600);
    engine.render(RenderTarget::Cpu(&mut pixmap)).unwrap();

    // After the second region is cached the budget evicts the first.
    // Total bytes in cache must stay ≤ budget.
    assert!(
        engine.cache_bytes() <= 4096,
        "cache_bytes={} exceeds budget=4096",
        engine.cache_bytes()
    );
    // At most one entry survives.
    assert!(
        engine.cache_count() <= 1,
        "cache_count={} should be ≤ 1 under tight budget",
        engine.cache_count()
    );
}
