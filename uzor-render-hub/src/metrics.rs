//! Per-frame render metrics.
//!
//! Collected by hub during submit; consumers (overlays, telemetry, debug
//! panels) can read the latest snapshot. Hub does NOT render any UI for
//! metrics — that is the consumer's job.

/// One frame's worth of render-side timing + counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderMetrics {
    /// Total wall time for `submit_frame` (microseconds).
    pub submit_us: u64,
    /// Time spent producing the off-screen target texture
    /// (vello render-to-texture, CPU pixel upload, etc.) — subset of `submit_us`.
    pub render_to_texture_us: u64,
    /// Time spent on `surface.get_current_texture` + `surface.present` — subset of `submit_us`.
    pub present_us: u64,

    /// Active backend that produced this frame.
    pub backend: Option<crate::backend::RenderBackend>,

    /// Number of distinct draw calls submitted (best-effort, populated where
    /// the backend exposes a counter; 0 otherwise).
    pub draw_calls: u32,
    /// Total instances/vertices submitted this frame (best-effort).
    pub primitives: u32,
}

impl RenderMetrics {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
