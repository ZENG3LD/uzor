//! GPU auto-detection for uzor applications.
//!
//! Recommends a rendering backend based on wgpu adapter capabilities.

use serde::{Deserialize, Serialize};

// =============================================================================
// Full RenderBackend enum (matches mlc sidebar-content::state::RenderBackend)
// =============================================================================

/// Full set of rendering backends available in uzor applications.
///
/// Copied verbatim from `sidebar_content::state::RenderBackend` in mylittlechart.
/// mlc keeps its own definition until the cutover step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackend {
    VelloGpu,
    InstancedWgpu,
    VelloCpu,
    VelloHybrid,
    TinySkia,
}

/// Per-backend performance defaults.
pub struct PerfDefaults {
    /// Target frames per second.
    pub fps_limit: u32,
    /// MSAA sample count (1 = disabled, 4/8/16 = enabled).
    pub msaa_samples: u8,
}

/// Detect the recommended [`RenderBackend`] from a wgpu adapter.
///
/// Match arms copied verbatim from mlc `chart-app-vello/src/main.rs`.
pub fn detect_backend(adapter_info: &wgpu::AdapterInfo) -> RenderBackend {
    match adapter_info.device_type {
        wgpu::DeviceType::DiscreteGpu => RenderBackend::VelloGpu,
        wgpu::DeviceType::IntegratedGpu => RenderBackend::VelloGpu,
        wgpu::DeviceType::VirtualGpu => RenderBackend::VelloCpu,
        wgpu::DeviceType::Cpu => RenderBackend::TinySkia,
        _ => RenderBackend::VelloGpu,
    }
}

/// Return per-backend performance defaults.
///
/// Values copied verbatim from mlc `chart-app-vello/src/main.rs`.
pub fn default_perf(backend: RenderBackend) -> PerfDefaults {
    let (fps_limit, msaa_samples) = match backend {
        RenderBackend::VelloGpu => (120u32, 8u8),
        RenderBackend::VelloCpu => (30, 0),
        RenderBackend::TinySkia => (90, 8),
        RenderBackend::InstancedWgpu => (90, 8),
        RenderBackend::VelloHybrid => (90, 8),
    };
    PerfDefaults { fps_limit, msaa_samples }
}

/// Recommended rendering backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedBackend {
    /// GPU-accelerated Vello (discrete or integrated GPU)
    VelloGpu,
    /// CPU-based Vello (virtual GPU / software renderer)
    VelloCpu,
    /// tiny-skia fallback (no GPU at all)
    TinySkia,
}

/// GPU information extracted from adapter.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// GPU device name (e.g. "NVIDIA GeForce RTX 3080")
    pub name: String,
    /// GPU driver string
    pub driver: String,
    /// wgpu device type
    pub device_type: wgpu::DeviceType,
    /// Recommended backend based on device type
    pub recommended: RecommendedBackend,
}

/// Detect GPU and recommend a backend from a wgpu adapter.
///
/// Call this after creating a wgpu adapter (e.g. after `create_surface()`).
///
/// # Example
/// ```ignore
/// let info = adapter.get_info();
/// let gpu = uzor_autodetect::detect(&info);
/// println!("GPU: {}, recommended: {:?}", gpu.name, gpu.recommended);
/// ```
pub fn detect(info: &wgpu::AdapterInfo) -> GpuInfo {
    let recommended = match info.device_type {
        wgpu::DeviceType::DiscreteGpu => RecommendedBackend::VelloGpu,
        wgpu::DeviceType::IntegratedGpu => RecommendedBackend::VelloGpu,
        wgpu::DeviceType::VirtualGpu => RecommendedBackend::VelloCpu,
        wgpu::DeviceType::Cpu => RecommendedBackend::TinySkia,
        _ => RecommendedBackend::VelloGpu,
    };

    GpuInfo {
        name: info.name.clone(),
        driver: info.driver.clone(),
        device_type: info.device_type,
        recommended,
    }
}
