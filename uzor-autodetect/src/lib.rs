//! GPU auto-detection for uzor applications.
//!
//! Recommends a rendering backend based on wgpu adapter capabilities.

use serde::{Deserialize, Serialize};

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
