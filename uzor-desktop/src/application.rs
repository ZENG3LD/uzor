//! High-level Application API for uzor-desktop
//!
//! This module provides a simple, batteries-included API for creating
//! desktop applications with uzor. It handles:
//! - Window creation via winit
//! - Event loop management
//!
//! Rendering is the caller's responsibility — this crate only manages
//! the window lifecycle and dispatches winit events.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop as WinitEventLoop};
use winit::window::{Window, WindowId as WinitWindowId};

/// Configuration for application window
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Window title
    pub title: String,
    /// Window width in logical pixels
    pub width: u32,
    /// Window height in logical pixels
    pub height: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "UZOR Application".to_string(),
            width: 1280,
            height: 720,
        }
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

/// User callback signature for application events.
///
/// Receives the winit `Window` (so the caller can create their own render
/// surface) and the `WindowEvent`. Return `true` to continue, `false` to exit.
pub type EventCallback = dyn FnMut(Arc<Window>, WindowEvent) -> bool;

/// High-level application that manages the window and event loop.
///
/// This type is intentionally renderer-agnostic. Pass any backend
/// (vello, wgpu-instanced, tiny-skia, …) through the event callback.
pub struct Application {
    config: AppConfig,
    event_callback: Box<EventCallback>,
}

impl Application {
    /// Create a new application with the given configuration.
    pub fn new(config: AppConfig, event_callback: Box<EventCallback>) -> Self {
        Self {
            config,
            event_callback,
        }
    }

    /// Create a new application with default configuration.
    pub fn with_default_config(event_callback: Box<EventCallback>) -> Self {
        Self::new(AppConfig::default(), event_callback)
    }

    /// Run the application (consumes self).
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = WinitEventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app_handler = AppHandler::new(self.config, self.event_callback);
        event_loop.run_app(&mut app_handler)?;

        Ok(())
    }
}

// =============================================================================
// Internal winit application handler
// =============================================================================

struct AppHandler {
    config: AppConfig,
    event_callback: Box<EventCallback>,
    window: Option<Arc<Window>>,
}

impl AppHandler {
    fn new(config: AppConfig, event_callback: Box<EventCallback>) -> Self {
        Self {
            config,
            event_callback,
            window: None,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));

        match event_loop.create_window(window_attrs) {
            Ok(w) => self.window = Some(Arc::new(w)),
            Err(e) => eprintln!("Failed to create window: {}", e),
        }
    }
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.create_window(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
            return;
        }

        if let Some(window) = self.window.clone() {
            let should_continue = (self.event_callback)(window, event);
            if !should_continue {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Callers trigger redraws by calling window.request_redraw() themselves.
    }
}
