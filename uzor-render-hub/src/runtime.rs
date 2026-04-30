//! Runtime helper that bundles a [`WindowRenderState`] with the two
//! frame-lifecycle methods the framework runtime needs.
//!
//! `uzor-framework` instantiates one [`RuntimeBackend`] per window and calls
//! [`begin_frame`](RuntimeBackend::begin_frame) /
//! [`submit_frame`](RuntimeBackend::submit_frame) each tick without knowing
//! which concrete backend variant is active.
//!
//! # Architecture summary (post-refactor)
//!
//! | Item | Location |
//! |------|----------|
//! | [`crate::RenderBackend`] enum | `backend.rs` |
//! | [`WindowRenderState`] flat struct | `factory.rs` — all renderer slots + SurfaceMode |
//! | [`crate::RenderHub`] | `hub.rs` — pool + settings + metrics |
//! | [`submit_frame`](crate::submit_frame) | `submit.rs` — dispatches by `state.active` |
//! | [`SubmitOutcome`] | `submit.rs` — timing + `surface_lost` flag |

use crate::hub::RenderHub;
use crate::{RenderBackend, WindowRenderState};
use crate::submit::{SubmitOutcome, SubmitParams};

/// Per-window backend bundle used by `uzor-framework`.
///
/// Wraps a [`WindowRenderState`] (which owns all GPU resources including the
/// `RenderSurface`) and exposes the two frame-lifecycle methods:
/// [`begin_frame`](Self::begin_frame) and [`submit_frame`](Self::submit_frame).
pub struct RuntimeBackend {
    /// Persistent per-window render state (renderers, scene, pixel buffer).
    pub state: WindowRenderState,
}

impl RuntimeBackend {
    /// Wrap an existing [`WindowRenderState`] into a `RuntimeBackend`.
    pub fn new(state: WindowRenderState) -> Self {
        Self { state }
    }

    /// Returns the active [`RenderBackend`] variant.
    pub fn backend(&self) -> RenderBackend {
        self.state.backend()
    }

    /// Per-frame reset — clears the vello scene for GPU, or resets the CPU
    /// context for CPU backends.  Call once at the **top** of each frame
    /// before issuing draw calls.
    pub fn begin_frame(&mut self) {
        self.state.begin_frame();
    }

    /// Submit accumulated draw calls to the GPU/CPU and present the swapchain.
    ///
    /// Delegates to the free [`crate::submit_frame`] function.  Returns
    /// [`SubmitOutcome`] containing frame-timing metrics and a `surface_lost`
    /// flag the caller uses to decide whether to close the window.
    pub fn submit_frame(&mut self, params: SubmitParams) -> SubmitOutcome {
        crate::submit_frame(&mut self.state, params)
    }

    /// Apply a [`RenderHub`]'s active backend to this render state.
    ///
    /// Call after [`RenderHub::set_active`] to propagate the switch to the
    /// per-window state.
    pub fn apply_hub_backend(&mut self, hub: &RenderHub) {
        self.state.set_active(hub.active());
    }
}
