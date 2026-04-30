//! [`WinitWindowProvider`] — implements [`crate::lifecycle::WindowProvider`]
//! over an [`Arc<winit::window::Window>`].
//!
//! Available only when the `desktop` feature is active (default).
//!
//! # Ownership model
//!
//! `WinitWindowProvider` does NOT own the winit `EventLoop`. The caller creates
//! the event loop, creates a window on `Resumed`, wraps it with
//! `WinitWindowProvider::new`, and feeds `WindowEvent`s via
//! [`push_winit_event`](WinitWindowProvider::push_winit_event) from inside the
//! `ApplicationHandler` callback.
//!
//! ```rust,ignore
//! // Inside ApplicationHandler::window_event:
//! provider.push_winit_event(&event);
//! if matches!(event, WindowEvent::CloseRequested) {
//!     provider.mark_close();
//! }
//! ```

use std::sync::Arc;

use winit::event::WindowEvent;
use winit::raw_window_handle::{RawWindowHandle, RawDisplayHandle};
use winit::window::Window;

use uzor::core::types::Rect;
use uzor::platform::PlatformEvent;

use crate::lifecycle::{RawHandle, WindowProvider};

// ─── SendSyncHandlePair ───────────────────────────────────────────────────────

/// Newtype wrapping `(RawWindowHandle, RawDisplayHandle)` with manual `Send +
/// Sync` impls.
///
/// `RawWindowHandle` is not automatically `Send + Sync` because some platform
/// variants (e.g. `UiKitWindowHandle`) contain `NonNull<c_void>`.
/// On all supported desktop platforms (Win32, macOS, X11/Wayland) raw handles
/// are safe to copy between threads when the underlying OS object remains alive.
///
/// # Safety
///
/// The caller must ensure the OS window / display outlive any thread that reads
/// these handles.
pub struct SendSyncHandlePair(pub RawWindowHandle, pub RawDisplayHandle);

// SAFETY: see doc comment above.
unsafe impl Send for SendSyncHandlePair {}
unsafe impl Sync for SendSyncHandlePair {}

/// Desktop window provider backed by a winit [`Window`].
///
/// Implements [`WindowProvider`] so it can be passed to
/// `uzor_framework::AppBuilder::window(Box::new(provider))`.
///
/// Create one per window after the winit event loop has been started
/// (i.e. inside `ApplicationHandler::resumed`).
pub struct WinitWindowProvider {
    window: Arc<Window>,
    pending_events: Vec<PlatformEvent>,
    should_close: bool,
}

impl WinitWindowProvider {
    /// Construct from an existing winit `Window`.
    ///
    /// The caller is responsible for continuing to drive the winit `EventLoop`
    /// and feeding events via [`push_winit_event`](Self::push_winit_event).
    pub fn new(window: Arc<Window>) -> Self {
        Self {
            window,
            pending_events: Vec::new(),
            should_close: false,
        }
    }

    /// Translate a winit `WindowEvent` and push it onto the internal queue.
    ///
    /// Call this from your `ApplicationHandler::window_event` implementation
    /// before delegating to the framework runtime.
    pub fn push_winit_event(&mut self, event: &WindowEvent) {
        use uzor_window_desktop::event_mapper::EventMapper;
        if let Some(ev) = EventMapper::map_window_event(event) {
            self.pending_events.push(ev);
        }
    }

    /// Signal that the window should close on the next frame check.
    ///
    /// Call this when you receive `WindowEvent::CloseRequested`.
    pub fn mark_close(&mut self) {
        self.should_close = true;
    }
}

impl WindowProvider for WinitWindowProvider {
    /// Drain all buffered platform events and return them.
    ///
    /// Called once per frame by the runtime. The buffer is cleared after this call.
    fn poll_events(&mut self) -> Vec<PlatformEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Current logical rect of the window.
    ///
    /// Origin uses `outer_position` when available; falls back to `(0, 0)`.
    fn window_rect(&self) -> Rect {
        let size = self.window.inner_size();
        let pos = self
            .window
            .outer_position()
            .unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
        let scale = self.window.scale_factor();
        Rect::new(
            pos.x as f64 / scale,
            pos.y as f64 / scale,
            size.width as f64 / scale,
            size.height as f64 / scale,
        )
    }

    /// Device pixel ratio (HiDPI scale factor).
    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    /// Request an OS redraw for the next vsync.
    fn request_redraw(&mut self) {
        self.window.request_redraw();
    }

    /// `true` once [`mark_close`](Self::mark_close) has been called.
    fn should_close(&self) -> bool {
        self.should_close
    }

    /// Return a [`RawHandle::RawWindowHandle`] wrapping winit's raw window and
    /// display handles for GPU surface creation.
    ///
    /// Uses `winit::raw_window_handle::{HasWindowHandle, HasDisplayHandle}`.
    fn raw_window_handle(&self) -> Option<RawHandle> {
        use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

        let window_handle = self.window.window_handle().ok()?.as_raw();
        let display_handle = self.window.display_handle().ok()?.as_raw();

        // Wrap in a newtype that is Send + Sync.
        // SAFETY: raw window handles are plain integer/pointer data. On Windows
        // the HWND is a thread-local concept but wgpu's surface creation is
        // invoked synchronously from the main thread (inside the event loop
        // callback), so sharing the values across thread boundaries is safe in
        // this usage pattern. On other platforms the handles are similarly safe
        // to copy to the GPU thread.
        let pair: Box<dyn std::any::Any + Send + Sync> =
            Box::new(SendSyncHandlePair(window_handle, display_handle));

        Some(RawHandle::RawWindowHandle(pair))
    }
}
