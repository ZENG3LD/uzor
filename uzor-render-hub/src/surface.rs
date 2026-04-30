//! `RenderSurfaceFactory` — bridge from window handle to backend render state.
//!
//! Each window provider crate (`uzor-window-desktop`, `uzor-window-web`,
//! `uzor-window-mobile`) supplies a factory implementation that knows how to
//! convert its raw handle type into a backend-appropriate [`WindowRenderState`].
//!
//! The runtime (`uzor-framework`) holds a `Box<dyn RenderSurfaceFactory>` and
//! calls [`RenderSurfaceFactory::create_render_state`] once per window. The
//! factory chooses the right surface initialization path based on the
//! [`RenderBackend`] variant.

use uzor_window_hub::RawHandle;

use crate::{RenderBackend, WindowRenderState};

// ── SurfaceError ──────────────────────────────────────────────────────────────

/// Errors a [`RenderSurfaceFactory`] may produce while creating a render state.
#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    /// The factory does not support the requested backend.
    #[error("backend {0:?} not supported by this factory")]
    UnsupportedBackend(RenderBackend),

    /// The handle variant is incompatible with the backend (e.g. `Canvas`
    /// handle paired with `VelloGpu`).
    #[error("handle variant incompatible with backend {0:?}")]
    HandleMismatch(RenderBackend),

    /// The window handle is `None` — the window has not yet been realized.
    #[error("window handle not available")]
    HandleUnavailable,

    /// Underlying GPU or surface initialization failed.
    #[error("surface init failed: {0}")]
    InitFailed(String),
}

// ── SurfaceSize ───────────────────────────────────────────────────────────────

/// Initial size of the render surface in physical pixels.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceSize {
    /// Physical-pixel width of the surface.
    pub width: u32,
    /// Physical-pixel height of the surface.
    pub height: u32,
}

// ── RenderSurfaceFactory ──────────────────────────────────────────────────────

/// Converts a [`RawHandle`] + [`RenderBackend`] into a ready-to-render
/// [`WindowRenderState`].
///
/// Implementations live in window-provider crates:
/// - `uzor-window-desktop` — winit `Window` → vello-gpu factory
/// - `uzor-window-web` — `<canvas>` → canvas2d factory
/// - `uzor-window-mobile` — `CALayer` → Metal-backed factory
///
/// The factory is free to support one or many backends; unsupported
/// `(handle, backend)` pairs should return [`SurfaceError::UnsupportedBackend`]
/// or [`SurfaceError::HandleMismatch`] as appropriate.
///
/// # Thread safety
///
/// `Send + Sync` is required so the factory can be stored in a
/// `Box<dyn RenderSurfaceFactory>` across the async framework runtime.
pub trait RenderSurfaceFactory: Send + Sync {
    /// Build a fresh [`WindowRenderState`] for the given window handle and backend.
    ///
    /// Called once per window during runtime initialization, before the main
    /// event loop starts. The returned state is owned by the runtime and
    /// drives all subsequent frame submissions.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError`] if the handle is unavailable, the combination
    /// is unsupported, or GPU initialization fails.
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError>;

    /// Returns `true` if this factory can handle the given `(handle, backend)` pair.
    ///
    /// The default implementation returns `true` unconditionally. Implementations
    /// should override with a cheap discriminant check so callers can probe
    /// compatibility without paying the cost of a full
    /// [`create_render_state`](Self::create_render_state) call.
    fn supports(&self, _handle: &RawHandle, _backend: RenderBackend) -> bool {
        true
    }
}
