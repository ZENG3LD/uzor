//! Desktop backend for uzor using winit
//!
//! This crate provides the desktop platform implementation for uzor,
//! supporting Windows, macOS, and Linux.
//!
//! Rendering is intentionally out of scope — consumers bring their own
//! backend (vello, wgpu-instanced, tiny-skia, …) and receive a raw
//! `Arc<winit::window::Window>` via the event callback.

pub use uzor;

// Re-export windowing dependency to avoid version conflicts
pub use winit;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop as WinitEventLoop};
use winit::window::{Window, WindowId as WinitWindowId};

use uzor::platform::backends::PlatformBackend;
use uzor::platform::types::{PlatformError, WindowId, SystemIntegration, EventResult};
use uzor::platform::{PlatformEvent, SystemTheme, WindowConfig};

mod event_mapper;
pub mod application;

use event_mapper::EventMapper;

// Re-export high-level API for convenience
pub use application::{Application, AppConfig};

// =============================================================================
// Desktop Platform
// =============================================================================

/// Desktop platform backend using winit
pub struct DesktopPlatform {
    /// Shared state between event loop and platform
    state: Arc<Mutex<PlatformState>>,
}

/// Internal platform state
struct PlatformState {
    /// Active windows
    windows: Vec<DesktopWindow>,
    /// Next window ID to assign (managed by WindowId::new() in core now)
    event_queue: VecDeque<PlatformEvent>,
    /// Clipboard backend
    clipboard: Option<arboard::Clipboard>,
    /// Pending window creation requests
    pending_windows: VecDeque<(WindowId, WindowConfig)>,
}

/// Desktop window wrapper
struct DesktopWindow {
    id: WindowId,
    #[allow(dead_code)]
    winit_id: WinitWindowId,
    window: Arc<Window>,
    #[allow(dead_code)]
    scale_factor: f64,
}

impl DesktopPlatform {
    /// Create a new desktop platform
    pub fn new() -> Result<Self, PlatformError> {
        let clipboard = arboard::Clipboard::new().ok();

        Ok(Self {
            state: Arc::new(Mutex::new(PlatformState {
                windows: Vec::new(),
                event_queue: VecDeque::new(),
                clipboard,
                pending_windows: VecDeque::new(),
            })),
        })
    }

    /// Run the event loop
    pub fn run<F>(self, callback: F)
    where
        F: FnMut(PlatformEvent) -> EventResult + 'static,
    {
        let event_loop = WinitEventLoop::new()
            .expect("Failed to create event loop");

        let mut app = DesktopApp {
            state: self.state.clone(),
            callback: Box::new(callback),
            should_exit: false,
        };

        let _ = event_loop.run_app(&mut app);
    }
}

impl PlatformBackend for DesktopPlatform {
    fn name(&self) -> &'static str {
        "desktop"
    }

    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError> {
        let mut state = self.state.lock().unwrap();
        let window_id = WindowId::new();
        state.pending_windows.push_back((window_id, config));
        Ok(window_id)
    }

    fn close_window(&mut self, window_id: WindowId) -> Result<(), PlatformError> {
        let mut state = self.state.lock().unwrap();
        let index = state.windows.iter().position(|w| w.id == window_id)
            .ok_or(PlatformError::WindowNotFound)?;
        state.windows.remove(index);
        Ok(())
    }

    fn primary_window(&self) -> Option<WindowId> {
        let state = self.state.lock().unwrap();
        state.windows.first().map(|w| w.id)
    }

    fn poll_events(&mut self) -> Vec<PlatformEvent> {
        let mut state = self.state.lock().unwrap();
        state.event_queue.drain(..).collect()
    }

    fn request_redraw(&self, window_id: WindowId) {
        let state = self.state.lock().unwrap();
        if let Some(w) = state.windows.iter().find(|w| w.id == window_id) {
            w.window.request_redraw();
        }
    }
}

impl SystemIntegration for DesktopPlatform {
    fn get_clipboard(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap();
        state.clipboard.as_mut()?.get_text().ok()
    }

    fn set_clipboard(&self, text: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(cb) = state.clipboard.as_mut() {
            let _ = cb.set_text(text);
        }
    }

    fn get_system_theme(&self) -> Option<SystemTheme> {
        match dark_light::detect() {
            dark_light::Mode::Dark => Some(SystemTheme::Dark),
            dark_light::Mode::Light => Some(SystemTheme::Light),
            dark_light::Mode::Default => None,
        }
    }
}

/// Application handler for winit event loop
struct DesktopApp {
    state: Arc<Mutex<PlatformState>>,
    callback: Box<dyn FnMut(PlatformEvent) -> EventResult>,
    should_exit: bool,
}

impl ApplicationHandler for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.create_pending_windows(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if let Some(platform_event) = EventMapper::map_window_event(&event) {
            let result = (self.callback)(platform_event);
            match result {
                EventResult::Exit => {
                    self.should_exit = true;
                    event_loop.exit();
                }
                EventResult::Redraw => {
                    let state = self.state.lock().unwrap();
                    for window in &state.windows {
                        window.window.request_redraw();
                    }
                }
                EventResult::Continue => {}
            }
        }

        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.create_pending_windows(event_loop);
        if self.should_exit {
            event_loop.exit();
        }
    }
}

impl DesktopApp {
    fn create_pending_windows(&mut self, event_loop: &ActiveEventLoop) {
        let mut state = self.state.lock().unwrap();
        while let Some((window_id, config)) = state.pending_windows.pop_front() {
            let window_attrs = Window::default_attributes()
                .with_title(config.title.clone())
                .with_inner_size(winit::dpi::LogicalSize::new(config.width, config.height));

            match event_loop.create_window(window_attrs) {
                Ok(window) => {
                    let winit_id = window.id();
                    let scale_factor = window.scale_factor();
                    state.windows.push(DesktopWindow {
                        id: window_id,
                        winit_id,
                        window: Arc::new(window),
                        scale_factor,
                    });
                }
                Err(e) => eprintln!("Failed to create window: {}", e),
            }
        }
    }
}