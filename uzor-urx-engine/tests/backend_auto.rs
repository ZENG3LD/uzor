//! Backend::auto heuristic: pure-fn, no GPU init.

use uzor_urx_engine::engine::{Backend, WorkloadHint};

#[test]
fn no_gpu_always_cpu() {
    let h = WorkloadHint {
        region_count: 100,
        total_pixels: 10_000_000,
        gpu_available: false,
        ..Default::default()
    };
    assert_eq!(Backend::auto(h), Backend::Cpu);
}

#[test]
fn tiny_workload_stays_cpu_even_with_gpu() {
    let h = WorkloadHint {
        region_count: 2,
        total_pixels: 5_000,
        gpu_available: true,
        ..Default::default()
    };
    assert_eq!(Backend::auto(h), Backend::Cpu);
}

#[cfg(feature = "hybrid-backend")]
#[test]
fn retained_low_hz_with_gpu_goes_hybrid() {
    let h = WorkloadHint {
        region_count: 16,
        total_pixels: 2_000_000,
        retained: true,
        high_hz: false,
        gpu_available: true,
        ..Default::default()
    };
    assert_eq!(Backend::auto(h), Backend::Hybrid);
}

#[cfg(feature = "hybrid-backend")]
#[test]
fn unified_memory_pulls_to_hybrid_regardless() {
    // Even high-Hz huge area: unified memory removes the upload cost.
    let h = WorkloadHint {
        region_count: 32,
        total_pixels: 5_000_000,
        high_hz: true,
        retained: false,
        gpu_available: true,
        unified_memory: true,
        ..Default::default()
    };
    assert_eq!(Backend::auto(h), Backend::Hybrid);
}

#[cfg(all(feature = "wgpu-backend", not(feature = "hybrid-backend")))]
#[test]
fn hybrid_off_falls_through_to_wgpu() {
    let h = WorkloadHint {
        region_count: 16,
        total_pixels: 5_000_000,
        gpu_available: true,
        ..Default::default()
    };
    assert_eq!(Backend::auto(h), Backend::Wgpu);
}

#[test]
fn default_hint_no_gpu_is_cpu() {
    assert_eq!(Backend::auto(WorkloadHint::default()), Backend::Cpu);
}
