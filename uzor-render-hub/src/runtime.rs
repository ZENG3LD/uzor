//! Runtime helper that bundles [`RenderBackend`] choice + [`WindowRenderState`]
//! lifecycle in one place.
//!
//! `uzor-framework` instantiates one [`RuntimeBackend`] per window and calls
//! [`begin_frame`](RuntimeBackend::begin_frame) / [`submit_frame`](RuntimeBackend::submit_frame)
//! each tick without knowing which concrete backend is active.
//!
//! # Existing surface (for reference)
//!
//! | Item | Where |
//! |------|-------|
//! | [`RenderBackend`] enum | `backend.rs` — 5 variants: VelloGpu, InstancedWgpu, VelloCpu, VelloHybrid, TinySkia |
//! | [`WindowRenderState`] | `factory.rs` — persistent per-window renderer state (lazy renderer slots, scene, cpu_pixels) |
//! | [`submit_frame`](crate::submit_frame) | `submit.rs` — dispatches GPU/CPU submission by backend |
//! | [`BackendContext`] | `factory.rs` — per-frame caller-built context fed into submit |
//! | [`SubmitOutcome`] | `submit.rs` — frame timing metrics + `surface_lost` flag |

use crate::{RenderBackend, WindowRenderState};
use crate::submit::{SubmitOutcome, SubmitParams};

/// Per-window backend bundle used by `uzor-framework` runtime.
///
/// Wraps [`WindowRenderState`] (which already owns the active
/// [`RenderBackend`]) and exposes the two frame-lifecycle methods the runtime
/// needs: [`begin_frame`](Self::begin_frame) and [`submit_frame`](Self::submit_frame).
///
/// Switching backends at runtime is supported via
/// [`WindowRenderState::set_backend`] on the inner `state` field.
pub struct RuntimeBackend {
    /// Persistent per-window render state (renderers, scene, pixel buffer).
    pub state: WindowRenderState,
}

impl RuntimeBackend {
    /// Wrap an existing [`WindowRenderState`] into a `RuntimeBackend`.
    ///
    /// The active backend is determined by `state.backend`. Call
    /// [`WindowRenderState::new`] to construct the state before wrapping.
    pub fn new(state: WindowRenderState) -> Self {
        Self { state }
    }

    /// Convenience constructor — equivalent to wrapping a freshly-built state.
    ///
    /// `vello_renderer` is required even for non-vello backends because
    /// `WindowRenderState` uses it for the CPU-to-swapchain blit pipeline.
    /// See [`WindowRenderState::new`] for details.
    pub fn with_backend(backend: RenderBackend, vello_renderer: vello::Renderer) -> Self {
        Self::new(WindowRenderState::new(backend, vello_renderer))
    }

    /// Returns the active [`RenderBackend`] variant.
    pub fn backend(&self) -> RenderBackend {
        self.state.backend
    }

    /// Per-frame reset — clears the vello scene, instance-command list, and
    /// hybrid context. Call once at the **top** of each frame before issuing
    /// draw calls.
    pub fn begin_frame(&mut self) {
        self.state.begin_frame();
    }

    /// Submit accumulated draw calls to the GPU/CPU and present the swapchain.
    ///
    /// Delegates to the free [`crate::submit_frame`] function. Returns
    /// [`SubmitOutcome`] containing frame-timing metrics and a `surface_lost`
    /// flag the caller uses to decide whether to close the window.
    pub fn submit_frame(&mut self, params: SubmitParams<'_>) -> SubmitOutcome {
        crate::submit_frame(&mut self.state, params)
    }
}
