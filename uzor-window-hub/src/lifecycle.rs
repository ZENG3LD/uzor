//! `WindowProvider` — abstraction over OS window sources.
//!
//! Implemented by `uzor-window-desktop` (winit), `uzor-window-web` (DOM
//! canvas), and `uzor-window-mobile` (UIKit / Android). Used by the
//! `uzor-framework` runtime to poll events and drive redraws regardless
//! of platform.

use uzor::core::types::Rect;
use uzor::input::PlatformEvent;

// ── SoftwarePresenter ─────────────────────────────────────────────────────────

/// Push CPU-rasterized pixels to an OS window without a GPU.
///
/// Implemented by window providers that can wrap their native window in a
/// software back-buffer (e.g. softbuffer on desktop, `putImageData` on web).
///
/// The implementor must convert the RGBA8 input into whatever pixel format the
/// underlying OS surface requires (e.g. softbuffer expects `0x00RRGGBB` u32).
///
/// # Thread safety
///
/// `SoftwarePresenter` requires `Send` so it can be moved to whichever thread
/// drives the render loop.  It does **not** require `Sync` — softbuffer's
/// `Surface` is `Send` but not `Sync` on Windows; callers always hold an
/// exclusive `&mut` reference during presentation.
pub trait SoftwarePresenter: Send {
    /// Push a full-frame RGBA8 buffer to the OS window and present it.
    ///
    /// `pixels` must have exactly `width * height * 4` bytes in row-major
    /// `[R, G, B, A]` order.
    fn present(&mut self, pixels: &[u8], width: u32, height: u32);

    /// Notify the back-buffer of a window resize.
    ///
    /// Call this whenever the physical window size changes so the presenter
    /// can reallocate its internal buffer before the next [`present`](Self::present).
    fn resize(&mut self, width: u32, height: u32);
}

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

// ── Resize direction ─────────────────────────────────────────────────────────

/// Direction of a borderless-window resize drag, started via
/// [`WindowProvider::drag_resize_window`].  Mirrors winit's `ResizeDirection`
/// without forcing every consumer to depend on winit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    North, South, East, West,
    NorthEast, NorthWest, SouthEast, SouthWest,
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

    /// Begin an OS-level window drag operation.
    ///
    /// Call this on mouse-down within the custom title-bar drag zone.
    /// The platform will move the window as the user drags.
    /// Default: no-op for providers that don't support OS-level drag.
    fn drag_window(&mut self) {}

    /// Begin an OS-level window resize operation along the given edge or
    /// corner. Call this on mouse-down within the custom resize hit-zone of
    /// a borderless window. Default: no-op.
    fn drag_resize_window(&mut self, _direction: ResizeDirection) {}

    /// Minimize the window to the OS task strip. Default: no-op.
    fn set_minimized(&mut self, _on: bool) {}

    /// Toggle window maximize state. Default: no-op.
    fn set_maximized(&mut self, _on: bool) {}

    /// `true` when the window is currently maximized. Default: `false`.
    fn is_maximized(&self) -> bool { false }

    /// Request graceful application close (consumed by `should_close()`).
    /// Default: no-op.
    fn request_close(&mut self) {}

    /// Set or clear the OS window icon (taskbar / window caption).
    ///
    /// Pass `Some(icon)` to set a new icon, `None` to revert to the default.
    /// Default: no-op.
    fn set_window_icon(&mut self, _rgba: Option<RgbaIcon>) {}

    /// Set the window title at runtime.
    ///
    /// Default: no-op.
    fn set_title(&mut self, _title: &str) {}

    /// Show or hide the window.
    ///
    /// Default: no-op.
    fn set_visible(&mut self, _visible: bool) {}

    /// Create a software-presentation surface bound to this window.
    ///
    /// Returns a [`SoftwarePresenter`] that can receive CPU-rasterized RGBA8
    /// pixel buffers and push them to the OS window without GPU involvement.
    ///
    /// Returns `None` on platforms that do not support software surfaces
    /// (e.g. web canvas path not yet implemented).
    ///
    /// The default implementation returns `None`.
    fn create_software_presenter(&self) -> Option<Box<dyn SoftwarePresenter>> {
        None
    }
}
