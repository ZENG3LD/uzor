//! `WindowProvider` — abstraction over OS window sources.
//!
//! Implemented by `uzor-window-desktop` (winit), `uzor-window-web` (DOM
//! canvas), and `uzor-window-mobile` (UIKit / Android). Used by the
//! `uzor-framework` runtime to poll events and drive redraws regardless
//! of platform.

use uzor::core::types::Rect;
use uzor::input::PlatformEvent;

// ── Opaque window-handle wrapper ─────────────────────────────────────────────

/// Platform-specific window handle, erased to avoid forcing low-level
/// window dependencies on every consumer.
///
/// Concrete backend implementations return the appropriate variant; the
/// GPU surface factory downcasts via [`std::any::Any`].
pub enum RawHandle {
    /// Raw window + display handle pair from the `raw-window-handle` crate,
    /// boxed to avoid a direct dep on that crate at this level.
    RawWindowHandle(Box<dyn std::any::Any + Send + Sync>),
    /// HTML `<canvas>` element handle for WASM / web backends.
    Canvas(Box<dyn std::any::Any + Send + Sync>),
    /// `CALayer` pointer for iOS / macOS Metal backends.
    CALayer(Box<dyn std::any::Any + Send + Sync>),
}

// ── WindowProvider trait ──────────────────────────────────────────────────────

/// Abstraction over any OS window source.
///
/// `uzor-framework` depends on this trait only — it never imports a
/// platform crate (`uzor-window-desktop`, `uzor-window-web`, …) directly.
/// Each platform crate provides a concrete implementation.
///
/// # Frame lifecycle
///
/// ```text
/// loop {
///     let events = provider.poll_events();   // 1 — drain OS queue
///     // … feed events to widgets …
///     provider.request_redraw();             // 2 — schedule vsync
///     // … build scene, submit frame …
///     if provider.should_close() { break; }  // 3 — exit check
/// }
/// ```
pub trait WindowProvider {
    /// Drain pending OS events and translate them to uzor [`PlatformEvent`]s.
    ///
    /// Called once per frame at the top of the runtime tick. The returned
    /// `Vec` may be empty between frames.
    fn poll_events(&mut self) -> Vec<PlatformEvent>;

    /// Current logical rect of the window (origin + size in logical pixels).
    ///
    /// Origin is typically `(0, 0)` for single-window apps; multi-window
    /// runtimes may use non-zero origins for window positioning.
    fn window_rect(&self) -> Rect;

    /// Current device pixel ratio (HiDPI scale factor).
    ///
    /// Multiply logical dimensions by this value to get physical pixels.
    /// Typically `1.0` on standard displays, `2.0` on Retina / 4K.
    fn scale_factor(&self) -> f64;

    /// Request a redraw on the next vsync.
    ///
    /// Idempotent — calling it multiple times before the next frame is
    /// allowed. The platform batches the request.
    fn request_redraw(&mut self);

    /// `true` when the OS has signalled the application should exit.
    ///
    /// Triggered by the user clicking the close button, a system shutdown,
    /// or an explicit [`PlatformEvent::WindowCloseRequested`]. The runtime
    /// checks this once per frame and begins its teardown sequence when true.
    fn should_close(&self) -> bool;

    /// Optional raw handle for GPU surface creation.
    ///
    /// Returns `None` on backends that don't need a native window handle
    /// (e.g. pure software / tiny-skia rendering to a buffer). Concrete
    /// backends such as `uzor-window-desktop` return the appropriate
    /// [`RawHandle`] variant so the GPU surface factory can downcast it.
    fn raw_window_handle(&self) -> Option<RawHandle>;
}
