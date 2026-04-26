//! Render backend enum.

use serde::{Deserialize, Serialize};

/// All rendering backends supported by uzor.
///
/// Copied verbatim from `sidebar_content::state::RenderBackend` in mylittlechart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackend {
    /// Full vello GPU pipeline. Best on discrete / integrated GPUs.
    VelloGpu,
    /// Custom wgpu instanced renderer (lighter than vello, suitable for
    /// many shapes / many glyphs at high frame rates).
    InstancedWgpu,
    /// vello running on CPU (vello_cpu). Used on virtual GPUs / WARP.
    VelloCpu,
    /// vello hybrid — CPU strip encoding + GPU fine rasterization.
    VelloHybrid,
    /// Pure CPU tiny-skia fallback.
    TinySkia,
}

impl RenderBackend {
    /// True if the backend renders into a CPU pixel buffer.
    pub fn is_cpu(self) -> bool {
        matches!(self, Self::VelloCpu | Self::TinySkia)
    }

    /// True if the backend renders directly to the swapchain on the GPU.
    pub fn is_gpu_swapchain(self) -> bool {
        matches!(self, Self::VelloGpu | Self::InstancedWgpu | Self::VelloHybrid)
    }

    /// Stable identifier suitable for config files / UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VelloGpu      => "vello_gpu",
            Self::InstancedWgpu => "instanced_wgpu",
            Self::VelloCpu      => "vello_cpu",
            Self::VelloHybrid   => "vello_hybrid",
            Self::TinySkia      => "tiny_skia",
        }
    }

    /// Human-readable label suitable for dropdowns.
    pub fn label(self) -> &'static str {
        match self {
            Self::VelloGpu      => "Vello GPU",
            Self::InstancedWgpu => "Instanced wGPU",
            Self::VelloCpu      => "Vello CPU",
            Self::VelloHybrid   => "Vello Hybrid",
            Self::TinySkia      => "Tiny-Skia CPU",
        }
    }
}
