//! High-level Application API for uzor-desktop
//!
//! This module provides a simple, batteries-included API for creating
//! desktop applications with uzor. It handles:
//! - Window creation via winit
//! - Event loop management
//! - Vello surface setup
//!
//! Applications are responsible for their own rendering implementation.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop as WinitEventLoop};
use winit::window::{Window, WindowId as WinitWindowId};

use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::PresentMode;
use vello::Scene;

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

/// Render surface provider for applications
pub struct RenderSurfaceProvider {
    pub window: Arc<Window>,
    pub render_context: RenderContext,
    pub surface: RenderSurface<'static>,
    pub scene: Scene,
}

impl RenderSurfaceProvider {
    /// Create a new render surface for the given window
    pub fn new(window: Arc<Window>) -> Self {
        let mut render_context = RenderContext::new();

        let size = window.inner_size();
        let surface = pollster::block_on(render_context.create_surface(
            window.clone(),
            size.width,
            size.height,
            PresentMode::AutoVsync,
        ))
        .expect("Failed to create surface");

        Self {
            window,
            render_context,
            surface,
            scene: Scene::new(),
        }
    }

    /// Get the scale factor for this window
    pub fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    /// Get the window size in physical pixels
    pub fn size(&self) -> (u32, u32) {
        let size = self.window.inner_size();
        (size.width, size.height)
    }

    /// Resize the render surface
    pub fn resize(&mut self, width: u32, height: u32) {
        self.render_context.resize_surface(&mut self.surface, width, height);
    }

    /// Request a redraw
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

/// User callback signature for application events
/// Return true to continue, false to exit
pub type EventCallback = dyn FnMut(&mut RenderSurfaceProvider, WindowEvent) -> bool;

/// High-level application that manages the window and event loop
pub struct Application {
    config: AppConfig,
    event_callback: Box<EventCallback>,
}

impl Application {
    /// Create a new application with the given configuration
    pub fn new(config: AppConfig, event_callback: Box<EventCallback>) -> Self {
        Self {
            config,
            event_callback,
        }
    }

    /// Create a new application with default configuration
    pub fn with_default_config(event_callback: Box<EventCallback>) -> Self {
        Self::new(AppConfig::default(), event_callback)
    }

    /// Run the application (consumes self)
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = WinitEventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app_handler = AppHandler::new(self.config, self.event_callback);
        event_loop.run_app(&mut app_handler)?;

        Ok(())
    }
}

/// Internal application handler for winit event loop
struct AppHandler {
    config: AppConfig,
    event_callback: Box<EventCallback>,
    surface_provider: Option<RenderSurfaceProvider>,
}

impl AppHandler {
    fn new(config: AppConfig, event_callback: Box<EventCallback>) -> Self {
        Self {
            config,
            event_callback,
            surface_provider: None,
        }
    }

    fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes()
            .with_title(self.config.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.width,
                self.config.height,
            ));

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Failed to create window: {}", e);
                return;
            }
        };

        self.surface_provider = Some(RenderSurfaceProvider::new(window));
    }
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface_provider.is_none() {
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

        if let WindowEvent::Resized(size) = event {
            if let Some(provider) = self.surface_provider.as_mut() {
                provider.resize(size.width, size.height);
            }
        }

        // Call user event callback
        if let Some(provider) = self.surface_provider.as_mut() {
            let should_continue = (self.event_callback)(provider, event);
            if !should_continue {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Application can request redraws via RenderSurfaceProvider
    }
}
