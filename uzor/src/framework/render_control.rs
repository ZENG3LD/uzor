use crate::platform::types::RenderBackend;

/// Runtime control over the render backend / performance settings.
///
/// Implemented by L4 runtimes (`uzor-desktop::Manager` etc.) and exposed
/// to apps through `WindowCtx::render_control`.  Apps must NOT depend on
/// any concrete render-hub type — only this trait.
pub trait RenderControl {
    /// Currently active backend.
    fn active_backend(&self) -> RenderBackend;
    /// Backends available in the runtime's pool (autodetect probes them).
    fn available_backends(&self) -> Vec<RenderBackend>;
    /// Switch the active backend. Silently no-op if `b` isn't in the pool.
    fn set_backend(&mut self, b: RenderBackend);

    fn fps_limit(&self) -> u32;
    fn set_fps_limit(&mut self, fps: u32);

    fn msaa_samples(&self) -> u8;
    fn set_msaa_samples(&mut self, n: u8);

    fn vsync(&self) -> bool;
    fn set_vsync(&mut self, on: bool);

    /// Current measured FPS (smoothed via EMA, α=0.1, like mlc).
    /// Updated by the runtime once per frame from frame timing.
    /// Default: returns 0.0 (runtimes that don't track this).
    fn measured_fps(&self) -> f32 { 0.0 }

    /// Last frame time in milliseconds (smoothed via EMA).
    /// Default: returns 0.0.
    fn last_frame_time_ms(&self) -> f32 { 0.0 }

    /// Total frames rendered since the runtime started.
    /// Default: returns 0.
    fn frame_count(&self) -> u64 { 0 }
}
