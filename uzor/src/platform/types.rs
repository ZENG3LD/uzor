//! Platform-specific types

use std::sync::atomic::{AtomicUsize, Ordering};

// ── RgbaIcon ──────────────────────────────────────────────────────────────────

/// RGBA image used to set the OS window or system-tray icon.
///
/// `pixels` must be exactly `width * height * 4` bytes in row-major RGBA order.
#[derive(Debug, Clone)]
pub struct RgbaIcon {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Raw RGBA pixel data: `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

impl RgbaIcon {
    /// Construct from an RGBA pixel buffer.
    ///
    /// # Panics (debug only)
    ///
    /// Asserts that `pixels.len() == width * height * 4` in debug builds.
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        debug_assert_eq!(
            pixels.len(),
            (width * height * 4) as usize,
            "RgbaIcon: pixel buffer length must equal width*height*4"
        );
        Self { width, height, pixels }
    }
}

// ── ResizeDirection ───────────────────────────────────────────────────────────

/// Direction of a borderless-window resize drag, started via
/// `WindowProvider::drag_resize_window`.  Mirrors winit's `ResizeDirection`
/// without forcing every consumer to depend on winit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    North, South, East, West,
    NorthEast, NorthWest, SouthEast, SouthWest,
}

// ── RenderBackend ─────────────────────────────────────────────────────────────

/// All rendering backends supported by uzor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// HTML Canvas 2D backend (wasm32 only).
    Canvas2d,
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

    /// True if the backend renders into a DOM canvas (wasm32 only).
    pub fn is_canvas(self) -> bool {
        matches!(self, Self::Canvas2d)
    }

    /// Stable identifier suitable for config files / UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VelloGpu      => "vello_gpu",
            Self::InstancedWgpu => "instanced_wgpu",
            Self::VelloCpu      => "vello_cpu",
            Self::VelloHybrid   => "vello_hybrid",
            Self::TinySkia      => "tiny_skia",
            Self::Canvas2d      => "canvas2d",
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
            Self::Canvas2d      => "Canvas 2D (Web)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WindowId(usize);

impl WindowId {
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Window not found")]
    WindowNotFound,
    #[error("Failed to create window: {0}")]
    CreationFailed(String),
    #[error("Platform operation not supported")]
    NotSupported,
    #[error("System error: {0}")]
    SystemError(String),
}

pub trait RenderSurface: Send + Sync {
    fn size(&self) -> (u32, u32);
}

pub trait SystemIntegration {
    fn get_clipboard(&self) -> Option<String>;
    fn set_clipboard(&self, text: &str);
    fn get_system_theme(&self) -> Option<crate::input::core::SystemTheme>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    Continue,
    Redraw,
    Exit,
}
