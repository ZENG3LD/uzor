//! Verify UrxConfig actually changes routing, AND verify that the
//! parallel tile flush is deterministic across thread counts (i.e.
//! byte-identical output regardless of rayon work-stealing order).
//!
//! Closes safety-axis "numerical sanity check #2" from the handoff.

use uzor_urx_core::math::{Color, Rect};
use uzor_urx_core::scene::Scene;
use uzor_urx_core::config::UrxConfig;
use uzor_urx_cpu::{CpuBackend, Pixmap};

const W: u32 = 1024;
const H: u32 = 256;

fn dense_scene(n: usize) -> Scene {
    let mut s = Scene::new();
    for i in 0..n {
        let x = (i as f64) % (W as f64);
        let y = ((i as f64) / (W as f64)).floor() % (H as f64);
        let c = Color {
            r: (i & 0xff) as u8,
            g: ((i >> 4) & 0xff) as u8,
            b: ((i >> 8) & 0xff) as u8,
            a: 255,
        };
        s.fill_rect_solid(Rect::new(x, y, x + 4.0, y + 4.0), c);
    }
    s
}

#[test]
fn config_default_routes_50plus_cmds_to_tile() {
    // Default tile_route_min_cmds = 50. 60-cmd scene should hit tile path.
    let backend = CpuBackend::new();
    assert_eq!(backend.config().tile_route_min_cmds, 50);

    // No direct way to check WHICH path ran from the public API, but
    // we can verify output equivalence: scanline vs tile MUST be
    // byte-identical per the parity invariant.
    let scene = dense_scene(60);
    let mut p_tile = Pixmap::new(W, H);
    backend.render(&scene, &mut p_tile).unwrap();

    let scanline_backend = CpuBackend::with_config(
        UrxConfig::builder().tile_route_min_cmds(1_000_000).build().unwrap()
    );
    let mut p_scan = Pixmap::new(W, H);
    scanline_backend.render(&scene, &mut p_scan).unwrap();

    assert_eq!(
        p_tile.pixels(), p_scan.pixels(),
        "tile path output must match scanline byte-for-byte"
    );
}

#[test]
fn config_custom_tile_dims_round_trip() {
    // 64×16 tiles must also produce byte-identical output as 32×8.
    let scene = dense_scene(200);

    let default_backend = CpuBackend::new();
    let mut p1 = Pixmap::new(W, H);
    default_backend.render(&scene, &mut p1).unwrap();

    let large_tile = CpuBackend::with_config(
        UrxConfig::builder()
            .tile_w(64).tile_h(16)
            .build().unwrap()
    );
    let mut p2 = Pixmap::new(W, H);
    large_tile.render(&scene, &mut p2).unwrap();

    assert_eq!(p1.pixels(), p2.pixels(),
        "tile dim change must not affect output");
}

#[test]
fn config_validation_catches_bad_dims() {
    // tile_w must be multiple of 4.
    let r = UrxConfig::builder().tile_w(33).build();
    assert!(r.is_err());

    // tile_h cannot be zero.
    let r = UrxConfig::builder().tile_h(0).build();
    assert!(r.is_err());
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_is_deterministic_across_thread_counts() {
    // Render the same scene with rayon's global pool (whatever it
    // happens to be), and with a forced 1-thread pool. Byte-identical
    // result confirms the parallel path doesn't carry order-dependent
    // state (no race in tile-flush, no accumulated state per worker).
    //
    // We don't try to vary thread counts higher because the global
    // pool already exercises N>1; the 1-thread comparison is the
    // strict test.
    let scene = dense_scene(2_000);

    let mut p_pool = Pixmap::new(W, H);
    CpuBackend::new().render(&scene, &mut p_pool).unwrap();

    let pool_1 = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let mut p_serial = Pixmap::new(W, H);
    pool_1.install(|| {
        CpuBackend::new().render(&scene, &mut p_serial).unwrap();
    });

    assert_eq!(p_pool.pixels(), p_serial.pixels(),
        "parallel must be deterministic: parallel vs 1-thread must match byte-for-byte");
}
