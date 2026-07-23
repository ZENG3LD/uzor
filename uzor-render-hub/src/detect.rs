//! Backend auto-detection from a wgpu adapter.
//!
//! Logic copied verbatim from mlc `chart-app-vello/src/main.rs`.

use serde::{Deserialize, Serialize};

use crate::backend::RenderBackend;

/// Per-backend performance defaults (fps target + MSAA).
#[derive(Debug, Clone, Copy)]
pub struct PerfDefaults {
    /// Target frames per second.
    pub fps_limit: u32,
    /// MSAA sample count (1 = disabled, 4/8/16 = enabled).
    pub msaa_samples: u8,
}

/// Pick a [`RenderBackend`] from wgpu adapter info.
///
/// Match arms copied verbatim from mlc.
pub fn detect_backend(info: &wgpu::AdapterInfo) -> RenderBackend {
    match info.device_type {
        wgpu::DeviceType::DiscreteGpu   => RenderBackend::VelloGpu,
        wgpu::DeviceType::IntegratedGpu => RenderBackend::VelloGpu,
        wgpu::DeviceType::VirtualGpu    => RenderBackend::VelloCpu,
        wgpu::DeviceType::Cpu           => RenderBackend::TinySkia,
        _                               => RenderBackend::VelloGpu,
    }
}

/// URX-family autodetect decision tree (Wave 6 Commit 2,
/// `urx-wave6-autodetect-cutover-design-2026-07-25.md` §5.1) — lives
/// ALONGSIDE [`detect_backend`], which stays the one `RenderHub::autodetect`
/// actually calls until the flip commit (Wave 6 Commit 4). **Not called
/// by anything yet** — a `git revert`-able, one-line-call-site-swap-away
/// second named function, not a runtime feature flag (the project's
/// hard-cutover doctrine rejects permanent flags for exactly this kind
/// of transition). `pub fn` in a `pub mod` — the crate's own public API
/// surface, so this needs no `#[allow(dead_code)]`: an unreferenced-
/// internally `pub` item in a library crate is never flagged
/// `dead_code` by rustc (it's reachable from any external consumer of
/// this crate, whether or not anything inside the crate calls it yet).
///
/// Mapping vs [`detect_backend`]: `DiscreteGpu`/`IntegratedGpu` (was
/// `VelloGpu`) → [`RenderBackend::UrxWgpu`] — the Wave 6 Commit 1
/// gate-verified native GPU path. `VirtualGpu` (was `VelloCpu`) →
/// [`RenderBackend::UrxCpu`] — `VirtualGpu` denotes a software/
/// virtualized adapter, functionally CPU-class despite the GPU-shaped
/// enum name; the brief's "CPU-only → urx-cpu" arm covers it directly
/// (design §8 risk 1: this specific remap is unverified live on an
/// actual `VirtualGpu`-reporting box as of this pass — verify on a VM/CI
/// runner before trusting broadly). `Cpu` (was `TinySkia`) →
/// [`RenderBackend::UrxCpu`]. Unknown/`_` (was `VelloGpu`, an optimistic
/// default) → [`RenderBackend::UrxWgpu`], keeping the same "assume
/// GPU-capable" policy the old function had for that fallthrough arm.
pub fn detect_backend_urx(info: &wgpu::AdapterInfo) -> RenderBackend {
    match info.device_type {
        wgpu::DeviceType::DiscreteGpu   => RenderBackend::UrxWgpu,
        wgpu::DeviceType::IntegratedGpu => RenderBackend::UrxWgpu,
        wgpu::DeviceType::VirtualGpu    => RenderBackend::UrxCpu,
        wgpu::DeviceType::Cpu           => RenderBackend::UrxCpu,
        _                                => RenderBackend::UrxWgpu,
    }
}

/// Per-backend performance defaults. FPS values copied verbatim from mlc.
///
/// MSAA defaults are `0` (= vello `AaConfig::Area`) across the board: the
/// default GPU factory compiles vello shaders area-only
/// (`factories.rs` — `AaSupport { area: true, msaa8: false, msaa16: false }`),
/// so a non-zero default made every vello-gpu app panic at first submit
/// ("shaders not configured to support AA mode: msaa8") unless it carried a
/// `.msaa(0)` workaround. Explicit MSAA remains available via
/// `RenderControl::set_msaa_samples` for apps that install an
/// `AaSupport::all()` factory.
pub fn default_perf(backend: RenderBackend) -> PerfDefaults {
    let (fps_limit, msaa_samples) = match backend {
        RenderBackend::VelloGpu      => (120u32, 0u8),
        RenderBackend::VelloCpu      => (30,     0),
        RenderBackend::TinySkia      => (90,     0),
        RenderBackend::InstancedWgpu => (90,     0),
        RenderBackend::VelloHybrid   => (90,     0),
        RenderBackend::Canvas2d      => (60,     0),
        // URX family — mirrors Vello-equivalent defaults until real benches
        // give us numbers.
        RenderBackend::UrxCpu        => (30,     0),
        RenderBackend::UrxWgpu       => (90,     0),
        RenderBackend::UrxHybrid     => (90,     0),
        RenderBackend::UrxWgpuFull   => (120,    0),
    };
    PerfDefaults { fps_limit, msaa_samples }
}

/// Coarse-grained recommendation: GPU vs CPU vs fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedBackend {
    VelloGpu,
    VelloCpu,
    TinySkia,
}

/// GPU info extracted from a wgpu adapter.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub driver: String,
    pub device_type: wgpu::DeviceType,
    pub recommended: RecommendedBackend,
    pub backend: RenderBackend,
}

/// Detect GPU + recommend a backend.
pub fn detect(info: &wgpu::AdapterInfo) -> GpuInfo {
    let backend = detect_backend(info);
    let recommended = match info.device_type {
        wgpu::DeviceType::DiscreteGpu   => RecommendedBackend::VelloGpu,
        wgpu::DeviceType::IntegratedGpu => RecommendedBackend::VelloGpu,
        wgpu::DeviceType::VirtualGpu    => RecommendedBackend::VelloCpu,
        wgpu::DeviceType::Cpu           => RecommendedBackend::TinySkia,
        _                               => RecommendedBackend::VelloGpu,
    };
    GpuInfo {
        name: info.name.clone(),
        driver: info.driver.clone(),
        device_type: info.device_type,
        recommended,
        backend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(device_type: wgpu::DeviceType) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "test-adapter".to_string(),
            vendor: 0,
            device: 0,
            device_type,
            device_pci_bus_id: String::new(),
            driver: String::new(),
            driver_info: String::new(),
            backend: wgpu::Backend::Noop,
            subgroup_min_size: 0,
            subgroup_max_size: 0,
            transient_saves_memory: false,
        }
    }

    /// Wave 6 Commit 2 — every `detect_backend_urx` arm (design §5.1),
    /// covering all 5 `wgpu::DeviceType` variants (the 5th, `Other`,
    /// exercises the `_` fallthrough).
    #[test]
    fn discrete_and_integrated_gpu_map_to_urx_wgpu() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::DiscreteGpu)), RenderBackend::UrxWgpu);
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::IntegratedGpu)), RenderBackend::UrxWgpu);
    }

    #[test]
    fn virtual_gpu_and_cpu_map_to_urx_cpu() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::VirtualGpu)), RenderBackend::UrxCpu);
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::Cpu)), RenderBackend::UrxCpu);
    }

    #[test]
    fn unknown_device_type_falls_through_to_urx_wgpu() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::Other)), RenderBackend::UrxWgpu);
    }

    /// `detect_backend_urx` must never return a non-URX variant — the
    /// whole point of this decision tree is that autodetect, once
    /// flipped onto it, only ever lands on `UrxCpu`/`UrxWgpu`.
    #[test]
    fn never_returns_a_non_urx_backend() {
        for dt in [
            wgpu::DeviceType::DiscreteGpu,
            wgpu::DeviceType::IntegratedGpu,
            wgpu::DeviceType::VirtualGpu,
            wgpu::DeviceType::Cpu,
            wgpu::DeviceType::Other,
        ] {
            let backend = detect_backend_urx(&info(dt));
            assert!(
                matches!(backend, RenderBackend::UrxCpu | RenderBackend::UrxWgpu),
                "device_type {dt:?} produced non-URX backend {backend:?}"
            );
        }
    }
}
