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

/// Per-backend performance defaults. Values copied verbatim from mlc.
pub fn default_perf(backend: RenderBackend) -> PerfDefaults {
    let (fps_limit, msaa_samples) = match backend {
        RenderBackend::VelloGpu      => (120u32, 8u8),
        RenderBackend::VelloCpu      => (30,     0),
        RenderBackend::TinySkia      => (90,     8),
        RenderBackend::InstancedWgpu => (90,     8),
        RenderBackend::VelloHybrid   => (90,     8),
    };
    PerfDefaults { fps_limit, msaa_samples }
}

/// Coarse-grained recommendation kept for back-compat with `uzor-autodetect`.
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
